package chalmers.TDA367.B17.view;

import org.newdawn.slick.Graphics;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.sound.SoundHandler;


public class SoundSwitch {
	
	private int x;
	private int y;
	private float storedVolume;
	private GameController controller;
	
	public SoundSwitch(int x, int y){
		this.x = x;
		this.y = y;
		controller = GameController.getInstance();
	}
	
	public void render(Graphics g){
		if(controller.getSoundHandler().isSoundOn()){
			g.drawImage(controller.getImageHandler().getSprite("speaker_on"), x, y);
		}else{
			g.drawImage(controller.getImageHandler().getSprite("speaker_off"), x, y);
		}
	}
	/*
	public void soundOnOff(float volume){
		if(isSoundOn){
			storedVolume = volume;
			GameController.getInstance().getSoundHandler().setVolume(0);
			isSoundOn = false;
		}else{
			isSoundOn = true;
			GameController.getInstance().getSoundHandler().setVolume(storedVolume);
		}
	}*/
	
	public void turnSoundOff(float volume){
		storedVolume = volume;
		controller.getSoundHandler().setVolume(0);
	}
	
	public void turnSoundOn(){
		controller.getSoundHandler().setVolume(storedVolume);
	}
	
}
