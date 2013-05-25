package chalmers.TDA367.B17.states;

import org.newdawn.slick.*;
import org.newdawn.slick.geom.*;
import org.newdawn.slick.state.*;
import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.controller.GameController.GameSettings;
import chalmers.TDA367.B17.view.MenuButton;
import chalmers.TDA367.B17.view.Slider;

public class HostMenu extends BasicGameState{
	
	private MenuButton startButton;
	private MenuButton backButton;
	private int state;
	private SpriteSheet background;
	private Slider roundSlider;
	private Slider playerLivesSlider;
	private Slider spawnTimeSlider;
	private Slider roundTimeSlider;
	private Slider gameTimeSlider;
	private Slider scorelimitSlider;
	private boolean koth;
	private int tempScore;
	private int tempLives;
	
	public HostMenu(int state) {
		this.state = state;
	}

	public void init(GameContainer gc, StateBasedGame sbg)
			throws SlickException {
		startButton = new MenuButton(100, 475, GameController.getInstance().getImageHandler().getSprite("button_start"),
				GameController.getInstance().getImageHandler().getSprite("button_start_pressed"),
				GameController.getInstance().getImageHandler().getSprite("button_start_hover"));

		backButton = new MenuButton(100, 575, GameController.getInstance().getImageHandler().getSprite("button_back"),
				GameController.getInstance().getImageHandler().getSprite("button_back_pressed"),
				GameController.getInstance().getImageHandler().getSprite("button_back_hover"));
		
		background = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("background"),
				Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT);

		playerLivesSlider = new Slider(10, 1, 5, new Vector2f(110, 225), gc, "Player lives: ");
		scorelimitSlider = new Slider(100, 1, 15, new Vector2f(110, 225), gc, "Scorelimit: ");
		roundSlider = new Slider(30, 1, 4, new Vector2f(110, 275), gc, "Rounds: ");
		spawnTimeSlider = new Slider(20, 1, 5, new Vector2f(110, 325), gc, "Spawn time: ");
		roundTimeSlider = new Slider(600, 30, 300, new Vector2f(110, 375), gc, "Round time: ");
		gameTimeSlider = new Slider(1800, 60, 900, new Vector2f(110, 425), gc, "Game time: ");
	}

	@Override
	public void render(GameContainer gc, StateBasedGame sbg, Graphics g)
			throws SlickException {
		background.draw();
		startButton.draw();
		backButton.draw();

		roundSlider.draw(g);
		spawnTimeSlider.draw(g);
		roundTimeSlider.draw(g);
		gameTimeSlider.draw(g);
		
		if(koth)
			scorelimitSlider.draw(g);
		else
			playerLivesSlider.draw(g);
	}

	@Override
	public void update(GameContainer gc, StateBasedGame sbg, int delta) throws SlickException {			
		roundSlider.update();
		spawnTimeSlider.update();
		roundTimeSlider.update();
		gameTimeSlider.update();
		
		if(playerLivesSlider != null){
			playerLivesSlider.update();
			tempLives = playerLivesSlider.getValue();
		}
		if(scorelimitSlider != null){
			scorelimitSlider.update();
			tempScore = scorelimitSlider.getValue();
		}
		
		// debug
		if(gc.getInput().isKeyPressed(Input.KEY_SPACE)){
			if(koth){
				playerLivesSlider = new Slider(10, 1, tempLives, new Vector2f(110, 225), gc, "Player lives: ");
				scorelimitSlider = null;
				koth = false;
			} else {
				scorelimitSlider = new Slider(100, 1, tempScore, new Vector2f(110, 225), gc, "Scorelimit: ");
				playerLivesSlider = null;
				koth = true;
			}
		}
				
		if(backButton.isClicked(gc.getInput())){
			sbg.enterState(Tansk.MENU);
		} else if(startButton.isClicked(gc.getInput())){
			GameSettings gameSettings = new GameSettings();
			
			if(koth){
				gameSettings.gameMode = "koth";
				gameSettings.scorelimit = scorelimitSlider.getValue();
			} else {
				gameSettings.gameMode = "dm";
				gameSettings.playerLives = playerLivesSlider.getValue();
			}
			
			gameSettings.rounds = roundSlider.getValue();
			
			// convert from seconds to milliseconds
			gameSettings.spawnTime = spawnTimeSlider.getValue()*1000;
			gameSettings.roundTime = roundTimeSlider.getValue()*1000;
			gameSettings.gameTime = gameTimeSlider.getValue()*1000;
			
			GameController.getInstance().setGameSettings(gameSettings);
			System.out.println("Starting server!");
			sbg.enterState(Tansk.SERVER);
		}
	}

	@Override
	public int getID() {
		return this.state;
	}

}