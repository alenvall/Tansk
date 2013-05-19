package chalmers.TDA367.B17.states;

import java.awt.Dimension;
import java.io.IOException;
import java.util.*;
import java.util.Map.Entry;
import java.util.concurrent.ConcurrentLinkedQueue;
import org.newdawn.slick.*;
import org.newdawn.slick.geom.*;
import org.newdawn.slick.gui.AbstractComponent;
import org.newdawn.slick.gui.ComponentListener;
import org.newdawn.slick.gui.TextField;
import org.newdawn.slick.state.*;
import com.esotericsoftware.kryonet.*;
import com.esotericsoftware.minlog.Log;
import chalmers.TDA367.B17.*;
import chalmers.TDA367.B17.console.Console;
import chalmers.TDA367.B17.console.Console.*;
import chalmers.TDA367.B17.controller.*;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.model.*;
import chalmers.TDA367.B17.model.Entity.*;
import chalmers.TDA367.B17.network.*;
import chalmers.TDA367.B17.network.Network.*;
import chalmers.TDA367.B17.tanks.DefaultTank;
import chalmers.TDA367.B17.view.Lifebar;
import chalmers.TDA367.B17.weapons.DefaultProjectile;

public class ClientState extends BasicGameState {
	private int state;
	private static ClientState instance;
	private GameController controller;
	private ConcurrentLinkedQueue<Packet> packetQueue;
	
	private Client client;
	private boolean isConnected;
	private Input input;
	private Image map = null;
	private SpriteSheet entSprite;
	private String playerName;
	private boolean mapLoaded;
	private int frameCounter;
	private int updates;
	private AbstractTank playerTank;
	private Lifebar lifebar;
	private long deltaTime;
	private int packetsSentPerSecond;
	private int packetsSent;
	private int updatesPerSecond;
	private long oldTime;
	private int packetsRecPerSecond;
	private int packetsReceived;
	private TextField chatField;
		
	private ClientState(int state){
		this.state = state;
		controller = GameController.getInstance();
	}
	
	public static ClientState getInstance(){
		if(instance == null)
			instance = new ClientState(Tansk.CLIENT);
	
		return instance;
	}
	
	@Override
    public void init(GameContainer gc, StateBasedGame game) throws SlickException {
		gc.setSmoothDeltas(true);
		gc.setAlwaysRender(true);
		gc.setMouseCursor(new Image("data/crosshair.png"), 16, 16);
		
		map = new Image("data/map.png");
		playerName = "Nisse" + Math.round(Math.random() * 1000);
		lifebar = new Lifebar((Tansk.SCREEN_WIDTH/2)-100, 10, 200, 25);
    }
	
	@Override
	public void enter(GameContainer gc, StateBasedGame game) throws SlickException {
		super.enter(gc, game);

		controller.setConsole(new Console(10, 533, 450, 192, OutputLevel.ALL));
		chatField = new TextField(gc, gc.getDefaultFont(), 10, 733, 450, 23, new ComponentListener() {
			public void componentActivated(AbstractComponent source) {
				if(!chatField.getText().equals("")){
					if(client != null){
						String msg = playerName + ": " + chatField.getText();
						Pck31_ChatMessage pck = new Pck31_ChatMessage();
						pck.message = msg;
						chatField.setText("");
						client.sendTCP(pck);
					}
				}
			}
		});
		
		controller.setWorld(new World(new Dimension(Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT), false));
		
		packetQueue = new ConcurrentLinkedQueue<Packet>();
		client = new Client();
		Network.register(client);
		client.addListener(new Listener(){
			@Override
			public void connected(Connection connection) {
				Pck0_JoinRequest pck = new Pck0_JoinRequest();
				pck.playerName = ClientState.getInstance().getPlayerName();
			    client.sendTCP(pck);
			}
			
			public void received(Connection con, Object msg) {
			   super.received(con, msg);
			   if (msg instanceof Packet) {
				   Packet packet = (Packet)msg;
				   packet.setConnection(con);
				   packetQueue.add(packet);
				   packetsReceived++;
			   }
			}
			
			@Override
			public void disconnected(Connection connection) {
				GameController.getInstance().getConsole().addMsg("Disconnected from server.", MsgLevel.ERROR);
			}
		});
		client.start();
		
		try {
	        client.connect(600000, "127.0.0.1", Network.PORT, Network.PORT);
        } catch (IOException e) {
        	Log.info("[CLIENT] Failed to connect!");
			GameController.getInstance().getConsole().addMsg("Failed to connect to server.", MsgLevel.ERROR);
	        e.printStackTrace();
	        client.stop();
        } 
		
		controller.getWorld().init();

		input = gc.getInput();   
		input.addKeyListener(this);
		input.addMouseListener(this);
	}	
	
