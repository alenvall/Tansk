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
	public static final String SOUNDS_FOLDER = DATA_FOLDER + "/sounds";
	public static final String IMAGES_FOLDER = DATA_FOLDER + "/images";
	public static final String NAME = "Tansk!";
	public static final int MENU = 0;
	public static final int PLAY = 5;
	public static final int HOST = 1;
	public static final int JOIN = 2;
	public static final int CLIENT = 3;
	public static final int SERVER = 4;
	public static final int SETTINGS = 6;

	public Tansk(String name) {
		super(name);
	}

	@Override
	public void initStatesList(GameContainer gc) throws SlickException {
		this.addState(new Menu(MENU));
		this.addState(new Play(PLAY));
		this.addState(new HostMenu(HOST));
		this.addState(new JoinMenu(JOIN));
		this.addState(new SettingsMenu(SETTINGS));
		this.addState(ClientState.getInstance());
		this.addState(ServerState.getInstance());
	}

	public static void main(String[] args) throws SlickException {
		AppGameContainer app = new AppGameContainer(new Tansk(NAME));
		
		app.setShowFPS(false);
		app.setTargetFrameRate(30);
		app.setMaximumLogicUpdateInterval(34);
		app.setMinimumLogicUpdateInterval(19);
		app.setDisplayMode(SCREEN_WIDTH, SCREEN_HEIGHT, false);
		
		app.start();
  }
}
