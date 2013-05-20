package chalmers.TDA367.B17.sound;


import java.io.File;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import org.newdawn.slick.SlickException;
import org.newdawn.slick.Sound;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.event.GameEvent;

public class SoundHandler {
	
	private Map<String, Sound> sounds;
	
	private float volume = 0.5f;

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
		List<Sound> tmpList = new ArrayList<Sound>();
		
		if(event.getEventType().equals("TANK_DEATH_EVENT")) {
			tmpList.add(sounds.get("Tank_Destroyed"));
			
			//Argh...
			Vector2f tmpPos = event.getSource().getSpritePosition();
			GameController.getInstance().getAnimationHandler().newExplosion(new Vector2f(tmpPos.x-24, tmpPos.y-31));
		}else if(event.getEventType().equals("DEFAULTTURRET_FIRE_EVENT")){
			tmpList.add(sounds.get("Default_Firing"));
			
		}else if(event.getEventType().equals("TANK_HIT_EVENT")){
			tmpList.add(sounds.get("Tank_Hit"));
			
		}else if(event.getEventType().equals("FLAMETHROWER_EVENT")){
			tmpList.add(sounds.get("Flamethrower_Firing"));
			
		}else if(event.getEventType().equals("SHOTGUN_FIRE_EVENT")){
			tmpList.add(sounds.get("Shotgun_Firing"));
			
		}else if(event.getEventType().equals("SLOWSPEEDY_FIRE_SECONDARY_EVENT")){
			tmpList.add(sounds.get("Slowspeedy_Firing"));
			
		}else if(event.getEventType().equals("SLOWSPEEDY_FIRE_EVENT")){
			tmpList.add(sounds.get("Shockwave_Firing"));
			
		}else if(event.getEventType().equals("SHOCKWAVE_DETONATE_EVENT")){
			
		}else if(event.getEventType().equals("SHOCKWAVE_FIRE_EVENT")){
			
		}
		for(Sound s : tmpList){
			s.play(1, volume);
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

	public void setVolume(float volume) {
		this.volume = volume;
	}
	public float getVolume() {
		return volume;
	}
}
