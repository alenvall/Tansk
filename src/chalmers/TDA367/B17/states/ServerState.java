package chalmers.TDA367.B17.states;

import java.awt.*;
import java.io.*;
import java.net.*;
import java.util.*;
import java.util.Map.Entry;

import org.newdawn.slick.*;
import org.newdawn.slick.Color;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.geom.Vector2f;
import org.newdawn.slick.gui.TextField;
import org.newdawn.slick.state.*;

import chalmers.TDA367.B17.*;
import chalmers.TDA367.B17.console.Console;
import chalmers.TDA367.B17.console.Console.*;
import chalmers.TDA367.B17.controller.*;
import chalmers.TDA367.B17.event.*;
import chalmers.TDA367.B17.gamemodes.KingOfTheHillMode;
import chalmers.TDA367.B17.model.*;
import chalmers.TDA367.B17.model.Entity.RenderLayer;
import chalmers.TDA367.B17.network.*;
import chalmers.TDA367.B17.network.Network.*;

import com.esotericsoftware.kryonet.*;
import com.esotericsoftware.kryonet.Listener.*;
import com.esotericsoftware.minlog.*;

public class ServerState extends TanskState {
	private static ServerState instance;

	private Server server;
	private String ipAddress;
	private ArrayList<Packet> allClientsPacketQueue;
	private ArrayList<Packet> clientPacketQueue;
	private ArrayList<Player> disconnectedPlayersTemp;

	protected TextField chatField;
	private boolean gameStarted = false;
	private boolean serverStarted;
	
	private ServerState(int state) {
		super(state);
    }
	
	public static ServerState getInstance(){
		if(instance == null)
			instance = new ServerState(Tansk.SERVER);
	
		return instance;
	}
	
	@Override
    public void init(GameContainer gc, StateBasedGame game) throws SlickException {
		super.init(gc, game);

		allClientsPacketQueue = new ArrayList<Network.Packet>();
		clientPacketQueue = new ArrayList<Packet>();
		disconnectedPlayersTemp = new ArrayList<Player>();
		ipAddress = "N/A";
    }
	
