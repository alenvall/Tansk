package chalmers.TDA367.B17.states;

import java.awt.Dimension;
import java.io.IOException;
import java.util.*;
import java.util.Map.Entry;
import java.util.concurrent.ConcurrentLinkedQueue;
import org.newdawn.slick.*;
import org.newdawn.slick.geom.*;
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
	private static final int UPDATE_INTERVAL = 20;
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
	private ClientButtons clientButtons;
	private int delta;
	private int frameCounter;
	private int timeSinceLastUpdate;
	private int updates;
	private AbstractTank playerTank;
	private Lifebar lifebar;
		
	private ClientState(int state){
		this.state = state;
		controller = GameController.getInstance();
		controller.setConsole(new Console(10, 565, 450, 192, OutputLevel.ALL));
	}
	
	public static ClientState getInstance(){
		if(instance == null)
			instance = new ClientState(Tansk.CLIENT);
	
		return instance;
	}
	
	@Override
    public void init(GameContainer gc, StateBasedGame game) throws SlickException {
		gc.setAlwaysRender(true);
		gc.setMouseCursor(new Image("data/crosshair.png"), 16, 16);
		
		clientButtons = new ClientButtons();
		map = new Image("data/map.png");
		playerName = "Nisse" + Math.round(Math.random() * 1000);
		lifebar = new Lifebar((Tansk.SCREEN_WIDTH/2)-100, 10, 200, 25);
    }
	
	@Override
	public void enter(GameContainer gc, StateBasedGame game) throws SlickException {
		super.enter(gc, game);
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
		this.delta = delta;
		if((isConnected) && (!mapLoaded)){
			MapLoader.createEntities("whatever");
			mapLoaded = true;
		}
		processPackets();
		
		timeSinceLastUpdate -= delta;
		if(timeSinceLastUpdate <= 0 && isConnected){
			sendClientInput(gc.getInput());
			timeSinceLastUpdate = UPDATE_INTERVAL;
			updates++;
		}
	}
	
	private void sendClientInput(Input input) {
		if(isConnected){
			// only update button if they have changed
			if(clientButtons.isStateChanged()){
				Pck4_ClientInput buttonPck = new Pck4_ClientInput();
				buttonPck.W_pressed = clientButtons.button_W_pressed;
				buttonPck.A_pressed = clientButtons.button_A_pressed;
				buttonPck.S_pressed = clientButtons.button_S_pressed;
				buttonPck.D_pressed = clientButtons.button_D_pressed;
				buttonPck.LMB_pressed = clientButtons.button_LMB_pressed;

				client.sendTCP(buttonPck);
				clientButtons.setStateChanged(false);
			}
			
			// always update angle
			if(playerTank != null){
				Pck5_ClientTurretAngle anglePck = new Pck5_ClientTurretAngle();
				AbstractTurret playerTurret = playerTank.getTurret();
				anglePck.turretNewAngle = (float) Math.toDegrees(Math.atan2(playerTurret.getPosition().x - input.getMouseX() + 0, playerTurret.getPosition().y - input.getMouseY() + 0)* -1)+180;		
				client.sendTCP(anglePck);
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
						
			if(packet instanceof Pck3_ServerMessage){
				String message = ((Pck3_ServerMessage) packet).message;
				GameController.getInstance().getConsole().addMsg(message, MsgLevel.INFO);
				Log.info(message);
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
					
			if(packet instanceof Pck100_WorldState){
				if(isConnected)
					updateClientWorld((Pck100_WorldState) packet);
			}	
			
			if(packet instanceof Pck999_PlaySound){
				GameController.getInstance().getSoundHandler().handleEvent(new GameEvent(null, ((Pck999_PlaySound)packet).sound));
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
			g.drawString("Delta: " + delta, 18, 400);
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
			if(playerTank.getShield() != null && playerTank.getShield().getHealth() <= 100){
				lifebar.render(playerTank.getHealth()/playerTank.getMaxHealth(), playerTank.getShield().getHealth()/playerTank.getMaxShieldHealth(), g);
			}else{
				lifebar.render(playerTank.getHealth()/playerTank.getMaxHealth(), 0, g);
			}
		}
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
		g.drawString("Frame: " + frameCounter, 18, 440);
		g.drawString("Updates: " + updates, 18, 420);
		g.drawString("Diff: " + (frameCounter-updates), 18, 460);
	}
    
	public String getPlayerName() {
	    return playerName;
    }
	
	@Override
    public int getID() {
	    return this.state;
    }
	
	@Override
	public void mousePressed(int button, int x, int y) {
		clientButtons.stateChanged = true;	
		clientButtons.button_LMB_pressed = true;			
	}
	
	@Override
	public void mouseReleased(int button, int x, int y) {
		clientButtons.stateChanged = true;	
		clientButtons.button_LMB_pressed = false;					
	}
		
	@Override
    public void keyPressed(int key, char c) {
		clientButtons.stateChanged = true;
		switch(key){
			case Input.KEY_W:
				clientButtons.button_W_pressed = true;
				break;
			case Input.KEY_A:
				clientButtons.button_A_pressed = true;
				break;
			case Input.KEY_S:
				clientButtons.button_S_pressed = true;
				break;
			case Input.KEY_D:
				clientButtons.button_D_pressed = true;
				break;
		}
    }

	@Override
    public void keyReleased(int key, char c) {
		clientButtons.stateChanged = true;
		switch(key){
			case Input.KEY_W:
				clientButtons.button_W_pressed = false;
				break;
			case Input.KEY_A:
				clientButtons.button_A_pressed = false;
				break;
			case Input.KEY_S:
				clientButtons.button_S_pressed = false;
				break;
			case Input.KEY_D:
				clientButtons.button_D_pressed = false;
				break;
		}
    }
	
	public static class ClientButtons{
		public boolean button_W_pressed;
		public boolean button_A_pressed;
		public boolean button_S_pressed;
		public boolean button_D_pressed;
		public boolean button_LMB_pressed;

		private boolean stateChanged;
		
		public boolean isStateChanged(){
			return stateChanged;
		}
		
		public void setStateChanged(boolean stateChanged){
			this.stateChanged = stateChanged;
		}
	}
	
	private void createClientEntity(int entityID, String identifier) {
	    if(identifier.equals("DefaultTank")){
			new DefaultTank(entityID);
		}
	    if(identifier.equals("DefaultProjectile")){
			new DefaultProjectile(entityID, null, null);
	    }
    }
}
