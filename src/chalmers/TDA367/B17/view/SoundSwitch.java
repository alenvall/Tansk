package chalmers.TDA367.B17.view;

import org.newdawn.slick.Graphics;

import chalmers.TDA367.B17.controller.GameController;


public class SoundSwitch {
	
	private int x;
	private int y;
	private boolean isSoundOn;
	
	public SoundSwitch(int x, int y){
		this.x = x;
		this.y = y;
	}
	
	public void render(Graphics g){
		if(isSoundOn){
			g.drawImage(GameController.getInstance().getImageHandler().getSprite("speaker_on"), x, y);
		}else{
			g.drawImage(GameController.getInstance().getImageHandler().getSprite("speaker_off"), x, y);
		}
	}
}
