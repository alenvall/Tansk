package chalmers.TDA367.B17.controller;

import java.awt.Dimension;

import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.console.Console;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.event.GameEvent.EventType;
import chalmers.TDA367.B17.gamemodes.GameConditions;
import chalmers.TDA367.B17.gamemodes.StandardGameMode;
import chalmers.TDA367.B17.animations.AnimationHandler;
import chalmers.TDA367.B17.model.World;
import chalmers.TDA367.B17.sound.SoundHandler;

public class GameController {
	private static GameController instance;
	private World world;
	private Console console;

	private ImageHandler imgHandler;
	private SoundHandler soundHandler;
	private AnimationHandler animationHandler;
	private GameConditions gameMode;
	private int latestID;

	
	private GameController() {
		imgHandler = new ImageHandler();
		imgHandler.loadAllImages(Tansk.IMAGES_FOLDER);
		soundHandler = new SoundHandler();
		soundHandler.loadEverySound(Tansk.SOUNDS_FOLDER);
		animationHandler = new AnimationHandler();
		gameMode = new StandardGameMode();
	}

	/**
	 * Get an instance of the controller.
	 * @return instance
	 */
	public static GameController getInstance(){
		if(instance == null)
			instance = new GameController();
		
		return instance;
	}
	
	/**
	 * Prepare for a new game to be started.
	 * @param width Width of the map.
	 * @param height Height of the map.
	 * @param rounds The game's number of rounds.
	 * @param playerLives The number of lives each player get.
	 * @param spawnTime The time it takes to spawn.
	 * @param roundTime The time each round takes.
	 * @param gameTime The total time the game takes.
	 * @param serverWorld If the world should be on a server or not. 
	 */
	public void newGame(int width, int height, int rounds, 
			int playerLives, int spawnTime, int roundTime, int gameTime, boolean serverWorld){
		world = new World(new Dimension(width, height), serverWorld);
		world.init();
		GameController.getInstance().getConsole().addMsg("GameController.newGame()");
		gameMode.init(rounds, 
				playerLives, spawnTime, roundTime, gameTime);
	}
	
	/**
	 * Get the controllers World.
	 * @return world
	 */
	public World getWorld(){
		return world;
	}
	
	/**
	 * Set the controllers World.
	 * @param world
	 */
	public void setWorld(World world){
		this.world = world;
	}
	
	/**
	 * Get the controllers Console.
	 * @return console
	 */
	public Console getConsole(){
		return console;
	}
	
	/**
	 * Set the controllers Console.
	 * @param console
	 */
	public void setConsole(Console console){
		this.console = console;
	}
	
	/**
	 * Get the game mode.
	 * @return gameMode
	 */		
	public GameConditions getGameMode() {
		return gameMode;
	}
	
	/**
	 * Set the controllers GameConditions.
	 * @param gameConditions
	 */
	public void setGameConditions(GameConditions gameConditions) {
		this.gameMode = gameConditions;
	}
	
	/**
	 * Get the controllers SoundHandler.
	 * @return soundHandler
	 */
	public SoundHandler getSoundHandler(){
		return soundHandler;
	}
	
	/**
	 * Get the controllers AnimationHandler.
	 * @return animationHandler
	 */
	public AnimationHandler getAnimationHandler() {
		return animationHandler;
	}
	
	/**
	 * Get the controllers ImageHandler.
	 * @return imgHandler
	 */
	public ImageHandler getImageHandler() {
		return imgHandler;
	}

	/**
	 * Generate a new ID for entities.
	 * @return id
	 */
	public int generateID(){
		latestID += 1;
		return latestID;
	}

	/**
	 * Handle a game event such as a sound effect or an animation.
	 * @param event
	 */
	public void handleEvent(GameEvent event) {
	    if(event.getEventType().equals(EventType.SOUND)){
	    	GameController.getInstance().getSoundHandler().playSound(event);
	    } else if(event.getEventType().equals(EventType.ANIM)){
	    	GameController.getInstance().getAnimationHandler().playAnimation(event);
	    }	
    }
}
