package chalmers.TDA367.B17;

import org.newdawn.slick.AppGameContainer;
import org.newdawn.slick.GameContainer;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.state.StateBasedGame;

public class Game extends StateBasedGame {

	public static final String NAME = "Tansk!";
	public static final int MENU = 0;
	public static final int PLAY = 1;

	public static final int SCREEN_WIDTH = 1024;
	public static final int SCREEN_HEIGHT = 768;

	public Game(String name) {
		super(name);
		this.addState(new Menu(MENU));
		this.addState(new Play(PLAY));
	}

	@Override
	public void initStatesList(GameContainer gc) throws SlickException {
		this.getState(MENU).init(gc, this);
		this.getState(PLAY).init(gc, this);
		this.enterState(MENU);
	}

	public static void main(String[] args) throws SlickException {
		AppGameContainer app = new AppGameContainer(new Game(NAME));

		app.setTargetFrameRate(60);
		app.setMaximumLogicUpdateInterval(500);
		app.setMinimumLogicUpdateInterval(5);
		app.setDisplayMode(SCREEN_WIDTH, SCREEN_HEIGHT, false);
		
		app.start();
  }
}
