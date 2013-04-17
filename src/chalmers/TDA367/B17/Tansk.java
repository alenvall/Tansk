package chalmers.TDA367.B17;

import chalmers.TDA367.B17.model.*;
import org.newdawn.slick.*;
import org.newdawn.slick.geom.*;
import java.util.*;

public class Tansk extends BasicGame {
	World world;
	Input input;
	ArrayList<Player> players;
	Player playerOne;
	Image plane = null;
	Image map = null;
	boolean newImages = true;

	public Tansk() {
		super("Tansk!");
	}
 
	@Override
	public void init(GameContainer gc) throws SlickException {
		world = new World();
		playerOne = new Player("Player One");
		players = new ArrayList<Player>();
		players.add(playerOne);
		
		plane = new Image("tank.png");
		map = new Image("map.png");
	}
 
	@Override
	public void update(GameContainer gc, int delta) throws SlickException {
		input = gc.getInput();

		if(input.isKeyDown(Input.KEY_W)){
			playerOne.getTank().accelerate(delta);
		}else if(input.isKeyDown(Input.KEY_S)){
			playerOne.getTank().reverse(delta);
		}else{
			playerOne.getTank().deaccelerate(delta);
		}
	  
		if(input.isKeyDown(Input.KEY_A) && !input.isKeyDown(Input.KEY_D)){
			if(input.isKeyDown(Input.KEY_S)){
				playerOne.getTank().turnRight(delta);
			} else {
				playerOne.getTank().turnLeft(delta);
			}
		}
	
		if(input.isKeyDown(Input.KEY_D) && !input.isKeyDown(Input.KEY_A)){
			if(input.isKeyDown(Input.KEY_S)){
				playerOne.getTank().turnLeft(delta);
			} else {
				playerOne.getTank().turnRight(delta);
			}
		}
		
		if(input.isKeyDown(Input.KEY_TAB)){
			if(newImages){
				plane = new Image("plane.png");
				map = new Image("map.jpg");
				playerOne.getTank().setDirection(new Vector2f(0, -1));
				newImages = false;
			}else{
				plane = new Image("tank.png");
				map = new Image("map.png");
				playerOne.getTank().setDirection(new Vector2f(0, -1));
				newImages = true;
			}
					
		}
		plane.setRotation((float) playerOne.getTank().getDirection().getTheta() + 90);
		playerOne.getTank().update(delta);
}
 
	@Override
	public void render(GameContainer gc, Graphics g) throws SlickException {
		map.draw();
		plane.draw(playerOne.getTank().getPosition().x, playerOne.getTank().getPosition().y);
		
		g.setColor(Color.white);
		g.drawString("posX:  " + playerOne.getTank().getPosition().x, 10, 30); g.drawString("rotX: " + playerOne.getTank().getDirection().x, 180, 30);
		g.drawString("posY:  " + playerOne.getTank().getPosition().y, 10, 50); g.drawString("rotY: " + playerOne.getTank().getDirection().y, 180, 50);
		g.drawString("speed: " + Double.toString(playerOne.getTank().getSpeed()), 10, 70);
  }
 
	public static void main(String[] args) throws SlickException {
		AppGameContainer app = new AppGameContainer(new Tansk());

		app.setTargetFrameRate(60);
		app.setMaximumLogicUpdateInterval(500);
		app.setMinimumLogicUpdateInterval(5);
		app.setDisplayMode(800, 600, false);
	
		app.start();
  }
}