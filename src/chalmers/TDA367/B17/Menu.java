package chalmers.TDA367.B17;

import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Input;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.state.BasicGameState;
import org.newdawn.slick.state.StateBasedGame;

public class Menu extends BasicGameState{
	
	Rectangle playGame;
	Rectangle exitGame;

	public Menu(int state) {
		
	}

	public void init(GameContainer gc, StateBasedGame sbg)
			throws SlickException {
		playGame = new Rectangle(100, 125, 150, 50);
		exitGame = new Rectangle(100, 225, 150, 50);
		
	}

	@Override
	public void render(GameContainer gc, StateBasedGame sbg, Graphics g)
			throws SlickException {
		g.drawString("Welcome to Tansk!", 100, 50);
		g.drawString("Play", 150, 140);
		g.drawString("Exit", 150, 240);
		g.draw(playGame);
		g.draw(exitGame);
	}

	@Override
	public void update(GameContainer gc, StateBasedGame sbg, int delta)
			throws SlickException {
		Input input = gc.getInput();
		int x = input.getMouseX();
		int y = input.getMouseY();
		
		if(x > 100 && x < 250 && y > 125 && y < 175){
			if(input.isMouseButtonDown(0)){
				sbg.enterState(1);
			}
		}
		
		if(x > 100 && x < 250 && y > 225 && y < 275){
			if(input.isMouseButtonDown(0)){
				gc.exit();
			}
		}
	}

	@Override
	public int getID() {
		return 0;
	}

}