	@Override
    public void update(GameContainer gc, StateBasedGame game, int delta) throws SlickException {
		frameCounter++;
		long newTime = Calendar.getInstance().getTimeInMillis();
		deltaTime = newTime - oldTime;
		if(deltaTime >= 1000){
			packetsRecPerSecond = packetsReceived;
			packetsReceived = 0;
			packetsSentPerSecond = packetsSent;
			packetsSent = 0;
			updatesPerSecond = updates;
			updates = 0;
			oldTime = newTime;
		}
		
		if((isConnected) && (!mapLoaded)){
			MapLoader.createEntities("whatever");
			mapLoaded = true;
		}
		processPackets();
					
		sendClientInput(gc.getInput());
		updates++;
	}

	private void sendClientInput(Input input) {
		if(isConnected){
			if(playerTank != null){
				Pck4_ClientInput clientPck = new Pck4_ClientInput();
				clientPck.W_pressed = input.isKeyDown(Input.KEY_W);
				clientPck.A_pressed = input.isKeyDown(Input.KEY_A);
				clientPck.S_pressed = input.isKeyDown(Input.KEY_S);
				clientPck.D_pressed = input.isKeyDown(Input.KEY_D);
				clientPck.LMB_pressed = input.isMouseButtonDown(0);

				AbstractTurret playerTurret = playerTank.getTurret();
				clientPck.turretNewAngle = (float) Math.toDegrees(Math.atan2(playerTurret.getPosition().x - input.getMouseX() + 0, playerTurret.getPosition().y - input.getMouseY() + 0)* -1)+180;		
			
				client.sendTCP(clientPck);
				packetsSent++;
			}
		}
    }

	public void processPackets() {
		Packet packet;
		while ((packet = packetQueue.poll()) != null) {
			if(packet instanceof Pck1_LoginAnswer) {
				if(((Pck1_LoginAnswer) packet).accepted){
					Pck2_ClientConfirmJoin pck = new Pck2_ClientConfirmJoin();
					pck.message = playerName + " connected!";
					packet.getConnection().sendTCP(pck);
					isConnected = true;
					controller.getConsole().addMsg("Joined game!", MsgLevel.INFO);
				} else {
					GameController.getInstance().getConsole().addMsg("Connection refused by server.", MsgLevel.ERROR);
					GameController.getInstance().getConsole().addMsg("Reason: " + ((Pck1_LoginAnswer) packet).reason, MsgLevel.ERROR);
					packet.getConnection().close();
				}
			}
						
			if(packet instanceof Pck3_Message){
				if(packet instanceof Pck31_ChatMessage){
					GameController.getInstance().getConsole().addMsg(((Pck31_ChatMessage)packet).message);
				} else {
					String message = ((Pck3_Message) packet).message;
					GameController.getInstance().getConsole().addMsg(message, MsgLevel.INFO);
					Log.info(message);
				}
			}	
			
			if(packet instanceof Pck7_TankID){
				Pck7_TankID pck = (Pck7_TankID) packet;
				playerTank = (AbstractTank) controller.getWorld().getEntity(pck.tankID);
			}		
			
			if(packet instanceof Pck8_EntityDestroyed){
				Pck8_EntityDestroyed pck = (Pck8_EntityDestroyed) packet;
				controller.getWorld().removeEntity(pck.entityID);
			}
			
			if(packet instanceof Pck9_EntityCreated){
				Pck9_EntityCreated pck = (Pck9_EntityCreated) packet;
				createClientEntity(pck.entityID, pck.identifier);
			}
			
			if(packet instanceof Pck10_TankCreated){
				Pck10_TankCreated pck = (Pck10_TankCreated) packet;
				createClientTank(pck.entityID, pck.identifier, pck.direction);
			}
					
			if(packet instanceof Pck100_WorldState){
				if(isConnected)
					updateClientWorld((Pck100_WorldState) packet);
			}			
			
			if(packet instanceof Pck1000_GameEvent){
				Pck1000_GameEvent evtPck = (Pck1000_GameEvent)packet;
				GameController.getInstance().getWorld().handleEvent(new GameEvent(evtPck.eventType, evtPck.sourceID, evtPck.eventDesc));
			}
		}
    }			

