package chalmers.TDA367.B17.sound;


import java.io.File;
import java.util.HashMap;
import java.util.Map;

import org.newdawn.slick.SlickException;
import org.newdawn.slick.Sound;

import chalmers.TDA367.B17.event.GameEvent;

public class SoundHandler {
	
	private Map<String, Sound> sounds;

	/**
	 * Create a new SoundHandler.
	 */
	public SoundHandler(){
		sounds = new HashMap<String, Sound>();
	}

	/**
	 * Play sound related to an event.
	 * @param event a GameEvent
	 */
	public void playSound(GameEvent event){
		if(event.getEventDesc().equals("TANK_DEATH_EVENT")) {
			sounds.get("Tank_Destroyed").play();
//			moved to AnimationHandler:
//			Vector2f tmpPos = event.getSource().getSpritePosition();
//			GameController.getInstance().getAnimationHandler().newExplosion(new Vector2f(tmpPos.x-24, tmpPos.y-31));
		}else if(event.getEventDesc().equals("DEFAULTTURRET_FIRE_EVENT")){
			sounds.get("Default_Firing").play();
		}else if(event.getEventDesc().equals("TANK_HIT_EVENT")){
			sounds.get("Tank_Hit").play();
		}else if(event.getEventDesc().equals("FLAMETHROWER_EVENT")){
			Sound tmp = sounds.get("Flamethrower_Firing");
			tmp.play(1, 135);
		}else if(event.getEventDesc().equals("SHOTGUN_EVENT")){
			sounds.get("Shotgun_Firing").play();
		}
	}
	
	public void loadEverySound(String directory){
		File folder = new File(directory);
		File[] listOfFiles = folder.listFiles();

		for(File file : listOfFiles) {
			if(file.isFile()) {
				if(file.getName().contains(".wav")){
					try {						
	                    Sound sound = new Sound(directory + "/" + file.getName());
	                    sounds.put(file.getName().substring(0, file.getName().lastIndexOf('.')), sound);
                    } catch (SlickException e) {
                    	System.out.println("SoundHandler: Failed to load sound!");
	                    e.printStackTrace();
                    }
				}
			}
		}
        System.out.println("SoundHandler: Loaded sound.");
	}
}
