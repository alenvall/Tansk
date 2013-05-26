package chalmers.TDA367.B17.controller;

import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.event.GameEvent.EventType;
import chalmers.TDA367.B17.gamemodes.GameConditions;
import chalmers.TDA367.B17.gamemodes.KingOfTheHillMode;
import chalmers.TDA367.B17.gamemodes.StandardGameMode;
import chalmers.TDA367.B17.model.World;
import chalmers.TDA367.B17.resource.AnimationHandler;
import chalmers.TDA367.B17.resource.ImageHandler;
import chalmers.TDA367.B17.resource.SoundHandler;
import chalmers.TDA367.B17.view.Console;

public class GameController {
	private static GameController instance;
	private World world;
	private Console console;

	private ImageHandler imgHandler;
	private SoundHandler soundHandler;
	private AnimationHandler animationHandler;
	private GameConditions gameMode;
	private int latestID;
	private String playerName;
	private GameSettings gameSettings;

	public static class GameSettings {
		public String gameMode = "";
		public int rounds;
		public int playerLives; 
		public int spawnTime; 
		public int roundTime; 
		public int gameTime;
		public int scorelimit;
	}
	
	private GameController() {
		imgHandler = new ImageHandler();
		imgHandler.loadAllImages(Tansk.IMAGES_FOLDER);
		soundHandler = new SoundHandler();
		soundHandler.loadEverySound(Tansk.SOUNDS_FOLDER);
		animationHandler = new AnimationHandler();
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
	public void newGame(){
		GameController.getInstance().getConsole().addMsg("GameController.newGame()");
		
		if(gameSettings.gameMode.equals("koth")){
			gameMode = new KingOfTheHillMode(gameSettings.scorelimit);
		} else if(gameSettings.gameMode.equals("standard")){
			gameMode = new StandardGameMode(gameSettings.scorelimit);
		}
			
		gameMode.init(gameSettings.rounds, gameSettings.playerLives, gameSettings.spawnTime, gameSettings.roundTime, gameSettings.gameTime);
	}
	
	public void setGameSettings(GameSettings gameSettings){
		this.gameSettings = gameSettings;
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

	public String getPlayerName() {
	    return playerName;
    }
	
	public void setPlayerName(String playerName) {
	    this.playerName = playerName;
    }

	public GameSettings getGameSettings() {
	    return gameSettings;
    }
}