	private void updateClientWorld(Pck100_WorldState worldState) {
		for(EntityPacket pck : worldState.updatePackets){
			if(pck instanceof Pck101_TankUpdate){
				Pck101_TankUpdate packet = (Pck101_TankUpdate) pck;;
				AbstractTank tank = (AbstractTank) controller.getWorld().getEntity(packet.entityID);
				tank.setPosition(packet.tankPosition);
				tank.setDirection(packet.tankDirection);
				tank.setHealth(packet.tankHealth);
				if(tank.getShield() != null)
					tank.getShield().setHealth(packet.tankShieldHealth);
				AbstractTurret turret = tank.getTurret();
				turret.setPosition(packet.turretPosition);
				turret.setRotation(packet.turretAngle);
			}
			if(pck instanceof Pck102_ProjectileUpdate){
				Pck102_ProjectileUpdate packet = (Pck102_ProjectileUpdate) pck;
				AbstractProjectile proj = (AbstractProjectile) controller.getWorld().getEntity(packet.entityID);
				if(proj != null){
					proj.setPosition(packet.projPosition);
					proj.setDirection(packet.projDirection);
				}
			}
		}
    }

	@Override
    public void render(GameContainer container, StateBasedGame game, Graphics g) throws SlickException {
		if(isConnected){
			g.drawImage(map, 0, 0);
			if(controller.getWorld().getEntities() != null){
				ArrayList<Entity> firstLayerEnts = new ArrayList<Entity>();
				ArrayList<Entity> secondLayerEnts = new ArrayList<Entity>();
				ArrayList<Entity> thirdLayerEnts = new ArrayList<Entity>();
				ArrayList<Entity> fourthLayerEnts = new ArrayList<Entity>();
	
				Iterator<Entry<Integer, Entity>> iterator = controller.getWorld().getEntities().entrySet().iterator();
				while(iterator.hasNext()){
					Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) iterator.next();
					Entity entity = entry.getValue();
					
					if(!entity.getSpriteID().equals("")){
						if(entity.getRenderLayer() == RenderLayer.FIRST)
							firstLayerEnts.add(entity);
						else if(entity.getRenderLayer() == RenderLayer.SECOND)
							secondLayerEnts.add(entity);
						else if(entity.getRenderLayer() == RenderLayer.THIRD)
							thirdLayerEnts.add(entity);
						else if(entity.getRenderLayer() == RenderLayer.FOURTH)
							fourthLayerEnts.add(entity);
					}
				}
				renderEntities(firstLayerEnts);
				renderEntities(secondLayerEnts);
				renderEntities(thirdLayerEnts);
				renderEntities(fourthLayerEnts);
			}
		}
		controller.getConsole().renderMessages(g);
		debugRender(g);
		controller.getAnimationHandler().renderAnimations();
		if(playerTank != null){
			if(playerTank.getHealth() > 0){
				if(playerTank.getShield() != null && playerTank.getShield().getHealth() <= 100){
					lifebar.render(playerTank.getHealth()/playerTank.getMaxHealth(), playerTank.getShield().getHealth()/playerTank.getMaxShieldHealth(), g);
				}else{
					lifebar.render(playerTank.getHealth()/playerTank.getMaxHealth(), 0, g);
				}
			}
		}
		g.setColor(Color.white);
		g.setLineWidth(1);
		g.drawRect(chatField.getX(), chatField.getY(), chatField.getWidth(), chatField.getHeight());
		chatField.render(container, g);
    }
	
	private void renderEntities(ArrayList<Entity> entities){
		for(Entity entity : entities){
			entSprite = GameController.getInstance().getImageHandler().getSprite(entity.getSpriteID());
			
			if(entSprite != null){
				if(entity instanceof AbstractTank){
					entSprite = GameController.getInstance().getImageHandler().getSprite(entity.getSpriteID());
					if(entity.getRotation()!=0){
						entSprite.setRotation((float) entity.getRotation());
						// draw sprite at the coordinates of the top left corner of tank when it is not rotated
						Shape nonRotatedShape = entity.getShape().transform(Transform.createRotateTransform((float)Math.toRadians(-entity.getRotation()), entity.getPosition().x, entity.getPosition().y));
						entSprite.draw(nonRotatedShape.getMinX(), nonRotatedShape.getMinY());
					} else {
						entSprite.draw(entity.getShape().getMinX(), entity.getShape().getMinY());
					}
				} else {
					if(entity instanceof AbstractTurret){
						entSprite.setCenterOfRotation(((AbstractTurret) entity).getTurretCenter().x, ((AbstractTurret) entity).getTurretCenter().y);
					}
					entSprite.setRotation((float) entity.getRotation());
					entSprite.draw(entity.getSpritePosition().x, entity.getSpritePosition().y);						
				}
			}
		}
	}
	
	public void debugRender(Graphics g){
//		if(playerTank != null){
//			g.setColor(Color.green);
//			g.draw(playerTank.getShape());
//			g.setColor(Color.yellow);
//			g.drawLine(playerTank.getSpritePosition().x, playerTank.getSpritePosition().y, Tansk.SCREEN_WIDTH/2, Tansk.SCREEN_HEIGHT/2);
//			g.setColor(Color.red);
//			g.drawLine(playerTank.getPosition().x, playerTank.getPosition().y, Tansk.SCREEN_WIDTH/2, Tansk.SCREEN_HEIGHT/2);
//			g.setColor(Color.blue);
//			g.drawLine(playerTank.getTurret().getTurretNozzle().x, playerTank.getTurret().getTurretNozzle().y, Tansk.SCREEN_WIDTH/2, Tansk.SCREEN_HEIGHT/2);
//		}
		g.setColor(Color.white);
		g.drawString("Packet rec/sec: " + packetsRecPerSecond, 18, 320);
		g.drawString("Packet sent/sec: " + packetsSentPerSecond, 18, 340);
		g.drawString("Update/sec: " + updatesPerSecond, 18, 360);
		g.drawString("Frame: " + frameCounter, 18, 400);
		g.drawString("Entities: " + controller.getWorld().getEntities().size(), 18, 420);
	}
    
	public String getPlayerName() {
	    return playerName;
    }
	
	@Override
    public int getID() {
	    return this.state;
    }
		
	private void createClientTank(int entityID, String identifier, Vector2f direction) {
	    if(identifier.equals("DefaultTank")){
			new DefaultTank(entityID, direction, null);
	    }
    }
	
	private void createClientEntity(int entityID, String identifier) {
	    if(identifier.equals("DefaultProjectile")){
			new DefaultProjectile(entityID, null, null);
	    }
    }
}
