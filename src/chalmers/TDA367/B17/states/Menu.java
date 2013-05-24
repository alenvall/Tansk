package chalmers.TDA367.B17.states;

import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Input;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.SpriteSheet;
import org.newdawn.slick.state.BasicGameState;
import org.newdawn.slick.state.StateBasedGame;

import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.sound.SoundHandler.MusicType;

public class Menu extends BasicGameState{
	
	SpriteSheet playGame;
	boolean playPressed = false;
	boolean exitPressed = false;
	boolean joinPressed = false;
	boolean hostPressed = false;
	SpriteSheet exitGame;
	SpriteSheet hostGame;
	SpriteSheet joinGame;
	private int state;
	private SpriteSheet background;
	
	public Menu(int state) {
		this.state = state;
	}

	public void init(GameContainer gc, StateBasedGame sbg)
			throws SlickException {
		playGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_playground"), 150, 50);
		exitGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_exit"), 150, 50);
		hostGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_host"), 150, 50);
		joinGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_join"), 150, 50);
		background = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("background"),
				Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT);
		GameController.getInstance().getSoundHandler().playMusic(MusicType.MENU_MUSIC);
	}
	

	@Override
	public void render(GameContainer gc, StateBasedGame sbg, Graphics g)
			throws SlickException {
		background.draw(0,0);
		playGame.draw(100, 125);
		joinGame.draw(100, 225);
		hostGame.draw(100, 325);
		exitGame.draw(100, 425);
	}

	@Override
	public void update(GameContainer gc, StateBasedGame sbg, int delta)
			throws SlickException {
		Input input = gc.getInput();
		int x = input.getMouseX();
		int y = input.getMouseY();
		
		if(x > 100 && x < 250 && y > 125 && y < 175){
			playGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_playground_hover"), 150, 50);
			if(input.isMouseButtonDown(0)){
				playGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_playground_pressed"), 150, 50);
				playPressed = true;
			}
			if(!input.isMouseButtonDown(0)){
				if(playPressed){
					playPressed = false;
					sbg.enterState(Tansk.PLAY);
				}
			}
		}else{
			playGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_playground"), 150, 50);
			playPressed = false;
		}
		
		if(x > 100 && x < 250 && y > 425 && y < 475){
			exitGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_exit_hover"), 150, 50);
			if(input.isMouseButtonDown(0)){
				exitGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_exit_pressed"), 150, 50);
				exitPressed = true;
			}
			if(!input.isMouseButtonDown(0)){
				if(exitPressed){
					exitPressed = false;
					gc.exit();
				}
			}
		}else{
			exitGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_exit"), 150, 50);
			exitPressed = false;
		}
		
		if(x > 100 && x < 250 && y > 325 && y < 375){
			hostGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_host_hover"), 150, 50);
			if(input.isMouseButtonDown(0)){
				hostGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_host_pressed"), 150, 50);
				hostPressed = true;
			}
			if(!input.isMouseButtonDown(0)){
				if(hostPressed){
					hostPressed = false;
					sbg.enterState(Tansk.HOST);
				}
			}
		}else{
			hostGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_host"), 150, 50);
			hostPressed = false;
		}		
		
		if(x > 100 && x < 250 && y > 225 && y < 275){
			joinGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_join_hover"), 150, 50);
			if(input.isMouseButtonDown(0)){
				joinGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_join_pressed"), 150, 50);
				joinPressed = true;
			}
			if(!input.isMouseButtonDown(0)){
				if(joinPressed){
					joinPressed = false;
					sbg.enterState(Tansk.JOIN);
				}
			}
		}else{
			joinGame = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_join"), 150, 50);
			joinPressed = false;
		}
	}

	@Override
	public int getID() {
		return this.state;
	}

}
