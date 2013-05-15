package chalmers.TDA367.B17.sound;


import java.io.File;
import java.util.HashMap;
import java.util.Map;

import org.newdawn.slick.SlickException;
import org.newdawn.slick.Sound;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
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
	public void handleEvent(GameEvent event){
		if(event.getEventType().equals("TANK_DEATH_EVENT")) {
			sounds.get("Tank_Destroyed").play();
			Vector2f tmpPos = event.getSource().getSpritePosition();
			GameController.getInstance().getAnimationHandler().newExplosion(new Vector2f(tmpPos.x-24, tmpPos.y-31));
		}else if(event.getEventType().equals("DEFAULTTURRET_FIRE_EVENT")){
			Sound tmp = sounds.get("DefaultWeapon_Firing2");
			tmp.play(3, 125);
		}else if(event.getEventType().equals("TANK_HIT_EVENT")){
			sounds.get("Tank_Hit").play();
		}else if(event.getEventType().equals("FLAMETHROWER_EVENT")){
			Sound tmp = sounds.get("Flamethrower_Firing");
			tmp.play(1, 135);
		}else if(event.getEventType().equals("SHOTGUN_EVENT")){
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
