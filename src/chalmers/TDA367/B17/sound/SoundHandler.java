package chalmers.TDA367.B17.sound;


import chalmers.TDA367.B17.event.GameEvent;

public class SoundHandler {

	private static SoundHandler instance;

	private SoundHandler() {
		// TODO Load and store sound effects for later use (?)
	}

	/**
	 * Get the SoundHandler instance.
	 * @return the instance of SoundHandler
	 */
	public static SoundHandler getInstance() {
		if(instance==null)
			instance = new SoundHandler();

		return instance;
	}

	/**
	 * Play sound related to an event.
	 * @param event a GameEvent
	 */
	public void handleEvent(GameEvent event){
		// TODO Implement
	}
}
