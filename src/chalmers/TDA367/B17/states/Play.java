package chalmers.TDA367.B17.states;

import java.awt.Dimension;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.Map;
import java.util.Map.Entry;

import org.newdawn.slick.*;
import org.newdawn.slick.geom.*;
import org.newdawn.slick.state.*;

import chalmers.TDA367.B17.MapLoader;
import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.console.Console;
import chalmers.TDA367.B17.console.Console.OutputLevel;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.controller.GameController.GameSettings;
import chalmers.TDA367.B17.gamemodes.KingOfTheHillMode;
import chalmers.TDA367.B17.model.*;
import chalmers.TDA367.B17.sound.SoundHandler.MusicType;
import chalmers.TDA367.B17.view.Lifebar;
import chalmers.TDA367.B17.view.SoundSwitch;
import chalmers.TDA367.B17.weaponPickups.SlowspeedyPickup;
import chalmers.TDA367.B17.weapons.*;

public class Play extends TanskState {
	
	public ArrayList<AbstractTurret> turrets;
	private ArrayList<Player> players;
	private Player playerOne;
	private Image map = null;
	private Input input;
	private SpriteSheet entSprite = null;
	private Lifebar lifebar;
	private SoundSwitch soundSwitch;
	
	private Player playerTwo;
	private Player playerThree;
	private Player playerFour;
	
	public Play(int state) {
	    super(state);
	}
	
	@Override
	public void init(GameContainer gc, StateBasedGame sbg) throws SlickException {
		super.init(gc, sbg);
		
		map = new Image(Tansk.IMAGES_FOLDER + "/map.png");
		
		input = gc.getInput();
		input.addMouseListener(this);
	}	
	
	@Override
	public void enter(GameContainer container, StateBasedGame game) throws SlickException {
		super.enter(container, game);

		Console console = new Console(10, 533, 450, 192, OutputLevel.ALL);
		console.setBorder(false);
		controller.setConsole(console);
		
		controller.setWorld(new World(new Dimension(Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT), false));
		controller.getWorld().init();
		
		GameSettings gameSettings = new GameSettings();
		gameSettings.gameMode = "koth";
		gameSettings.scorelimit = 15;
		gameSettings.rounds = 4;
		
		// convert from seconds to milliseconds
		gameSettings.spawnTime = 5000;
		gameSettings.roundTime = 500000;
		gameSettings.gameTime = 1500000;
		
		GameController.getInstance().setGameSettings(gameSettings);
		controller.newGame();
		if(controller.getGameMode() instanceof KingOfTheHillMode)
			((KingOfTheHillMode)controller.getGameMode()).generateZone(new Vector2f(512, 384));
		
		controller.getSoundHandler().playMusic(MusicType.BATTLE_MUSIC);
			
		lifebar = new Lifebar((Tansk.SCREEN_WIDTH/2)-100, 10);
		soundSwitch = new SoundSwitch(Tansk.SCREEN_WIDTH-40, 10);

		//Players
		playerOne = new Player("Player One");
		playerOne.setColor("pink");
		players = new ArrayList<Player>();
		players.add(playerOne);
		
		playerTwo = new Player("Player Two");
		playerTwo.setColor("red");
		players.add(playerTwo);
		
		playerThree = new Player("Player Three");
		playerThree.setColor("yellow");
		players.add(playerThree);
		
		playerFour = new Player("Player Four");
		players.add(playerFour);
		
		for(Player player : players){
			GameController.getInstance().getGameMode().addPlayer(player);
			player.setLives(GameController.getInstance().getGameMode().getPlayerLives());
			player.setRespawnTime(GameController.getInstance().getGameMode().getSpawnTime());
		}
		
		MapLoader.createEntities("map_standard");

		//Start a new round
		controller.getGameMode().newRoundDelayTimer(3000);
	}
	
	@Override
	public void leave(GameContainer gc, StateBasedGame sbg) throws SlickException {
		super.leave(gc, sbg);
	}
	
