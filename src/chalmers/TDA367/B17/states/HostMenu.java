package chalmers.TDA367.B17.states;

import org.newdawn.slick.*;
import org.newdawn.slick.geom.*;
import org.newdawn.slick.state.*;
import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.controller.GameController;
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

		roundSlider = new Slider(30, 1, 4, new Vector2f(110, 225), gc, "Rounds: ");
		
		playerLivesSlider = new Slider(10, 1, 5, new Vector2f(110, 275), gc, "Player lives: ");
		
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
		playerLivesSlider.draw(g);
		spawnTimeSlider.draw(g);
		roundTimeSlider.draw(g);
		gameTimeSlider.draw(g);
	}

	@Override
	public void update(GameContainer gc, StateBasedGame sbg, int delta) throws SlickException {			
		if(backButton.isClicked(gc.getInput())){
			sbg.enterState(Tansk.MENU);
		}
		else if(startButton.isClicked(gc.getInput())){
			System.out.println("Starting server!");
			sbg.enterState(Tansk.SERVER);
		}
		roundSlider.update();
		playerLivesSlider.update();
		spawnTimeSlider.update();
		roundTimeSlider.update();
		gameTimeSlider.update();
	}

	@Override
	public int getID() {
		return this.state;
	}

}