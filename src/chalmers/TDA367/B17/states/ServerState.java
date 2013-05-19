package chalmers.TDA367.B17.states;

import java.awt.Dimension;
import java.io.IOException;
import java.net.*;
import java.util.*;
import java.util.Map.*;
import java.util.concurrent.ConcurrentLinkedQueue;
import org.newdawn.slick.*;
import org.newdawn.slick.geom.Vector2f;
import org.newdawn.slick.gui.AbstractComponent;
import org.newdawn.slick.gui.ComponentListener;
import org.newdawn.slick.gui.TextField;
import org.newdawn.slick.state.*;
import com.esotericsoftware.kryonet.*;
import com.esotericsoftware.kryonet.Listener.ThreadedListener;
import com.esotericsoftware.minlog.Log;
import chalmers.TDA367.B17.*;
import chalmers.TDA367.B17.console.*;
import chalmers.TDA367.B17.console.Console.*;
import chalmers.TDA367.B17.controller.*;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.model.*;
import chalmers.TDA367.B17.model.Entity.RenderLayer;
import chalmers.TDA367.B17.network.Network.*;
import chalmers.TDA367.B17.network.*;
import chalmers.TDA367.B17.tanks.*;

public class ServerState extends BasicGameState {
	private int state;
	private static ServerState instance;
	private GameController controller;
	private ConcurrentLinkedQueue<Packet> packetQueue;
	
	private Server server;
	private String ipAddress;
	private ArrayList<Packet> allClientsPacketQueue;
	private ArrayList<Packet> clientPacketQueue;
	private ArrayList<Player> players;
	private ArrayList<Player> disconnectedPlayersTemp;
	
	private boolean gameStarted = false;
	private long deltaTime;
	private int frameCounter;
	private int updates;
	private long oldTime;
	private int updatesPerSecond;
	private int packetsSent;
	private int packetsSentPerSecond;
	private int packetsRecPerSecond;
	private int packetsReceived;
	private TextField chatField;
	private boolean serverStarted;
	
	private ServerState(int state) {
	    this.state = state;
		controller = GameController.getInstance();
    }
	
	public static ServerState getInstance(){
		if(instance == null)
			instance = new ServerState(Tansk.SERVER);
	
		return instance;
	}
	
	@Override
    public void init(GameContainer gc, StateBasedGame game) throws SlickException {
		gc.setSmoothDeltas(true);
		gc.setAlwaysRender(true);
		gc.setMouseCursor(new Image("data/crosshair.png"), 16, 16);

		allClientsPacketQueue = new ArrayList<Network.Packet>();
		clientPacketQueue = new ArrayList<Packet>();
		disconnectedPlayersTemp = new ArrayList<Player>();
		players = new ArrayList<Player>();
		ipAddress = "N/A";
    }
	
	@Override
	public void enter(GameContainer container, StateBasedGame game) throws SlickException {
		super.enter(container, game);

		controller.setConsole(new Console(10, 533, 450, 192, OutputLevel.ALL));
		chatField = new TextField(container, container.getDefaultFont(), 10, 733, 450, 23, new ComponentListener() {
			public void componentActivated(AbstractComponent source) {
				if(!chatField.getText().equals("")){
					if(server != null){
						String msg = "Server: " + chatField.getText();
						Pck3_Message pck = new Pck3_Message();
						pck.message = msg;
						GameController.getInstance().getConsole().addMsg(msg, MsgLevel.INFO);
						chatField.setText("");
						addToAllClientsQueue(pck);
					}
				}
			}
		});

		controller.setWorld(new World(new Dimension(Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT), true));
		controller.getWorld().init();
//		controller.newGame(Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT, 10, 4, 1, 5000, 500000, 1500000, true);
		
		MapLoader.createEntities("whatever");
		Log.set(Log.LEVEL_INFO);
		packetQueue = new ConcurrentLinkedQueue<Packet>();
		server = new Server();
		Network.register(server);
		Listener listener = new Listener(){
				public void received(Connection con, Object msg){
					super.received(con, msg);
					if (msg instanceof Packet) {
						packetsReceived++;	
						Packet packet = (Packet)msg;
						packet.setConnection(con);
						packetQueue.add(packet);
					}
				}
				
				@Override
				public void connected(Connection connection) {
//					GameController.getInstance().getConsole().addMsg("Connected.", MsgLevel.INFO);
				}
				
				@Override
				public void disconnected(Connection connection) {
					if(getPlayer(connection) != null){
						disconnectedPlayersTemp.add(getPlayer(connection));
					} else {
						GameController.getInstance().getConsole().addMsg("Player disconnected.", MsgLevel.INFO);
						Log.info("[SERVER] Unkown player has  disconnected.");
					}
				}
			};
			
			ThreadedListener threadListener = new ThreadedListener(listener);
			server.addListener(threadListener);
	
		try {
	        server.bind(Network.PORT, Network.PORT);
			server.start();	
			controller.getConsole().addMsg("Server loaded and ready!", MsgLevel.INFO);
			serverStarted = true;
        } catch (IOException e) {
    		controller.getConsole().addMsg("Address in use! Sever closed.", MsgLevel.ERROR);
    		controller.getConsole().addMsg("Another server running?", MsgLevel.INFO);
    		controller.getConsole().addMsg("Sever closed.", MsgLevel.ERROR);
	        e.printStackTrace();
        }	
		try {
			ipAddress = InetAddress.getLocalHost().getHostAddress();
        } catch (UnknownHostException e1) {
	        e1.printStackTrace();
        }
	}
	
