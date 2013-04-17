package chalmers.TDA367.B17;

import chalmers.TDA367.B17.model.*;

import org.newdawn.slick.*;
import org.newdawn.slick.geom.*;

import java.awt.Point;
import java.util.*;

public class Tansk extends BasicGame implements MouseListener{
	World world;
	Input input;
	ArrayList<Player> players;
	Player playerOne;
	Image tank = null;
	Image turret = null;
	Image map = null;
	boolean newImages = true;
	Point mouseCoords;
	AbstractTurret tankTurret;
	AbstractTank playerTank;
	Shape recturret;

	public Tansk() {
		super("Tansk!");
	}
 
	@Override
	public void init(GameContainer gc) throws SlickException {
		world = new World();
		playerOne = new Player("Player One");
		players = new ArrayList<Player>();
		players.add(playerOne);
		
		turret = new Image("turret.png");
		tank = new Image("tank.png");
		map = new Image("map.png");
		input = gc.getInput();
		input.addMouseListener(this);
		mouseCoords = new Point();
		tankTurret = playerOne.getTank().getTurret();
		playerTank = playerOne.getTank();
		turret.setCenterOfRotation(0, 0);
	}
 
	@Override
	public void update(GameContainer gc, int delta) throws SlickException {

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
				tank = new Image("plane.png");
				map = new Image("map.jpg");
				playerOne.getTank().setDirection(new Vector2f(0, -1));
				newImages = false;
			}else{
				tank = new Image("tank.png");
				map = new Image("map.png");
				playerOne.getTank().setDirection(new Vector2f(0, -1));
				newImages = true;
			}		
		}
		playerTank.setTurretPosition(new Vector2f(playerTank.getPosition().x+playerTank.getTurretOffset().x, playerTank.getPosition().y+playerTank.getTurretOffset().y));

        Vector2f position = new Vector2f(playerTank.getTurretPosition().x, playerTank.getTurretPosition().y);
        float rotation = (float) Math.toDegrees(Math.atan2(position.x - mouseCoords.x + 0, position.y - mouseCoords.y + 0)* -1)+180;
        tankTurret.setRotation(rotation);
        turret.setRotation(tankTurret.getRotation());
        
 //		tankTurret.setTurretDirection(new Vector2f(mouseCoords.x, mouseCoords.y));
		
		playerOne.getTank().update(delta);
		tank.setRotation((float) (playerTank.getDirection().getTheta() + 90));
	}
	
	public void mouseMoved(int oldx, int oldy, int newx, int newy){
		mouseCoords.setLocation(newx, newy);
	}
 
	@Override
	public void render(GameContainer gc, Graphics g) throws SlickException {
		map.draw();
		tank.draw(playerOne.getTank().getPosition().x, playerOne.getTank().getPosition().y);
		turret.draw(playerTank.getTurretPosition().x, playerTank.getTurretPosition().y);
		
		g.setColor(Color.black);
		g.drawString("posX:  " + playerOne.getTank().getPosition().x, 10, 30);
		g.drawString("posY:  " + playerOne.getTank().getPosition().y, 10, 50);
		g.drawString("rotX: " + playerOne.getTank().getDirection().x, 10, 70);
		g.drawString("rotY: " + playerOne.getTank().getDirection().y, 10, 90);

		g.drawString("turPosX:   " + playerTank.getTurretPosition().x, 200, 30);
		g.drawString("turPosY:   " + playerTank.getTurretPosition().y, 200, 50);
		g.drawString("turAng:    " + tankTurret.getRotation(), 		 200, 70);
		g.drawString("turImgAng: " + turret.getRotation(),			 200, 90);
		
		g.drawString("mouseX: " + mouseCoords.x, 430, 30);
		g.drawString("mouseY: " + mouseCoords.y, 430, 50);
		
		g.drawString("speed: " + Double.toString(playerTank.getSpeed()), 430, 90);
				
		
		g.setColor(Color.red);
		g.drawLine(playerTank.getTurretPosition().x,
					playerTank.getTurretPosition().y, 
					mouseCoords.x, 
					mouseCoords.y);

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