	@Override
	public void update(GameContainer gc, StateBasedGame sbg, int delta) throws SlickException {
		super.update(gc, sbg, delta);
		
		if(input.isKeyDown(Input.KEY_W)){
			if(playerOne.getTank() != null)
				playerOne.getTank().accelerate(delta);
		} else if (input.isKeyDown(Input.KEY_S)){
			if(playerOne.getTank() != null)
			playerOne.getTank().reverse(delta);
		} else {
			if(playerOne.getTank() != null)
			playerOne.getTank().friction(delta);
		}

		if(input.isKeyDown(Input.KEY_A) && !input.isKeyDown(Input.KEY_D)){
			if(input.isKeyDown(Input.KEY_S)){
				if(playerOne.getTank() != null)
				playerOne.getTank().turnRight(delta);
			} else {
				if(playerOne.getTank() != null)
				playerOne.getTank().turnLeft(delta);
			}
		}

		if(input.isKeyDown(Input.KEY_D) && !input.isKeyDown(Input.KEY_A)){
			if(input.isKeyDown(Input.KEY_S)){
				if(playerOne.getTank() != null)
				playerOne.getTank().turnLeft(delta);
			} else {
				if(playerOne.getTank() != null)
				playerOne.getTank().turnRight(delta);
			}
		}

		if(input.isMouseButtonDown(0)){
			if(playerOne.getTank() != null)
				playerOne.getTank().fireTurret(delta);
			if(playerTwo.getTank() != null)
				playerTwo.getTank().fireTurret(delta);
		}
		
		if(input.isKeyDown(Input.KEY_Q)){
			new SlowspeedyPickup(GameController.getInstance().generateID(), new Vector2f(800, 300));
		}
						
		for(Player player : players){
			if(player.getTank() != null){
				AbstractTurret turret = player.getTank().getTurret();
				turret.setRotation((float) Math.toDegrees(Math.atan2(turret.getPosition().x - input.getMouseX() + 0, turret.getPosition().y - input.getMouseY() + 0)* -1)+180);		
			}
		}
		
		if(input.isKeyDown(Input.KEY_UP)){
			float tmp = controller.getSoundHandler().getVolume();
			if(tmp + 0.05 < 1){
				tmp+=0.05;
			}else{
				tmp = 1;
			}
			controller.getSoundHandler().setVolume(tmp);
		}
		if(input.isKeyDown(Input.KEY_DOWN)){
			float tmp = controller.getSoundHandler().getVolume();
			if(tmp - 0.05 >= 0){
				tmp-=0.05f;
			}else if(tmp < 0.1f){
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
		
		//Weapons
		if(playerOne.getTank() != null){
			AbstractTank playerOneTank = playerOne.getTank();
			AbstractTurret playerOneTurret = playerOneTank.getTurret();
		
			if(input.isKeyDown(Input.KEY_1)){
				if(playerOne.getTank() != null)
					playerOne.getTank().setTurret(new DefaultTurret(controller.generateID(), playerOneTurret.getPosition(), playerOneTurret.getRotation(), playerOneTank, playerOne.getColor()));
			}
			if(input.isKeyDown(Input.KEY_2)){
				if(playerOne.getTank() != null)
					playerOne.getTank().setTurret(new FlamethrowerTurret(controller.generateID(), playerOneTurret.getPosition(), playerOneTurret.getRotation(), playerOneTank, playerOne.getColor()));
			}
			if(input.isKeyDown(Input.KEY_3)){
				if(playerOne.getTank() != null)
					playerOne.getTank().setTurret(new ShotgunTurret(controller.generateID(), playerOneTurret.getPosition(), playerOneTurret.getRotation(), playerOneTank, playerOne.getColor()));
			}
			if(input.isKeyDown(Input.KEY_4)){
				if(playerOne.getTank() != null)
					playerOne.getTank().setTurret(new SlowspeedyTurret(controller.generateID(), playerOneTurret.getPosition(), playerOneTurret.getRotation(), playerOneTank, playerOne.getColor()));
			}
			if(input.isKeyDown(Input.KEY_5)){
				if(playerOne.getTank() != null)
					playerOne.getTank().setTurret(new ShockwaveTurret(controller.generateID(), playerOneTurret.getPosition(), playerOneTurret.getRotation(), playerOneTank, playerOne.getColor()));
			}
			if(input.isKeyDown(Input.KEY_6)){
				if(playerOne.getTank() != null)
					playerOne.getTank().setTurret(new BounceTurret(controller.generateID(), playerOneTurret.getPosition(), playerOneTurret.getRotation(), playerOneTank, playerOne.getColor()));
			}
		}
		
		controller.getWorld().getTankSpawner().update(delta);
		controller.getWorld().getSpawner().update(delta);
		controller.getGameMode().update(delta);
		
		updateWorld(delta);
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
	
	@Override
	public void render(GameContainer container, StateBasedGame sbg, Graphics g) throws SlickException {	
		map.draw();
		
		// Render the entities in three layers, bottom, middle and top
		ArrayList<Entity> firstLayerEnts = new ArrayList<Entity>();
		ArrayList<Entity> secondLayerEnts = new ArrayList<Entity>();
		ArrayList<Entity> thirdLayerEnts = new ArrayList<Entity>();
		ArrayList<Entity> fourthLayerEnts = new ArrayList<Entity>();

		Iterator<Entry<Integer, Entity>> iterator = controller.getWorld().getEntities().entrySet().iterator();
		while(iterator.hasNext()){
			Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) iterator.next();
			Entity entity = entry.getValue();
			
			if(!entity.getSpriteID().equals("")){
				if(entity.getRenderLayer() == Entity.RenderLayer.FIRST)
					firstLayerEnts.add(entity);
				else if(entity.getRenderLayer() == Entity.RenderLayer.SECOND)
					secondLayerEnts.add(entity);
				else if(entity.getRenderLayer() == Entity.RenderLayer.THIRD)
					thirdLayerEnts.add(entity);
				else if(entity.getRenderLayer() == Entity.RenderLayer.FOURTH)
					fourthLayerEnts.add(entity);
			}
		}
		renderEntities(firstLayerEnts);
		renderEntities(secondLayerEnts);
		renderEntities(thirdLayerEnts);
		renderEntities(fourthLayerEnts);
		
		controller.getAnimationHandler().renderAnimations();
		renderGUI(container, g);
	}
	
	@Override
	public void renderGUI(GameContainer gc, Graphics g){
		super.renderGUI(gc, g);
		if(playerOne.getTank() != null){
			if(playerOne.getTank().getShield() != null && playerOne.getTank().getShield().getHealth() <= 100){
				lifebar.render(playerOne.getTank().getHealth()/AbstractTank.MAX_HEALTH, playerOne.getTank().getShield().getHealth()/playerOne.getTank().getMaxShieldHealth(), g);
			}else{
				lifebar.render(playerOne.getTank().getHealth()/AbstractTank.MAX_HEALTH, 0, g);
			}
		}
		soundSwitch.render(g);
		
		//Cool timer
		if(controller.getGameMode().isDelaying()){
			if(controller.getGameMode().getDelayTimer() > 0)
				g.drawString("Round starts in: " + 
			(controller.getGameMode().getDelayTimer()/1000 + 1) + " seconds!", 500, 350);
		}
		
		if(controller.getGameMode().isGameOver()){
			g.drawString("Game Over!", 500, 300);
			for(Player p: controller.getGameMode().getWinningPlayers()){
				g.drawString("Winner(s): " + p.getName(), 500, 400 + 25*controller.getGameMode().getWinningPlayers().indexOf(p));
			}
			int i = 0;
			for(Player p : controller.getGameMode().getPlayerList()){
				i++;
				g.drawString(p.getName() + "'s score: " + p.getScore(), 500, (500+(i*25)));
			}
		}
		
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
}