	// only used for debug
	private void startGame() {
		gameStarted = true;
		int x = 100;
		int y = 100;
		for(int i = 0; i < getPlayers().size(); i++){
			AbstractTank tank = new DefaultTank(GameController.getInstance().generateID(), new Vector2f(0,-1), getPlayers().get(i));
			if(i > 7)
				y+=100;
			tank.setPosition(new Vector2f(x, y));
			getPlayers().get(i).setTank(tank);
	    	
			Pck7_TankID tankPck = new Pck7_TankID();
	    	tankPck.tankID = getPlayers().get(i).getTank().getId();
	    	getPlayers().get(i).getConnection().sendTCP(tankPck);
	    	addToClientQueue(tankPck, getPlayers().get(i).getConnection());
	    	
			x+=100;
		}
		// Start a new round
//		gameConditions.init(10, 2, 1, 5000, 500000, 1500000);
//		gameConditions.newRoundDelayTimer(3000);
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
				
		if(!gameStarted && serverStarted){
			Input input = gc.getInput();
			int x = input.getMouseX();
			int y = input.getMouseY();
			if(x > 410 && x < 465 && y > 495 && y < 525){
				if(input.isMousePressed(0)){
					startGame();
					GameController.getInstance().getConsole().addMsg("Game started!", MsgLevel.INFO);
				}
			}
		}
		
		processPackets();
		checkDisconnectedPlayers();
		if(gameStarted){
			// Update for tankspawner & gameconditions
//			controller.getWorld().getTankSpawner().update(delta, playerList);
//			gameConditions.update(delta);
			updatePlayerTanks(delta);
			updateWorld(delta);
			createWorldState();
		}
		updateClients();
    }

	public void checkDisconnectedPlayers(){
		for(Player lostPlayer : disconnectedPlayersTemp){
			if(lostPlayer != null){
		    	String msg = lostPlayer.getName() + " disconnected.";
				controller.getConsole().addMsg(msg, MsgLevel.INFO);
		    	Pck3_Message disconnectedMsg = new Pck3_Message();
		    	disconnectedMsg.message = msg;
		    	server.sendToAllExceptTCP(lostPlayer.getConnection().getID(), disconnectedMsg);
		    	getPlayers().remove(lostPlayer);
				
		    	Log.info("[SERVER] " + msg);
			}
		}
		disconnectedPlayersTemp = new ArrayList<Player>();
	}
	
	public void processPackets() {
		Packet packet;
		while ((packet = packetQueue.poll()) != null) {
			if(packet instanceof Pck0_JoinRequest){
		    	Pck0_JoinRequest pck = (Pck0_JoinRequest) packet;
		    	
		    	controller.getConsole().addMsg(pck.playerName + " attempting to connect..", MsgLevel.INFO);
		    	Pck1_LoginAnswer responsePacket = new Pck1_LoginAnswer();
		    	
		    	if(!gameStarted){
			    	Player newPlayer = new Player(packet.getConnection(), pck.playerName);
			    	newPlayer.setLives(GameController.getInstance().getGameConditions().getPlayerLives());
			    	newPlayer.setRespawnTime(GameController.getInstance().getGameConditions().getSpawnTime());
			    	getPlayers().add(newPlayer);
			    	responsePacket.accepted = true;
		    	} else {
		    		responsePacket.accepted = false;
		    		responsePacket.reason = "Game started.";
		    		GameController.getInstance().getConsole().addMsg(pck.playerName + " kicked, game has started.", MsgLevel.INFO);
		    	}
			   	packet.getConnection().sendTCP(responsePacket);
		    }
		    
		    if(packet instanceof Pck2_ClientConfirmJoin){
		    	String msg = getPlayer(packet.getConnection()).getName() + " joined.";
				controller.getConsole().addMsg(msg, MsgLevel.INFO);
		    	Pck3_Message welcomeMsg = new Pck3_Message();
		    	welcomeMsg.message = msg;
		    	server.sendToAllExceptTCP(packet.getConnection().getID(), welcomeMsg);
				Log.info(msg);
		    }
		    
		    if(packet instanceof Pck4_ClientInput){
		    	if(gameStarted)
		    		receiveClientInput((Pck4_ClientInput) packet);
 		    }
		    
		    if(packet instanceof Pck3_Message){
			    if(packet instanceof Pck31_ChatMessage){
			    	packet.setConnection(null);
			    	GameController.getInstance().getConsole().addMsg(((Pck31_ChatMessage)packet).message);
					addToAllClientsQueue(packet);
			    }
 		    }
	   	}
    }

