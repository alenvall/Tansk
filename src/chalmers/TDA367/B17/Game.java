package chalmers.TDA367.B17;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.states.*;
import org.newdawn.slick.AppGameContainer;
import org.newdawn.slick.GameContainer;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.state.StateBasedGame;

public class Game extends StateBasedGame {

	public static final String NAME = "Tansk!";
	public static final int MENU = 0;
	public static final int PLAY = 1;
	public static final int HOST = 2;
	public static final int JOIN = 3;
	public static final int CLIENT = 4;
	public static final int SERVER = 5;

	public Game(String name) {
		super(name);
	}

	@Override
	public void initStatesList(GameContainer gc) throws SlickException {
		this.addState(new Menu(MENU));
		this.addState(new Play(PLAY));
		this.addState(new Host(HOST));
		this.addState(new Join(JOIN));
		this.addState(new Client(CLIENT));
		this.addState(new Server(SERVER));
		
		this.getState(MENU).init(gc, this);
		this.getState(PLAY).init(gc, this);
		this.getState(HOST).init(gc, this);
		this.getState(JOIN).init(gc, this);
		this.getState(CLIENT).init(gc, this);
		this.getState(SERVER).init(gc, this);
	}

	public static void main(String[] args) throws SlickException {
		AppGameContainer app = new AppGameContainer(new Game(NAME));

		app.setTargetFrameRate(60);
		app.setMaximumLogicUpdateInterval(500);
		app.setMinimumLogicUpdateInterval(5);
		app.setDisplayMode(GameController.SCREEN_WIDTH, GameController.SCREEN_HEIGHT, false);
		
		app.start();
  }
}
