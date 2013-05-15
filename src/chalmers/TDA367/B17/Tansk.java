package chalmers.TDA367.B17;

import chalmers.TDA367.B17.states.*;
import org.newdawn.slick.AppGameContainer;
import org.newdawn.slick.GameContainer;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.state.StateBasedGame;

public class Tansk extends StateBasedGame {
	public static final int SCREEN_WIDTH = 1024;
	public static final int SCREEN_HEIGHT = 768;
	public static final String DATA_FOLDER = "data";
	public static final String NAME = "Tansk!";
	public static final int MENU = 0;
	public static final int PLAY = 5;
	public static final int HOST = 1;
	public static final int JOIN = 2;
	public static final int CLIENT = 3;
	public static final int SERVER = 4;

	public Tansk(String name) {
		super(name);
	}

	@Override
	public void initStatesList(GameContainer gc) throws SlickException {
		this.addState(new Menu(MENU));
		this.addState(new Play(PLAY));
		this.addState(new HostMenu(HOST));
		this.addState(new JoinMenu(JOIN));
		this.addState(ClientState.getInstance());
		this.addState(ServerState.getInstance());
	}

	public static void main(String[] args) throws SlickException {
		AppGameContainer app = new AppGameContainer(new Tansk(NAME));

		app.setTargetFrameRate(60);
		app.setMaximumLogicUpdateInterval(100);
		app.setMinimumLogicUpdateInterval(5);
		app.setDisplayMode(SCREEN_WIDTH, SCREEN_HEIGHT, false);
		
		app.start();
  }
}
