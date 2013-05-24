package chalmers.TDA367.B17.states;

import org.newdawn.slick.Color;
import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Input;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.SpriteSheet;
import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.state.BasicGameState;
import org.newdawn.slick.state.StateBasedGame;
import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.view.MenuButton;

public class HostMenu extends BasicGameState{
	
	private MenuButton startButton;
	private MenuButton backButton;
	private boolean running = false;
	private int state;
	private String message;
	private SpriteSheet background;
	
	public HostMenu(int state) {
		this.state = state;
	}

	public void init(GameContainer gc, StateBasedGame sbg)
			throws SlickException {
		startButton = new MenuButton(100, 125, GameController.getInstance().getImageHandler().getSprite("button_start"),
				GameController.getInstance().getImageHandler().getSprite("button_start_pressed"),
				GameController.getInstance().getImageHandler().getSprite("button_start_hover"));

		backButton = new MenuButton(100, 235, GameController.getInstance().getImageHandler().getSprite("button_back"),
				GameController.getInstance().getImageHandler().getSprite("button_back_pressed"),
				GameController.getInstance().getImageHandler().getSprite("button_back_hover"));
		
		background = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("background"),
				Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT);
		message = "Not running.";
	}

	@Override
	public void render(GameContainer gc, StateBasedGame sbg, Graphics g)
			throws SlickException {
		background.draw();
		startButton.draw();
		backButton.draw();
		if(running){
			g.setColor(Color.blue);
			g.drawString(message, 120, 180);
		} else {
			g.setColor(Color.red);
			g.drawString(message, 120, 180);
		}
	}

	@Override
	public void update(GameContainer gc, StateBasedGame sbg, int delta) throws SlickException {			
		if(backButton.isClicked(gc.getInput())){
			sbg.enterState(Tansk.MENU);
		}
		else if(startButton.isClicked(gc.getInput())){
			System.out.println("Starting server!");
			running = true;
			message = "Running!";
			sbg.enterState(Tansk.SERVER);
		}
	}

	@Override
	public int getID() {
		return this.state;
	}

}