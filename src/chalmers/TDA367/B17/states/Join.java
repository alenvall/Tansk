package chalmers.TDA367.B17.states;

import org.newdawn.slick.*;
import org.newdawn.slick.geom.*;
import org.newdawn.slick.state.*;
import chalmers.TDA367.B17.network.*;
import chalmers.TDA367.B17.Game;

public class Join extends BasicGameState{
	
	private Rectangle joinServer;
	private Rectangle sendMsg;
	private int state;
	private GameClient client;
	
	public Join(int state) {
		this.state = state;
	}

	public void init(GameContainer gc, StateBasedGame sbg)
			throws SlickException {
		joinServer = new Rectangle(100, 125, 150, 50);
		sendMsg = new Rectangle(100, 225, 150, 50);
	}

	@Override
	public void render(GameContainer gc, StateBasedGame sbg, Graphics g)
			throws SlickException {
		g.drawString("Join", 150, 140);
		g.drawString("Send", 150, 240);
		g.draw(joinServer);
		g.draw(sendMsg);
	}

	@Override
	public void update(GameContainer gc, StateBasedGame sbg, int delta) throws SlickException {	
		Input input = gc.getInput();
		int x = input.getMouseX();
		int y = input.getMouseY();
		
		if(x > 100 && x < 250 && y > 125 && y < 175){
			if(input.isMousePressed(0)){
				System.out.println("Attempting to join server!");
				client = new GameClient();
				client.Connect(600000, "127.0.0.1", Common.port, Common.port);
				((Client) sbg.getState(Game.CLIENT)).setClient(client);
				sbg.enterState(Game.CLIENT);
			}
		}
		
		if(x > 100 && x < 250 && y > 225 && y < 275){
			if(input.isMousePressed(0)){
				if(client != null){
					System.out.println("Attempting to send message to server!");
					client.MessageToServer("Hey!");
				}
			}
		}
	}

	@Override
	public int getID() {
		return this.state;
	}

}