package chalmers.TDA367.B17.states;

import org.newdawn.slick.Color;
import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Input;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.state.BasicGameState;
import org.newdawn.slick.state.StateBasedGame;
import chalmers.TDA367.B17.Tansk;

public class HostMenu extends BasicGameState{
	
	private Rectangle startServer;
	private boolean running = false;
	private int state;
	private String message;
	
	public HostMenu(int state) {
		this.state = state;
	}

	public void init(GameContainer gc, StateBasedGame sbg)
			throws SlickException {
		startServer = new Rectangle(100, 125, 150, 50);
		message = "Not running.";
	}

	@Override
	public void render(GameContainer gc, StateBasedGame sbg, Graphics g)
			throws SlickException {
		g.setColor(Color.white);
		g.drawString("Start", 150, 140);
		g.draw(startServer);
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
		Input input = gc.getInput();
		int x = input.getMouseX();
		int y = input.getMouseY();
		
		if(x > 100 && x < 250 && y > 125 && y < 175){
			if(input.isMousePressed(0)){
				System.out.println("Starting server!");
//				try {
//	                GameServer server = new GameServer();
	                running = true;
	                message = "Running!";
//					((ServerState) sbg.getState(Tansk.SERVER)).setServer(server);
					sbg.enterState(Tansk.SERVER);
//				} catch (java.net.BindException e) {
//					message = "Address in use!";
//                } catch (IOException e) {
//	                System.out.println("Failed to start server!");
//	                e.printStackTrace();
//                }
			}
		}
	}

	@Override
	public int getID() {
		return this.state;
	}

}