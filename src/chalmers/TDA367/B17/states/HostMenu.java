package chalmers.TDA367.B17.states;

import org.newdawn.slick.Color;
import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.SpriteSheet;

import org.newdawn.slick.geom.Vector2f;

import org.newdawn.slick.gui.TextField;

import org.newdawn.slick.state.BasicGameState;
import org.newdawn.slick.state.StateBasedGame;
import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.view.Label;
import chalmers.TDA367.B17.view.MenuButton;
import chalmers.TDA367.B17.view.Slider;

public class HostMenu extends BasicGameState{
	
	private MenuButton startButton;
	private MenuButton backButton;
	private int state;
	private SpriteSheet background;
	private TextField roundField;
	private Label roundLabel;
	private Label playerLivesLabel;
	private TextField playerLivesField;
	private Label roundTimeLabel;
	private TextField spawnTimeField;
	private Label spawnTimeLabel;
	private Label gameTimeLabel;
	private TextField gameTimeField;
	private TextField roundTimeField;
	
	private Slider slider;
	
	public HostMenu(int state) {
		this.state = state;
	}

	public void init(GameContainer gc, StateBasedGame sbg)
			throws SlickException {
		startButton = new MenuButton(100, 425, GameController.getInstance().getImageHandler().getSprite("button_start"),
				GameController.getInstance().getImageHandler().getSprite("button_start_pressed"),
				GameController.getInstance().getImageHandler().getSprite("button_start_hover"));

		backButton = new MenuButton(100, 525, GameController.getInstance().getImageHandler().getSprite("button_back"),
				GameController.getInstance().getImageHandler().getSprite("button_back_pressed"),
				GameController.getInstance().getImageHandler().getSprite("button_back_hover"));
		
		background = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("background"),
				Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT);

		slider = new Slider(30, 5, 15, new Vector2f(100, 500), gc, "Scorelimit: ");

	
		roundLabel = new Label("Rounds:", Color.black, 100, 255);
		roundField = new TextField(gc, gc.getDefaultFont(), 210, 255, 40, 20);
		
		playerLivesLabel = new Label("Lives:", Color.black, 100, 285);
		playerLivesField = new TextField(gc, gc.getDefaultFont(), 210, 285, 40, 20);
		
		spawnTimeLabel = new Label("Spawn time:", Color.black, 100, 315);
		spawnTimeField = new TextField(gc, gc.getDefaultFont(), 210, 315, 40, 20);
		
		roundTimeLabel = new Label("Round time:", Color.black, 100, 345);
		roundTimeField = new TextField(gc, gc.getDefaultFont(), 210, 345, 40, 20);
		
		gameTimeLabel = new Label("Game time:", Color.black, 100, 375);
		gameTimeField = new TextField(gc, gc.getDefaultFont(), 210, 375, 40, 20);

	}

	@Override
	public void render(GameContainer gc, StateBasedGame sbg, Graphics g)
			throws SlickException {
		background.draw();
		startButton.draw();
		backButton.draw();

		slider.draw(g);

		
		roundField.render(gc, g);
		roundLabel.render(g);
		playerLivesField.render(gc, g);
		playerLivesLabel.render(g);
		spawnTimeField.render(gc, g);
		spawnTimeLabel.render(g);
		roundTimeField.render(gc, g);
		roundTimeLabel.render(g);
		gameTimeField.render(gc, g);
		gameTimeLabel.render(g);

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
		slider.update();
	}

	@Override
	public int getID() {
		return this.state;
	}

}