	private void receiveClientInput(Pck4_ClientInput pck) {
		Player playerToUpdate = getPlayer(pck.getConnection());
		playerToUpdate.setInputStatus(Player.INPT_W, pck.W_pressed);
		playerToUpdate.setInputStatus(Player.INPT_A, pck.A_pressed);
		playerToUpdate.setInputStatus(Player.INPT_S, pck.S_pressed);
		playerToUpdate.setInputStatus(Player.INPT_D, pck.D_pressed);
		playerToUpdate.setInputStatus(Player.INPT_LMB, pck.LMB_pressed);
		if(playerToUpdate.getTank() != null)
			playerToUpdate.getTank().getTurret().setRotation(pck.turretNewAngle);
    }
	
	public void updatePlayerTanks(int delta){
		for(Player player : getPlayers()){		
			ArrayList<Boolean> pressedKeys = player.getInput();
			if(player.getTank() != null){
				if(pressedKeys.get(Player.INPT_W)){
					player.getTank().accelerate(delta);
				} else if (pressedKeys.get(Player.INPT_S)){
					player.getTank().reverse(delta);
				} else {
					player.getTank().friction(delta);
				}
				
				if(pressedKeys.get(Player.INPT_A) && !pressedKeys.get(Player.INPT_D)){
					if(pressedKeys.get(Player.INPT_S)){
						player.getTank().turnRight(delta);
					} else {
						player.getTank().turnLeft(delta);
					}
				}
				
				if(pressedKeys.get(Player.INPT_D) && !pressedKeys.get(Player.INPT_A)){
					if(pressedKeys.get(Player.INPT_S)){
						player.getTank().turnLeft(delta);
					} else {
						player.getTank().turnRight(delta);
					}
				}
				
				if(pressedKeys.get(Player.INPT_LMB)){
					player.getTank().fireWeapon(delta);
				}	
			}
		}
	}
	