	@Override
	public void enter(GameContainer container, StateBasedGame game) throws SlickException {
		super.enter(container, game);
		
		controller.setConsole(new Console(10, 533, 600, 192, OutputLevel.ALL));
		controller.setWorld(new World(new Dimension(Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT), true));
		controller.getWorld().init();

		controller.newGame();
		chatField = new TextField(container, container.getDefaultFont(), 10, 733, 450, 23);
		
		MapLoader.createEntities("standard");
		controller.getSoundHandler().stopAllMusic();
		
		Log.set(Log.LEVEL_INFO);
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
    		controller.getConsole().addMsg("Server closed.", MsgLevel.ERROR);
	        e.printStackTrace();
        }	
		try {
			ipAddress = InetAddress.getLocalHost().getHostAddress();
        } catch (UnknownHostException e1) {
	        e1.printStackTrace();
        }
	}
	
	@Override
	public void leave(GameContainer gc, StateBasedGame sbg) throws SlickException {
		super.leave(gc, sbg);
		server.close();
	}	
		
	@Override
    public void update(GameContainer gc, StateBasedGame game, int delta) throws SlickException {
		super.update(gc, game, delta);
		
		if(gc.getInput().isKeyDown(Input.KEY_ESCAPE)){
			game.enterState(Tansk.MENU);
		}
		
		if(!gameStarted && serverStarted){
			Input input = gc.getInput();
			int x = input.getMouseX();
			int y = input.getMouseY();
			if(x > 410 && x < 465 && y > 495 && y < 525){
				if(input.isMousePressed(0)){
					gameStarted = true;

					//Start a new round
					if(controller.getGameMode() instanceof KingOfTheHillMode)
						((KingOfTheHillMode)controller.getGameMode()).generateZone(new Vector2f(512, 384));
					controller.getGameMode().newRoundDelayTimer(3000);
					GameController.getInstance().getConsole().addMsg("Game started!", MsgLevel.INFO);
				}
			}
		}

		GameController.getInstance().getConsole().update(delta);
		
		processPackets();
		checkDisconnectedPlayers();
		if(gameStarted){		
			//Update for tankspawner
			controller.getWorld().getTankSpawner().update(delta);
			
			controller.getWorld().getSpawner().update(delta);
			
			//Update for getGameMode()
			controller.getGameMode().update(delta);

			updatePlayerTanks(delta);
			updateWorld(delta);
			createWorldState();
		}
		updateClients();
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
		renderGUI(container, g);
	}
		
	@Override
	public void renderGUI(GameContainer gc, Graphics g){
		super.renderGUI(gc, g);

		g.drawRect(chatField.getX(), chatField.getY(), chatField.getWidth(), chatField.getHeight());
		chatField.render(gc, g);
		if(gameStarted)
			
			g.setColor(Color.green);
		else
			g.setColor(Color.red);
		g.drawString("Run!", 415, 500);
		g.drawRect(410, 495, 50, 30);

		g.setColor(Color.white);
		if(getPlayers() != null)
			g.drawString("Players: " + getPlayers().size() + "/4", 18, 480);
		g.drawString("LAN IP: " + ipAddress, 18, 500);
		
		// some debug stuff
//		if(!getPlayers().isEmpty()){
//			Player playerOne = getPlayers().get(0);
//			if(playerOne.getTank() != null){
//				AbstractTank playerOneTank = playerOne.getTank();
//				g.setColor(Color.yellow);
//				g.drawLine(playerOneTank.getSpritePosition().x, playerOneTank.getSpritePosition().y, Tansk.SCREEN_WIDTH/2, Tansk.SCREEN_HEIGHT/2);
//				g.setColor(Color.red);
//				g.drawLine(playerOneTank.getPosition().x, playerOneTank.getPosition().y, Tansk.SCREEN_WIDTH/2, Tansk.SCREEN_HEIGHT/2);
//				g.setColor(Color.blue);
//				g.drawLine(playerOneTank.getTurret().getPosition().x, playerOneTank.getTurret().getPosition().y, Tansk.SCREEN_WIDTH/2, Tansk.SCREEN_HEIGHT/2);
//			}
//		}
		
			
//		TODO: not do this in render maybe
		//Cool timer
		if(gameStarted){
			if(controller.getGameMode().isDelaying()){
				if(controller.getGameMode().getDelayTimer() > 0)
//					serverMessage("Round starts in: " + (controller.getGameMode().getDelayTimer()/1000 + 1) + " seconds!");
					g.drawString("Round starts in: " + (controller.getGameMode().getDelayTimer()/1000 + 1) + " seconds!", 500, 400);
			}
		}
		
//		if(controller.getGameMode().isGameOver()){
//			serverMessage("Game Over!");
//			g.drawString("Game Over!", 500, 300);
//			serverMessage("Winner: " + controller.getGameMode().getWinningPlayer().getName());
//			for(Player p: controller.getGameMode().getWinningPlayers()){
//				g.drawString("Winner: " + p.getName(), 500, 400 + 10*controller.getGameMode().getWinningPlayers().indexOf(p));
//			}
//			int i = 0;
//			for(Player p : controller.getGameMode().getPlayerList()){
//				i++;
//				g.drawString(p.getName() + "'s score: " + p.getScore(), 500, (450+(i*25)));
//				serverMessage(p.getName() + "'s score: " + p.getScore());
//			}
//		}
	
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
	
	public void checkDisconnectedPlayers(){
		for(Player lostPlayer : disconnectedPlayersTemp){
			if(lostPlayer != null){
		    	String msg = lostPlayer.getName() + " disconnected.";
				controller.getConsole().addMsg(msg, MsgLevel.INFO);
		    	Pck3_Message disconnectedMsg = new Pck3_Message();
		    	disconnectedMsg.message = msg;
		    	server.sendToAllExceptTCP(lostPlayer.getConnection().getID(), disconnectedMsg);
		    	controller.getGameMode().removePlayer(lostPlayer);
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
		    	
	    		if(getPlayers().size() < 4){
			    	if(!gameStarted){
			    		createPlayer(pck.playerName, packet.getConnection());
		    			responsePacket.accepted = true;
			    	} else {
			    		responsePacket.accepted = false;
			    		responsePacket.reason = "Game started.";
			    		GameController.getInstance().getConsole().addMsg(pck.playerName + " kicked, game has started.", MsgLevel.INFO); 	
			    	}
	    		} else {
	    			responsePacket.accepted = false;
	    			responsePacket.reason = "Server full.";
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

	private void createPlayer(String playerName, Connection connection) {
		if(playerName.equals("Unnamed")){
			String newName = "Unnamed" + Math.round(Math.random() * 1000);
			serverMessage(playerName + " renamed to " + newName + ".");
			playerName = newName;
		}
		
		for(Player player : getPlayers()){
			if(player.getName().equals(playerName)){
				playerName = "Unnamed" + Math.round(Math.random() * 1000);
				serverMessage(player.getName() + " renamed to " + playerName + ".");
			}
		}
		
		Player newPlayer = new Player(connection, playerName);
		newPlayer.setLives(GameController.getInstance().getGameMode().getPlayerLives());
		newPlayer.setRespawnTime(GameController.getInstance().getGameMode().getSpawnTime());
		int numberOfPlayers = getPlayers().size();
		if(numberOfPlayers == 0){
			newPlayer.setColor("blue");
		} else if (numberOfPlayers == 1) {
			newPlayer.setColor("red");
		} else if (numberOfPlayers == 2) {
			newPlayer.setColor("yellow");
		} else if (numberOfPlayers == 3) {
			newPlayer.setColor("white");
		}
		getPlayers().add(newPlayer);
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
					player.getTank().fireTurret(delta);
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
					Pck102_TankUpdate pck = new Pck102_TankUpdate();
					pck.entityID = tank.getId();
					pck.tankPosition = tank.getPosition();
					pck.tankDirection = tank.getDirection();
					pck.tankHealth = tank.getHealth();
					if(tank.getShield() != null){
						pck.tankShieldHealth = tank.getShield().getHealth();
						pck.shieldPosition = tank.getShield().getPosition();
					}
					pck.turretPosition = tank.getTurret().getPosition();
					pck.turretAngle = (float) tank.getTurret().getRotation();
					worldState.updatePackets.add(pck);
				}
				
				if(entity instanceof AbstractProjectile){
					AbstractProjectile proj = (AbstractProjectile) entity;
					Pck103_ProjectileUpdate pck = new Pck103_ProjectileUpdate();
					pck.entityID = proj.getId();
					pck.projPosition = proj.getPosition();
					pck.projDirection = proj.getDirection();
					worldState.updatePackets.add(pck);
				}
			}
		}			
		addToAllClientsQueue(worldState);
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
		// empty packet "buffers"
		allClientsPacketQueue = new ArrayList<Network.Packet>();
		clientPacketQueue = new ArrayList<Network.Packet>();
	}
	
	public void addToAllClientsQueue(Packet packet){
		allClientsPacketQueue.add(packet);
	}
		
	public void addToClientQueue(Packet packet, Connection clientConnection){
		packet.setConnection(clientConnection);
		clientPacketQueue.add(packet);
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
		if(controller.getGameMode() != null)
			return controller.getGameMode().getPlayerList();
		else
			return null;
	}
	
	public void serverMessage(String message){
		if(server != null){		
			message = message.replace('\n', ' ');
			if(message.length() > 39)
				message = message.substring(0, 38);
			Pck3_Message pck = new Pck3_Message();
			pck.message = message;
			GameController.getInstance().getConsole().addMsg(message, MsgLevel.INFO);
			addToAllClientsQueue(pck);
		}
	}
		
	public void sendEvent(GameEvent event){
		Pck1000_GameEvent eventPck = new Pck1000_GameEvent();
		eventPck.eventType = event.getEventType().ordinal();
		eventPck.eventDesc = event.getEventDesc();
		eventPck.sourceID = event.getSourceID();
		addToAllClientsQueue(eventPck);
	} 
	
	@Override
	public void keyReleased(int key, char c){
		super.keyReleased(key, c);
		if(key == Input.KEY_ENTER){
			if(chatField.hasFocus()){
				if(!chatField.getText().equals("")){
					if(server != null){
						if((chatField.getText().charAt(0) + "").equals("/")){
							if(chatField.getText().length() >= 4){
								if(chatField.getText().substring(0, 3).equals("/k "))
									doCommand("kick");
							}
						} else {
							serverMessage("Server: " + chatField.getText());
						}
						chatField.setText("");
					}
				}
				chatField.setFocus(false);
			} else {
				chatField.setFocus(true);
			}	
		}
	}
	
	private void doCommand(String cmd){
		if(cmd.equals("kick")){
			if(getPlayers().size() > 0){
				Player matchedPlayer = null;
				String playerName = chatField.getText().substring(3).trim();
				for(Player player : getPlayers()){
					if(player.getName().equals(playerName))
						matchedPlayer = player;
					else
						GameController.getInstance().getConsole().addMsg("Player: " + playerName + " not found.");
				}
				if(matchedPlayer != null){
					Pck5_PlayerKicked kickPck = new Pck5_PlayerKicked();
					kickPck.reason = "";
					matchedPlayer.getConnection().sendTCP(kickPck);
					GameController.getInstance().getConsole().addMsg("Kicked " + matchedPlayer.getName() + ".");
					serverMessage(matchedPlayer.getName() + " kicked.");
				}
			} else {
				GameController.getInstance().getConsole().addMsg("No players to kick!");
			}
		}
	}
}