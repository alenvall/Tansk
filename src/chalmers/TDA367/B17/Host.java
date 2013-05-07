package chalmers.TDA367.B17;

import java.io.IOException;

import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Input;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.state.BasicGameState;
import org.newdawn.slick.state.StateBasedGame;

import chalmers.TDA367.B17.network.GameServer;

public class Host extends BasicGameState{
	
	private Rectangle startServer;
	private int state;
	
	public Host(int state) {
		this.state = state;
	}

	public void init(GameContainer gc, StateBasedGame sbg)
			throws SlickException {
		startServer = new Rectangle(100, 125, 150, 50);
	}

	@Override
	public void render(GameContainer gc, StateBasedGame sbg, Graphics g)
			throws SlickException {
		g.drawString("Start", 150, 140);
		g.draw(startServer);
	}

	@Override
	public void update(GameContainer gc, StateBasedGame sbg, int delta) throws SlickException {	
		Input input = gc.getInput();
		int x = input.getMouseX();
		int y = input.getMouseY();
		
		if(x > 100 && x < 250 && y > 125 && y < 175){
			if(input.isMousePressed(0)){
				System.out.println("Starting server!");
				try {
	                new GameServer();
                } catch (IOException e) {
	                System.out.println("Failed to start server!");
	                e.printStackTrace();
                }
			}
		}
	}

	@Override
	public int getID() {
		return this.state;
	}

}