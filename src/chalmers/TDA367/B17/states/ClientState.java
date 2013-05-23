package chalmers.TDA367.B17.states;

import java.awt.Dimension;
import java.io.IOException;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.Map;
import java.util.Map.Entry;

import org.newdawn.slick.Color;
import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Image;
import org.newdawn.slick.Input;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.SpriteSheet;
import org.newdawn.slick.geom.Shape;
import org.newdawn.slick.geom.Transform;
import org.newdawn.slick.geom.Vector2f;
import org.newdawn.slick.gui.TextField;
import org.newdawn.slick.state.StateBasedGame;

import chalmers.TDA367.B17.MapLoader;
import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.console.Console;
import chalmers.TDA367.B17.console.Console.MsgLevel;
import chalmers.TDA367.B17.console.Console.OutputLevel;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;
import chalmers.TDA367.B17.model.Entity;
import chalmers.TDA367.B17.model.Entity.RenderLayer;
import chalmers.TDA367.B17.model.World;
import chalmers.TDA367.B17.network.Network;
import chalmers.TDA367.B17.network.Network.EntityPacket;
import chalmers.TDA367.B17.network.Network.Packet;
import chalmers.TDA367.B17.network.Network.Pck0_JoinRequest;
import chalmers.TDA367.B17.network.Network.Pck1000_GameEvent;
import chalmers.TDA367.B17.network.Network.Pck100_WorldState;
import chalmers.TDA367.B17.network.Network.Pck102_TankUpdate;
import chalmers.TDA367.B17.network.Network.Pck103_ProjectileUpdate;
import chalmers.TDA367.B17.network.Network.Pck10_TankCreated;
import chalmers.TDA367.B17.network.Network.Pck11_PickupCreated;
import chalmers.TDA367.B17.network.Network.Pck1_LoginAnswer;
import chalmers.TDA367.B17.network.Network.Pck2_ClientConfirmJoin;
import chalmers.TDA367.B17.network.Network.Pck31_ChatMessage;
import chalmers.TDA367.B17.network.Network.Pck3_Message;
import chalmers.TDA367.B17.network.Network.Pck4_ClientInput;
import chalmers.TDA367.B17.network.Network.Pck7_TankID;
import chalmers.TDA367.B17.network.Network.Pck8_EntityDestroyed;
import chalmers.TDA367.B17.network.Network.Pck9_EntityCreated;
import chalmers.TDA367.B17.powerups.Shield;
import chalmers.TDA367.B17.powerups.powerupPickups.DamagePowerUpPickup;
import chalmers.TDA367.B17.powerups.powerupPickups.FireRatePowerUpPickup;
import chalmers.TDA367.B17.powerups.powerupPickups.HealthPowerUpPickup;
import chalmers.TDA367.B17.powerups.powerupPickups.ShieldPowerUpPickup;
import chalmers.TDA367.B17.powerups.powerupPickups.SpeedPowerUpPickup;
import chalmers.TDA367.B17.tanks.DefaultTank;
import chalmers.TDA367.B17.view.Lifebar;
import chalmers.TDA367.B17.view.SoundSwitch;
import chalmers.TDA367.B17.weaponPickups.BouncePickup;
import chalmers.TDA367.B17.weaponPickups.FlamethrowerPickup;
import chalmers.TDA367.B17.weaponPickups.ShockwavePickup;
import chalmers.TDA367.B17.weaponPickups.ShotgunPickup;
import chalmers.TDA367.B17.weaponPickups.SlowspeedyPickup;
import chalmers.TDA367.B17.weapons.BounceProjectile;
import chalmers.TDA367.B17.weapons.BounceTurret;
import chalmers.TDA367.B17.weapons.DefaultProjectile;
import chalmers.TDA367.B17.weapons.DefaultTurret;
import chalmers.TDA367.B17.weapons.FlamethrowerProjectile;
import chalmers.TDA367.B17.weapons.FlamethrowerTurret;
import chalmers.TDA367.B17.weapons.ShockwaveProjectile;
import chalmers.TDA367.B17.weapons.ShockwaveSecondaryProjectile;
import chalmers.TDA367.B17.weapons.ShockwaveTurret;
import chalmers.TDA367.B17.weapons.ShotgunProjectile;
import chalmers.TDA367.B17.weapons.ShotgunTurret;
import chalmers.TDA367.B17.weapons.SlowspeedyProjectile;
import chalmers.TDA367.B17.weapons.SlowspeedyTurret;

