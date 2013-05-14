package chalmers.TDA367.B17.sound;


import chalmers.TDA367.B17.event.GameEvent;

public class SoundHandler {

	private static SoundHandler instance;

	private SoundHandler() {}

	public static SoundHandler getInstance() {
		if(instance==null)
			instance = new SoundHandler();

		return instance;
	}

	public void handleEvent(GameEvent event){

	}
}
