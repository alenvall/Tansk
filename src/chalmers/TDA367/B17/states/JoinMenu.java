package chalmers.TDA367.B17.states;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.network.Network;
import chalmers.TDA367.B17.view.Label;
import chalmers.TDA367.B17.view.MenuButton;
import com.esotericsoftware.kryonet.Client;
import com.esotericsoftware.minlog.Log;
import org.newdawn.slick.*;
import org.newdawn.slick.gui.TextField;
import org.newdawn.slick.state.*;
import chalmers.TDA367.B17.Tansk;

import java.io.IOException;

public class JoinMenu extends BasicGameState{

	private Client client;
	private TextField serverIPField;
	private MenuButton joinButton;
	private Label errorLabel;
	private Label inputLabel;
	private int state;
	private StateBasedGame stateBasedGame;
	
	public JoinMenu(int state) {
		this.state = state;
	}

	public void init(GameContainer gc, StateBasedGame sbg)
			throws SlickException {
		this.stateBasedGame = sbg;

		inputLabel = new Label("Enter host IP:", Color.white, gc.getWidth()/2-200/2, gc.getHeight()/2-40);
		serverIPField = new TextField(gc, gc.getDefaultFont(), gc.getWidth()/2-200/2, gc.getHeight()/2-20/2, 200, 20);
		joinButton = new MenuButton(gc.getWidth()/2-MenuButton.WIDTH/2, gc.getHeight()/2+50,
				GameController.getInstance().getImageHandler().getSprite("button_join"),
				GameController.getInstance().getImageHandler().getSprite("button_join_pressed"));
		errorLabel = new Label("Invalid IP!", Color.red, gc.getWidth()/2-200/2, gc.getHeight()/2+20);
	}

	@Override
	public void enter(GameContainer gc, StateBasedGame stateBasedGame){
		serverIPField.setFocus(true);
	}

	@Override
	public void render(GameContainer gc, StateBasedGame sbg, Graphics g)
			throws SlickException {
		inputLabel.render(g);
		g.drawRect(serverIPField.getX(), serverIPField.getY(), serverIPField.getWidth(), serverIPField.getHeight());
		serverIPField.render(gc, g);
		if(client!=null && !client.isConnected())
			errorLabel.render(g);
		joinButton.draw();

	}

	@Override
	public void update(GameContainer gc, StateBasedGame sbg, int delta) throws SlickException {
		if(joinButton.isClicked(gc.getInput()))
			join();
	}

	@Override
	public void keyReleased(int key, char c) {
		super.keyReleased(key, c);
		if(key == Input.KEY_ENTER){
			if(serverIPField.hasFocus()){
				join();
			}
		}
	}

	private void join(){
		client = new Client();
		Network.register(client);
		client.start();

		try {
			client.connect(600000, serverIPField.getText(), Network.PORT, Network.PORT);
		} catch (IOException e) {
			Log.info("[CLIENT] Failed to connect!");
			e.printStackTrace();
			client.stop();
			return;
		}

		ClientState.getInstance().setClient(client);

		System.out.println("Attempting to join server...");
		stateBasedGame.enterState(Tansk.CLIENT);
	}


	@Override
	public int getID() {
		return this.state;
	}

}