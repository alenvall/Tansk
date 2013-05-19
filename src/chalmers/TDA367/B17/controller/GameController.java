package chalmers.TDA367.B17.controller;

import java.awt.Dimension;

import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.console.Console;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.event.GameEvent.EventType;
import chalmers.TDA367.B17.animations.AnimationHandler;
import chalmers.TDA367.B17.model.GameConditions;
import chalmers.TDA367.B17.model.World;
import chalmers.TDA367.B17.sound.SoundHandler;

public class GameController {
	private static GameController instance;
	private World world;
	private Console console;

	private ImageHandler imgHandler;
	private SoundHandler soundHandler;
	private AnimationHandler animationHandler;
	private GameConditions gameConditions;
	private int latestID;

	
	private GameController() {
		imgHandler = new ImageHandler();
		imgHandler.loadAllImages(Tansk.DATA_FOLDER);
		soundHandler = new SoundHandler();
		soundHandler.loadEverySound(Tansk.DATA_FOLDER);
		animationHandler = new AnimationHandler();
		gameConditions = new GameConditions();
	}

	public static GameController getInstance(){
		if(instance == null)
			instance = new GameController();
		
		return instance;
	}
	
	public void newGame(int width, int height, int scoreLimit, int rounds, 
			int playerLives, int spawnTime, int roundTime, int gameTime, boolean serverWorld){
		world = new World(new Dimension(width, height), false);
		world.init();
		GameController.getInstance().getConsole().addMsg("GameController.newGame()");
		gameConditions.init(scoreLimit, rounds, 
				playerLives, spawnTime, roundTime, gameTime);
	}
	
	public World getWorld(){
		return world;
	}
	
	public void setWorld(World world){
		this.world = world;
	}
	
	public Console getConsole(){
		return console;
	}
	
	public void setConsole(Console console){
		this.console = console;
	}

	public GameConditions getGameConditions() {
		return gameConditions;
	}

	public void setGameConditions(GameConditions gameConditions) {
		this.gameConditions = gameConditions;
	}
	
	public SoundHandler getSoundHandler(){
		return soundHandler;
	}
	
	public AnimationHandler getAnimationHandler() {
		return animationHandler;
	}
	
	public ImageHandler getImageHandler() {
		return imgHandler;
	}

	public int generateID(){
		latestID += 1;
		return latestID;
	}

	public void handleEvent(GameEvent event) {
	    if(event.getEventType().equals(EventType.SOUND)){
	    	GameController.getInstance().getSoundHandler().playSound(event);
	    } else if(event.getEventType().equals(EventType.ANIM)){
	    	GameController.getInstance().getAnimationHandler().playAnimation(event);
	    }	
    }
}