import com.esotericsoftware.kryonet.Client;
import com.esotericsoftware.kryonet.Connection;
import com.esotericsoftware.kryonet.Listener;
import com.esotericsoftware.minlog.Log;

public class ClientState extends TanskState {
	private static ClientState instance;
	
	private Client client;
	private boolean isConnected;
	private Input input;
	private Image map = null;
	private SpriteSheet entSprite;
	private String playerName;
	private boolean mapLoaded;
	private AbstractTank playerTank;
	private Lifebar lifebar;
	private SoundSwitch soundSwitch;
	protected TextField chatField;
		
	private ClientState(int state) {
		super(state);
	}
	
	public static ClientState getInstance(){
		if(instance == null)
			instance = new ClientState(Tansk.CLIENT);
	
		return instance;
	}
	
	@Override
    public void init(GameContainer gc, StateBasedGame game) throws SlickException {
		super.init(gc, game);
		
		map = new Image(Tansk.IMAGES_FOLDER + "/map.png");
		playerName = "Nisse" + Math.round(Math.random() * 1000);
    }
	
	@Override
	public void enter(GameContainer gc, StateBasedGame game) throws SlickException {
		super.enter(gc, game);

		chatField = new TextField(gc, gc.getDefaultFont(), 10, 733, 450, 23);
		Console console = new Console(10, 533, 450, 192, OutputLevel.ALL);
		console.setBorder(false);
		console.setTimerHide(true);
		controller.setConsole(console);

		lifebar = new Lifebar((Tansk.SCREEN_WIDTH/2)-100, 10);
		controller.setWorld(new World(new Dimension(Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT), false));
		soundSwitch = new SoundSwitch(Tansk.SCREEN_WIDTH-40, 10);
		
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
				GameController.getInstance().getConsole().setTimerHide(false);
			}
		});
		client.start();
		
		try {
	        client.connect(600000, "127.0.0.1", Network.PORT, Network.PORT);
        } catch (IOException e) {
        	Log.info("[CLIENT] Failed to connect!");
			GameController.getInstance().getConsole().addMsg("Failed to connect to server.", MsgLevel.ERROR);
			GameController.getInstance().getConsole().setTimerHide(false);
	        e.printStackTrace();
	        client.stop();
        } 
		
		input = gc.getInput();
		input.addMouseListener(this);
	}	
	
	@Override
    public void update(GameContainer gc, StateBasedGame game, int delta) throws SlickException {
		super.update(gc, game, delta);
		if((isConnected) && (!mapLoaded)){
			MapLoader.createEntities("whatever");
			mapLoaded = true;
		}
		
		if(input.isKeyDown(Input.KEY_UP)){
			float tmp = controller.getSoundHandler().getVolume();
			if(tmp < 1){
				tmp+=0.1;
				controller.getSoundHandler().setVolume(tmp);
			}
		}
		if(input.isKeyDown(Input.KEY_DOWN)){
			float tmp = controller.getSoundHandler().getVolume();
			if(tmp >= 0.1){
				tmp-=0.1f;
			}else if(tmp == 0.1f){
				tmp = 0;
			}
			controller.getSoundHandler().setVolume(tmp);
		}
		if(input.isKeyPressed(Input.KEY_S) && input.isKeyDown(Input.KEY_LCONTROL)){
			if(controller.getSoundHandler().isSoundOn()){
				soundSwitch.turnSoundOff(controller.getSoundHandler().getVolume());
			}else{
				soundSwitch.turnSoundOn();
			}
		}
		
		GameController.getInstance().getConsole().update(delta);
		processPackets();			
		sendClientInput(gc.getInput());
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
		controller.getAnimationHandler().renderAnimations();
		renderGUI(container, g);
    }
		
	@Override
	public void renderGUI(GameContainer gc, Graphics g){
		super.renderGUI(gc, g);
		
		g.drawRect(chatField.getX(), chatField.getY(), chatField.getWidth(), chatField.getHeight());
		chatField.render(gc, g);
		
		if(playerTank != null){
			if(playerTank.getShield() != null && playerTank.getShield().getHealth() <= 100){
				lifebar.render(playerTank.getHealth()/playerTank.getMaxHealth(), playerTank.getShield().getHealth()/playerTank.getMaxShieldHealth(), g);
			}else{
				lifebar.render(playerTank.getHealth()/playerTank.getMaxHealth(), 0, g);
			}
		}
		soundSwitch.render(g);
		
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
		
		g.setColor(Color.black);
		g.drawString("Volume: " + ((int)(controller.getSoundHandler().getVolume() * 100)) + " %",  10, 50);
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
					GameController.getInstance().getConsole().setTimerHide(false);
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
				if(playerTank != null){
					if(pck.entityID == playerTank.getId()){
						playerTank = null;			
					}	
				}
			}
			
			if(packet instanceof Pck9_EntityCreated){
				createClientEntity((Pck9_EntityCreated) packet);
			}
			
			if(packet instanceof Pck10_TankCreated){
				Pck10_TankCreated pck = (Pck10_TankCreated) packet;
				createClientTank(pck.entityID, pck.identifier, pck.direction, pck.color);
			}
			
			if(packet instanceof Pck11_PickupCreated){
				Pck11_PickupCreated pck = (Pck11_PickupCreated) packet;
				createClientPickup(pck.entityID, pck.identifier, pck.position);
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
			if(pck instanceof Pck102_TankUpdate){
				Pck102_TankUpdate packet = (Pck102_TankUpdate) pck;
				AbstractTank tank = (AbstractTank) controller.getWorld().getEntity(packet.entityID);
				tank.setPosition(packet.tankPosition);
				tank.setDirection(packet.tankDirection);
				tank.setHealth(packet.tankHealth);
				if(tank.getShield() != null){
					tank.getShield().setHealth(packet.tankShieldHealth);
					if(packet.shieldPosition != null)
						tank.getShield().setPosition(packet.shieldPosition);
				}
				AbstractTurret turret = tank.getTurret();
				turret.setPosition(packet.turretPosition);
				turret.setRotation(packet.turretAngle);
			}
			if(pck instanceof Pck103_ProjectileUpdate){
				Pck103_ProjectileUpdate packet = (Pck103_ProjectileUpdate) pck;
				AbstractProjectile proj = (AbstractProjectile) controller.getWorld().getEntity(packet.entityID);
				if(proj != null){
					proj.setPosition(packet.projPosition);
					proj.setDirection(packet.projDirection);
				}
			}
		}
    }

	public String getPlayerName() {
	    return playerName;
    }
		
	@Override
	public void keyReleased(int key, char c){
		super.keyReleased(key, c);
		if(key == Input.KEY_ENTER){
			if(chatField.hasFocus()){
				if(!chatField.getText().equals("")){
					if(client != null){
						String msg = playerName + ": " + chatField.getText();
						msg = msg.replace('\n', ' ');
						if(msg.length() > 39)
							msg = msg.substring(0, 38);
						Pck31_ChatMessage pck = new Pck31_ChatMessage();
						pck.message = msg;
						chatField.setText("");
						client.sendTCP(pck);
						GameController.getInstance().getConsole().setActive(true);
					}
				} else {
					GameController.getInstance().getConsole().setVisible(false);
				}
				chatField.setFocus(false);
			} else {
				GameController.getInstance().getConsole().setVisible(true);
				chatField.setFocus(true);
			}	
		}
	}
		
	private void createClientTank(int entityID, String identifier, Vector2f direction, String playerColor) {
	    if(identifier.equals("DefaultTank")){
			new DefaultTank(entityID, direction, null, playerColor);
	    }
    }
	
	private void createClientPickup(int entityID, String identifier, Vector2f position){
		if(identifier.equals("BouncePickup")){
			new BouncePickup(entityID, position);
		} else if(identifier.equals("FlamethrowerPickup")){
			new FlamethrowerPickup(entityID, position);
		} else if(identifier.equals("ShockwavePickup")){
			new ShockwavePickup(entityID, position);
		} else if(identifier.equals("SlowspeedyPickup")){
			new SlowspeedyPickup(entityID, position);
		}  else if(identifier.equals("ShotgunPickup")){
			new ShotgunPickup(entityID, position);
		}  else if(identifier.equals("DamagePowerUpPickup")){
			new DamagePowerUpPickup(entityID, position);
		}  else if(identifier.equals("FireRatePowerUpPickup")){
			new FireRatePowerUpPickup(entityID, position);
		}  else if(identifier.equals("HealthPowerUpPickup")){
			new HealthPowerUpPickup(entityID, position);
		}  else if(identifier.equals("ShieldPowerUpPickup")){
			new ShieldPowerUpPickup(entityID, position);
		}  else if(identifier.equals("SpeedPowerUpPickup")){
			new SpeedPowerUpPickup(entityID, position);
		} 
	}
	

