package chalmers.TDA367.B17.view;

import org.newdawn.slick.Graphics;

import chalmers.TDA367.B17.controller.GameController;


public class SoundSwitch {
	
	private int x;
	private int y;
	private float storedVolume;
	private GameController controller;
	
	/**
	 * Create a new SoundSwitch at the given position.
	 * @param x
	 * @param y
	 */
	public SoundSwitch(int x, int y){
		this.x = x;
		this.y = y;
		controller = GameController.getInstance();
	}
	
	/**
	 * Render the SoundSwitch on the current graphics object.
	 * @param g
	 */
	public void render(Graphics g){
		if(controller.getSoundHandler().isSoundOn()){
			if(controller.getSoundHandler().getVolume() > 0.75){
				g.drawImage(controller.getImageHandler().getSprite("speaker_on_3"), x, y);
			}else if(controller.getSoundHandler().getVolume() > 0.4){
				g.drawImage(controller.getImageHandler().getSprite("speaker_on_2"), x, y);
			}else{
				g.drawImage(controller.getImageHandler().getSprite("speaker_on_1"), x, y);
			}
		}else{
			g.drawImage(controller.getImageHandler().getSprite("speaker_off"), x, y);
		}
	}
	
	/**
	 * Turn the sound off and save the volume.
	 * @param volume
	 */
	public void turnSoundOff(float volume){
		storedVolume = volume;
		controller.getSoundHandler().setVolume(0);
	}
	
	/**
	 * Turn the sound on and restore the volume.
	 */
	public void turnSoundOn(){
		controller.getSoundHandler().setVolume(storedVolume);
	}
	
}
