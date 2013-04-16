package chalmers.TDA367.B17;

import chalmers.TDA367.B17.model.*;
import org.newdawn.slick.*;
import org.newdawn.slick.geom.*;
import java.util.*;

public class Tansk extends BasicGame {
	World world;
	Input input;
	Shape debugRect;
	ArrayList<Player> players;
	Player playerOne;

	public Tansk() {
		super("Tansk!");
	}
 
	@Override
	public void init(GameContainer gc) throws SlickException {
		world = new World();
		debugRect = new Rectangle(0, 0, 50, 50);

		playerOne = new Player("Player One");
		players = new ArrayList<Player>();
		players.add(playerOne);
	}
 
	@Override
	public void update(GameContainer gc, int delta) throws SlickException {
		input = gc.getInput();

		if(input.isKeyDown(Input.KEY_W)){
			playerOne.getTank().accelerate();
		}else{
			playerOne.getTank().deaccelerate();
		}
	  
		if(input.isKeyDown(Input.KEY_A) && !input.isKeyDown(Input.KEY_D)){
			playerOne.getTank().turnLeft();
			debugRect.setLocation(0, 0);
		}
	
		if(input.isKeyDown(Input.KEY_D) && !input.isKeyDown(Input.KEY_A)){
			playerOne.getTank().turnRight();
			debugRect.setLocation(0, 0);
		}
		playerOne.getTank().update();
		debugRect.setLocation(playerOne.getTank().getPosition().x, playerOne.getTank().getPosition().y);
  }
 
	@Override
	public void render(GameContainer gc, Graphics g) throws SlickException {

		g.setColor(Color.pink);
		g.draw(debugRect);
		g.setColor(Color.white);
		g.drawString("posX:  " + playerOne.getTank().getPosition().x, 10, 30); g.drawString("rotX: " + playerOne.getTank().getDirection().x, 180, 30);
		g.drawString("posY:  " + playerOne.getTank().getPosition().y, 10, 50); g.drawString("rotY: " + playerOne.getTank().getDirection().y, 180, 50);
		g.drawString("speed: " + Double.toString(playerOne.getTank().getSpeed()), 10, 70);
  }
 
	public static void main(String[] args) throws SlickException {
		AppGameContainer app = new AppGameContainer(new Tansk());

		app.setTargetFrameRate(50);
		app.setMaximumLogicUpdateInterval(50);
		app.setMinimumLogicUpdateInterval(10);
		app.setDisplayMode(800, 600, false);
		app.start();
  }
}