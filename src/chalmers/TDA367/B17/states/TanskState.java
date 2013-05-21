package chalmers.TDA367.B17.states;

import java.util.Calendar;
import java.util.concurrent.ConcurrentLinkedQueue;

import org.newdawn.slick.Color;
import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Image;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.gui.TextField;
import org.newdawn.slick.state.BasicGameState;
import org.newdawn.slick.state.StateBasedGame;

import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.network.Network.Packet;

public abstract class TanskState extends BasicGameState {
	protected int state;
	protected GameController controller;
	protected ConcurrentLinkedQueue<Packet> packetQueue;
	protected int frameCounter;
	protected int updatesPerSecond;
	protected int packetsSent;
	protected int packetsSentPerSecond;
	protected int packetsRecPerSecond;
	protected int packetsReceived;
	protected TextField chatField;
	protected long deltaTime;
	protected long oldTime;
	protected int updates;
	
	protected TanskState(int state) {
	    this.state = state;
    }
		
	@Override
	public void init(GameContainer gc, StateBasedGame game) throws SlickException {
		gc.setSmoothDeltas(true);
		gc.setAlwaysRender(true);
		gc.setMouseCursor(new Image(Tansk.IMAGES_FOLDER + "/crosshair.png"), 16, 16);
	}
	
	@Override
	public void enter(GameContainer gc, StateBasedGame game) throws SlickException {
		super.enter(gc, game);

		controller = GameController.getInstance();
		chatField = new TextField(gc, gc.getDefaultFont(), 10, 733, 450, 23);
		packetQueue = new ConcurrentLinkedQueue<Packet>();
	}
		
	@Override
	public void update(GameContainer container, StateBasedGame game, int delta) throws SlickException {
		frameCounter++;
		long newTime = Calendar.getInstance().getTimeInMillis();
		deltaTime = newTime - oldTime;
		if(deltaTime >= 1000){			
			packetsRecPerSecond = packetsReceived;
			packetsReceived = 0;
			packetsSentPerSecond = packetsSent;
			packetsSent = 0;
			updatesPerSecond = updates;
			updates = 0;
			oldTime = newTime;
		}
		updates++;
	}
	
	public void renderGUI(GameContainer container, Graphics g){
		controller.getConsole().renderMessages(g);
		g.setColor(Color.white);
		g.setLineWidth(1);
		g.drawRect(chatField.getX(), chatField.getY(), chatField.getWidth(), chatField.getHeight());
		chatField.render(container, g);
		
		g.setColor(Color.white);
		g.drawString("Packet rec/sec: " + packetsRecPerSecond, 18, 320);
		g.drawString("Packet sent/sec: " + packetsSentPerSecond, 18, 340);
		g.drawString("Update/sec: " + updatesPerSecond, 18, 360);
		g.drawString("Frame: " + frameCounter, 18, 400);
		g.drawString("Entities: " + controller.getWorld().getEntities().size(), 18, 420);
	}
	
	@Override
	public int getID() {
	    return this.state;
	}
}