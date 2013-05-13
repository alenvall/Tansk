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
import chalmers.TDA367.B17.model.*;
import chalmers.TDA367.B17.model.Entity.*;
import chalmers.TDA367.B17.network.*;
import chalmers.TDA367.B17.network.Network.*;
import chalmers.TDA367.B17.tanks.DefaultTank;

public class ClientState extends BasicGameState {
	private int state;
	private static ClientState instance;
	private GameController controller;
	private ImageHandler imgHandler;
	private ConcurrentLinkedQueue<Packet> packetQueue;
	
	private Client client;
	private boolean isConnected;
	private Input input;
	private Image map = null;
	private SpriteSheet entSprite;
	private String playerName;
	private int currentTankID;
		
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
		imgHandler = new ImageHandler();
		controller.setWorld(new World(new Dimension(Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT), true));
		
		map = new Image("data/map.png");
		playerName = "Nisse" + Math.round(Math.random() * 1000);
    }
	
	@Override
	public void enter(GameContainer gc, StateBasedGame game) throws SlickException {
		super.enter(gc, game);
		
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
		imgHandler.loadAllImages(Tansk.DATA_FOLDER);
		MapLoader.createEntities("whatever");

		input = gc.getInput();   
		input.addKeyListener(this);
		input.addMouseListener(this);
	}	
	
	@Override
    public void update(GameContainer gc, StateBasedGame game, int delta) throws SlickException {
		processPackets();
		
		sendClientInput(gc.getInput());
    }
	
	private void sendClientInput(Input input) {
		if(isConnected){
			Pck4_ClientInput pck = new Pck4_ClientInput();
			if(input.isKeyDown(Input.KEY_W))
				pck.W_pressed = true;
			else
				pck.W_pressed = false;	
			
			if(input.isKeyDown(Input.KEY_A))
				pck.A_pressed = true;
			else
				pck.A_pressed = false;	
			
			if(input.isKeyDown(Input.KEY_S))
				pck.S_pressed = true;
			else
				pck.S_pressed = false;
			
			if(input.isKeyDown(Input.KEY_D))
				pck.D_pressed = true;
			else
				pck.D_pressed = false;		
			
			if(input.isMouseButtonDown(0))
				pck.LMB_pressed = true;
			else
				pck.LMB_pressed = false;
	
			if(((AbstractTank) controller.getWorld().getEntity(currentTankID)) != null){
				AbstractTurret playerTurret = ((AbstractTank) controller.getWorld().getEntity(currentTankID)).getTurret();
				pck.turretNewAngle = (float) Math.toDegrees(Math.atan2(playerTurret.getPosition().x - input.getMouseX() + 0, playerTurret.getPosition().y - input.getMouseY() + 0)* -1)+180;		
			}
			client.sendTCP(pck);
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
			
			if(packet instanceof Pck6_CreateTank){
				Pck6_CreateTank pck = (Pck6_CreateTank) packet;
				new DefaultTank(pck.id, pck.velocity);
			}
			
			if(packet instanceof Pck7_TankID){
				Pck7_TankID pck = (Pck7_TankID) packet;
				currentTankID = pck.tankID;
			}
			
//			if(packet instanceof Pck8_TurretAngleUpdate){
//				Pck8_TurretAngleUpdate pck = (Pck8_TurretAngleUpdate) packet;
//				for(int i = 0; i < pck.turretIDs.size(); i++){
//					GameController.getInstance().getConsole().addMsg(Double.toString(pck.turretAngles.get(i)));
//					((AbstractTurret)controller.getWorld().getEntity(pck.turretIDs.get(i))).setRotation(pck.turretAngles.get(i));
//				}
//			}
			
//			if(packet instanceof Pck101_TankUpdate){
//				Pck101_TankUpdate pck = (Pck101_TankUpdate) packet;
//				((AbstractTank)controller.getWorld().getEntity(pck.entityID)).setPosition(pck.position);
//				((AbstractTank)controller.getWorld().getEntity(pck.entityID)).setDirection(pck.direction);
//			}
			
//			if(packet instanceof Pck102_TurretUpdate){
//				Pck102_TurretUpdate pck = (Pck102_TurretUpdate) packet;
//				((AbstractTurret)controller.getWorld().getEntity(pck.entityID)).setPosition(pck.position);
//			}
			
			if(packet instanceof Pck100_WorldState){
				updateClientWorld((Pck100_WorldState) packet);
			}
		}
    }			

	private void updateClientWorld(Pck100_WorldState worldState) {
		for(EntityPacket pck : worldState.updatePackets){
			if(pck instanceof Pck101_TankUpdate){
				Pck101_TankUpdate packet = (Pck101_TankUpdate) pck;
				AbstractTank tank = (AbstractTank) controller.getWorld().getEntity(packet.entityID);
				tank.setPosition(packet.tankPosition);
				tank.setDirection(packet.tankDirection);
				AbstractTurret turret = tank.getTurret();
				turret.setPosition(packet.turretPosition);
				turret.setRotation(packet.turretAngle);
			}

//			if(pck instanceof Pck102_TurretUpdate){
//				AbstractTurret turret = (AbstractTurret) controller.getWorld().getEntity(pck.entityID);
//				turret.setPosition(((Pck102_TurretUpdate)pck).position);
//				turret.setRotation(((Pck102_TurretUpdate)pck).angle);
//			}
			
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
    }
	
	private void renderEntities(ArrayList<Entity> entities){
		for(Entity entity : entities){
			entSprite = imgHandler.getSprite(entity.getSpriteID());
			
			if(entSprite != null){
				if(entity instanceof AbstractTank){
					entSprite = imgHandler.getSprite(entity.getSpriteID());
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
		g.drawString("Entities: " + controller.getWorld().getEntities().size(), 18, 545);
	}

//	@Override
//    public void mousePressed(int button, int x, int y) {
//		if(button == 0){
//			Pck4_ClientInput input = new Pck4_ClientInput();
//			input.pressed = true;
//			input.keyCode = Input.MOUSE_LEFT_BUTTON;
//			client.sendTCP(input);
//		}
//	}

//	@Override
//    public void mouseReleased(int button, int x, int y) {
//		if(button == 0){
//			Pck4_ClientInput input = new Pck4_ClientInput();
//			input.pressed = false;
//			input.keyCode = Input.MOUSE_LEFT_BUTTON;
//			client.sendTCP(input);
//		}
//	}

//	@Override
//    public void mouseMoved(int oldx, int oldy, int newx, int newy) {
//		Pck5_ClientTurretAngle input = new Pck5_ClientTurretAngle();
//		if(((AbstractTank) controller.getWorld().getEntity(currentTankID)) != null){
//			AbstractTurret playerTurret = ((AbstractTank) controller.getWorld().getEntity(currentTankID)).getTurret();
//			input.angle = (float) Math.toDegrees(Math.atan2(playerTurret.getPosition().x - newx + 0, playerTurret.getPosition().y - newy + 0)* -1)+180;
//		
//			client.sendTCP(input);
//		}
//	}

//	@Override
//    public void mouseDragged(int oldx, int oldy, int newx, int newy) {
//		Pck5_ClientTurretAngle input = new Pck5_ClientTurretAngle();
//		if(((AbstractTank) controller.getWorld().getEntity(currentTankID)) != null){
//			AbstractTurret playerTurret = ((AbstractTank) controller.getWorld().getEntity(currentTankID)).getTurret();
//			input.angle = (float) Math.toDegrees(Math.atan2(playerTurret.getPosition().x - newx + 0, playerTurret.getPosition().y - newy + 0)* -1)+180;
//		
//			client.sendTCP(input);
//		}
//    }

//	@Override
//    public void keyPressed(int key, char c) {
//		if((key == Input.KEY_W) || (key == Input.KEY_A) || (key == Input.KEY_S) || (key == Input.KEY_D))
//			sendKey(key, true);
//    }
//
//	@Override
//    public void keyReleased(int key, char c) {
//		if((key == Input.KEY_W) || (key == Input.KEY_A) || (key == Input.KEY_S) || (key == Input.KEY_D))
//			sendKey(key, false); 
//    }
//
//    private void sendKey(int keyCode, boolean pressed) {
//    	Pck4_ClientInput input = new Pck4_ClientInput();
//	   	input.pressed = pressed;
//	   	input.keyCode = keyCode;
//	   	client.sendTCP(input);
//    }
    
	public String getPlayerName() {
	    return playerName;
    }
	
	@Override
    public int getID() {
	    return this.state;
    }
}