	public void updateWorld(int delta){
		Dimension worldSize = GameController.getInstance().getWorld().getSize();
		
		Iterator<Entry<Integer, Entity>> updateIterator = controller.getWorld().getEntities().entrySet().iterator();
		while(updateIterator.hasNext()){
			Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) updateIterator.next();
			Entity entity = entry.getValue();
			
			entity.update(delta);
			
			if(entity instanceof MovableEntity){
				float x = entity.getPosition().getX();
				float y = entity.getPosition().getY();
				
				if((x < 0) || (x > worldSize.width) || (y < 0) || (y > worldSize.height)){
					entity.destroy();
				} else {
					controller.getWorld().checkCollisionsFor((MovableEntity)entity);
				}
			}
			if(entity instanceof AbstractSpawnPoint)
				controller.getWorld().checkCollisionsFor(entity);
		}
	}
	
	private void createWorldState(){
		Pck100_WorldState worldState = new Pck100_WorldState();
		worldState.updatePackets = new ArrayList<EntityPacket>();
		
		Iterator<Entry<Integer, Entity>> updateIterator = controller.getWorld().getEntities().entrySet().iterator();
		while(updateIterator.hasNext()){
			Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) updateIterator.next();
			Entity entity = entry.getValue();

			if(entity instanceof MovableEntity){
				if(entity instanceof AbstractTank){
					AbstractTank tank = (AbstractTank) entity;
					Pck101_TankUpdate pck = new Pck101_TankUpdate();
					pck.entityID = tank.getId();
					pck.tankPosition = tank.getPosition();
					pck.tankDirection = tank.getDirection();
					pck.tankHealth = tank.getHealth();
					if(tank.getShield() != null)
						pck.tankShieldHealth = tank.getShield().getHealth();
					pck.turretPosition = tank.getTurret().getPosition();
					pck.turretAngle = (float) tank.getTurret().getRotation();
					worldState.updatePackets.add(pck);
				}
				
				if(entity instanceof AbstractProjectile){
					AbstractProjectile proj = (AbstractProjectile) entity;
					Pck102_ProjectileUpdate pck = new Pck102_ProjectileUpdate();
					pck.entityID = proj.getId();
					pck.projPosition = proj.getPosition();
					pck.projDirection = proj.getDirection();
					worldState.updatePackets.add(pck);
				}
			}
		}			
		addToAllClientsQueue(worldState);
	}
		
	public void addToAllClientsQueue(Packet packet){
		allClientsPacketQueue.add(packet);
	}
	
	public void addToClientQueue(Packet packet, Connection clientConnection){
		packet.setConnection(clientConnection);
		clientPacketQueue.add(packet);
	}
	
	private void updateClients(){
		if(server != null){
			for(Packet packet : allClientsPacketQueue){
				server.sendToAllTCP(packet);
				packetsSent++;
			}
			for(Packet clientPacket : clientPacketQueue){
				if(clientPacket != null){
					Connection con = clientPacket.getConnection();
					clientPacket.setConnection(null);
					server.sendToTCP(con.getID(), clientPacket);
					packetsSent++;
				}
			}
		}
		updates++;
		// empty packet "buffers"
		allClientsPacketQueue = new ArrayList<Network.Packet>();
		clientPacketQueue = new ArrayList<Network.Packet>();
	}
	
	@Override
    public void render(GameContainer container, StateBasedGame game, Graphics g) throws SlickException {
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
			renderEntities(firstLayerEnts, g);
			renderEntities(secondLayerEnts, g);
			renderEntities(thirdLayerEnts, g);
			renderEntities(fourthLayerEnts, g);
		}
		
		if(!getPlayers().isEmpty()){
			Player playerOne = getPlayers().get(0);
			if(playerOne.getTank() != null){
				AbstractTank playerOneTank = playerOne.getTank();
				g.setColor(Color.yellow);
				g.drawLine(playerOneTank.getSpritePosition().x, playerOneTank.getSpritePosition().y, Tansk.SCREEN_WIDTH/2, Tansk.SCREEN_HEIGHT/2);
//				g.setColor(Color.red);
//				g.drawLine(playerOneTank.getPosition().x, playerOneTank.getPosition().y, Tansk.SCREEN_WIDTH/2, Tansk.SCREEN_HEIGHT/2);
//				g.setColor(Color.blue);
//				g.drawLine(playerOneTank.getTurret().getPosition().x, playerOneTank.getTurret().getPosition().y, Tansk.SCREEN_WIDTH/2, Tansk.SCREEN_HEIGHT/2);
			}
		}
		
		controller.getConsole().renderMessages(g);
		
		
		if(gameStarted)
			g.setColor(Color.green);
		else
			g.setColor(Color.red);
		g.drawString("Run!", 415, 500);
		g.drawRect(410, 495, 50, 30);
		g.setColor(Color.white);
		g.drawString("Packet rec/sec: " + packetsRecPerSecond, 18, 320);
		g.drawString("Packet sent/sec: " + packetsSentPerSecond, 18, 340);
		g.drawString("Update/sec: " + updatesPerSecond, 18, 360);
		g.drawString("Frame: " + frameCounter, 18, 400);
		g.drawString("Entities: " + controller.getWorld().getEntities().size(), 18, 420);
		g.drawString("Players: " + getPlayers().size(), 18, 480);
		g.drawString("LAN IP: " + ipAddress, 18, 500);
		
		g.setColor(Color.white);
		g.setLineWidth(1);
		g.drawRect(chatField.getX(), chatField.getY(), chatField.getWidth(), chatField.getHeight());
		chatField.render(container, g);
    }
	
	private void renderEntities(ArrayList<Entity> entities, Graphics g){
		g.setColor(Color.yellow);
		for(Entity entity : entities){
			if(entity instanceof AbstractTurret){
				g.drawOval(entity.getPosition().x-10, entity.getPosition().y-10, 20, 20);
				g.drawLine(entity.getPosition().x, entity.getPosition().y, ((AbstractTurret)entity).getTurretNozzle().x, ((AbstractTurret)entity).getTurretNozzle().y);
			} else {
				g.draw(entity.getShape());
			}
		}
		g.setColor(Color.white);
	}
	
	@Override
    public int getID() {
	    return this.state;
    }
		
	public Player getPlayer(Connection con){
		for(Player player : getPlayers()){
			if(player.getConnection().getID() == con.getID()){
				return player;
			}
		}
		return null;
	}
	
	public ArrayList<Player> getPlayers(){
//		return controller.getGameConditions().getPlayerList();
		return players;
	}
		
	public void sendEvent(GameEvent event){
		Pck1000_GameEvent eventPck = new Pck1000_GameEvent();
		eventPck.eventType = event.getEventType().ordinal();
		eventPck.eventDesc = event.getEventDesc();
		eventPck.sourceID = event.getSourceID();
		addToAllClientsQueue(eventPck);
	} 
}