//	private void createClientEntity(int entityID, String identifier, int possibleOwnerID) { 
	private void createClientEntity(Pck9_EntityCreated pck) {
	    if(pck.identifier.equals("DefaultProjectile")){
			new DefaultProjectile(pck.entityID, null, null);
	    } else if(pck.identifier.equals("BounceProjectile")){
			new BounceProjectile(pck.entityID, null, null);
	    } else if(pck.identifier.equals("FlamethrowerProjectile")){
			new FlamethrowerProjectile(pck.entityID, null, null);
	    } else if(pck.identifier.equals("ShockwaveProjectile")){
			new ShockwaveProjectile(pck.entityID, null, null);
	    } else if(pck.identifier.equals("ShockwaveSecondaryProjectile")){
			new ShockwaveSecondaryProjectile(pck.entityID, null, null, null);
	    } else if(pck.identifier.equals("ShotgunProjectile")){
			new ShotgunProjectile(pck.entityID, null, null);
	    } else if(pck.identifier.equals("SlowspeedyProjectile")){
			new SlowspeedyProjectile(pck.entityID, null, null);
	    }  else if(pck.identifier.equals("Shield")){
	    	AbstractTank tank = (AbstractTank)GameController.getInstance().getWorld().getEntity(pck.possibleOwnerID);
	    	tank.setShield(new Shield(pck.entityID, tank, 0));
	    } else if(pck.identifier.equals("DefaultTurret")){
	    	AbstractTank tank = (AbstractTank)GameController.getInstance().getWorld().getEntity(pck.possibleOwnerID);
	    	if(tank != null){
	    		double rotation = tank.getTurret().getRotation();
	    		Vector2f position = tank.getTurret().getPosition();
	    		AbstractTurret turret = new DefaultTurret(pck.entityID, position, rotation, tank, pck.color);
	    		tank.setTurret(turret);
	    	}
	    } else if(pck.identifier.equals("BounceTurret")){
	    	AbstractTank tank = (AbstractTank)GameController.getInstance().getWorld().getEntity(pck.possibleOwnerID);
	    	if(tank != null){
	    		double rotation = tank.getTurret().getRotation();
	    		Vector2f position = tank.getTurret().getPosition();
	    		AbstractTurret turret = new BounceTurret(pck.entityID, position, rotation, tank, tank.getColor());
	    		tank.setTurret(turret);
	    	}
	    } else if(pck.identifier.equals("FlamethrowerTurret")){
	    	AbstractTank tank = (AbstractTank)GameController.getInstance().getWorld().getEntity(pck.possibleOwnerID);
	    	if(tank != null){
	    		double rotation = tank.getTurret().getRotation();
	    		Vector2f position = tank.getTurret().getPosition();
	    		AbstractTurret turret = new FlamethrowerTurret(pck.entityID, position, rotation, tank, tank.getColor());
	    		tank.setTurret(turret);
	    	}
	    } else if(pck.identifier.equals("ShockwaveTurret")){
	    	AbstractTank tank = (AbstractTank)GameController.getInstance().getWorld().getEntity(pck.possibleOwnerID);
	    	if(tank != null){
	    		double rotation = tank.getTurret().getRotation();
	    		Vector2f position = tank.getTurret().getPosition();
	    		AbstractTurret turret = new ShockwaveTurret(pck.entityID, position, rotation, tank, tank.getColor());
	    		tank.setTurret(turret);
	    	}
	    } else if(pck.identifier.equals("ShotgunTurret")){
	    	AbstractTank tank = (AbstractTank)GameController.getInstance().getWorld().getEntity(pck.possibleOwnerID);
	    	if(tank != null){
	    		double rotation = tank.getTurret().getRotation();
	    		Vector2f position = tank.getTurret().getPosition();
	    		AbstractTurret turret = new ShotgunTurret(pck.entityID, position, rotation, tank, tank.getColor());
	    		tank.setTurret(turret);
	    	}
	    } else if(pck.identifier.equals("SlowspeedyTurret")){
	    	AbstractTank tank = (AbstractTank)GameController.getInstance().getWorld().getEntity(pck.possibleOwnerID);
	    	if(tank != null){
	    		double rotation = tank.getTurret().getRotation();
	    		Vector2f position = tank.getTurret().getPosition();
	    		AbstractTurret turret = new SlowspeedyTurret(pck.entityID, position, rotation, tank, tank.getColor());
	    		tank.setTurret(turret);
	    	}
	    } 
    }
}