package chalmers.TDA367.B17;

import java.awt.Point;
import java.util.ArrayList;
import java.util.List;

import org.newdawn.slick.Color;
import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Image;
import org.newdawn.slick.Input;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.SpriteSheet;

import chalmers.TDA367.B17.controller.TanskController;
import chalmers.TDA367.B17.model.*;
import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;
import org.newdawn.slick.state.BasicGameState;
import org.newdawn.slick.state.StateBasedGame;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTurret;
import chalmers.TDA367.B17.model.DefaultTurret;
import chalmers.TDA367.B17.model.FlamethrowerTurret;
import chalmers.TDA367.B17.model.Player;
import chalmers.TDA367.B17.model.ShotgunTurret;
import chalmers.TDA367.B17.model.SlowspeedyTurret;

public class Play extends BasicGameState{

	public static final int SCREEN_WIDTH = 1024;
	public static final int SCREEN_HEIGHT = 768;
	
	public ArrayList<AbstractTurret> turrets;
	public int turretIndex;
	
	TanskController controller;
	ArrayList<Player> players;
	Player playerOne;

	Entity obstacle;

	Image map = null;
	SpriteSheet tankSprite  = null;
	SpriteSheet turretSprite = null;
	SpriteSheet projectileSprite = null;
	
	Point mouseCoords;
	Input input;
	
	public Play(int state) {
		
	}

	@Override
	public void init(GameContainer gc, StateBasedGame sbg)
			throws SlickException {
		gc.setMouseCursor(new Image("data/crosshair.png"), 16, 16);
		controller = TanskController.getInstance();
		controller.newGame();
		playerOne = new Player("Player One");
		players = new ArrayList<Player>();
		players.add(playerOne);
		
		projectileSprite = new SpriteSheet("data/bullet.png", 5, 10);
		turretSprite = new SpriteSheet("data/turret.png", 45, 65);
		map = new Image("data/map.png");
		
		input = gc.getInput();
		input.addMouseListener(this);
		mouseCoords = new Point();
		turretSprite.setCenterOfRotation(playerOne.getTank().getTurret().getTurretCenter().x, playerOne.getTank().getTurret().getTurretCenter().y);
		tankSprite = new SpriteSheet("data/tank.png", 65,85);
		
		//Temporary to test switching between turrets
		turrets = new ArrayList<AbstractTurret>();
		turrets.add(new DefaultTurret());
		turrets.add(new FlamethrowerTurret());
		turrets.add(new ShotgunTurret());
		turrets.add(new SlowspeedyTurret());
		
		turretIndex = 0;

		obstacle = new Entity() {
		};
		obstacle.setShape(new Rectangle(75, 250, 40, 40));
	}

	@Override
	public void render(GameContainer container, StateBasedGame sbg, Graphics g)
			throws SlickException {
		map.draw();
//		tankSprite.draw(playerOne.getTank().getImagePosition().x, playerOne.getTank().getImagePosition().y);
		tankSprite.drawCentered(playerOne.getTank().getPosition().x, playerOne.getTank().getPosition().y);	
		turretSprite.draw(playerOne.getTank().getTurret().getImagePosition().x , playerOne.getTank().getTurret().getImagePosition().y);
		
		//Render projectiles:
		for(AbstractProjectile ap : playerOne.getTank().getProjectiles()){
			if(ap.isActive()){
				projectileSprite.setRotation((float)ap.getDirection().getTheta() + 90);
				projectileSprite.drawCentered(ap.getPosition().x, ap.getPosition().y);
			}
		}
		
		debugRender(g);
		
	}

	@Override
	public void update(GameContainer gc, StateBasedGame sbg, int delta)
			throws SlickException {
		if(input.isKeyDown(Input.KEY_W)){
			playerOne.getTank().accelerate(delta);
		} else if (input.isKeyDown(Input.KEY_S)){
			playerOne.getTank().reverse(delta);
		} else {
			playerOne.getTank().friction(delta);
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

		if(input.isMouseButtonDown(0)){
			playerOne.getTank().fireWeapon(delta);
		}
		
		if(input.isKeyDown(Input.KEY_Q)){
			turretSprite = new SpriteSheet("data/quaketurr.png", 45, 65);
			turretSprite.setCenterOfRotation(22.5f, 22.5f);
		}
		
		//Go back to the menu
		if(input.isKeyDown(Input.KEY_M) && input.isKeyDown(Input.KEY_LSHIFT)){
			sbg.enterState(0);
		}
		
		if(input.isKeyDown(Input.KEY_1)){
			Vector2f temp = playerOne.getTank().getTurret().getPosition();
			playerOne.getTank().setTurret(turrets.get(0));
			playerOne.getTank().getTurret().setPosition(temp);
		}if(input.isKeyDown(Input.KEY_2)){
			Vector2f temp = playerOne.getTank().getTurret().getPosition();
			playerOne.getTank().setTurret(turrets.get(1));
			playerOne.getTank().getTurret().setPosition(temp);
		}if(input.isKeyDown(Input.KEY_3)){
			Vector2f temp = playerOne.getTank().getTurret().getPosition();
			playerOne.getTank().setTurret(turrets.get(2));
			playerOne.getTank().getTurret().setPosition(temp);
		}if(input.isKeyDown(Input.KEY_4)){
			Vector2f temp = playerOne.getTank().getTurret().getPosition();
			playerOne.getTank().setTurret(turrets.get(3));
			playerOne.getTank().getTurret().setPosition(temp);
		}
		
		if(input.isKeyDown(Input.KEY_ESCAPE)){
			gc.exit();
		}
		
		controller.getWorld().update(delta);
      
		tankSprite.setRotation((float) (playerOne.getTank().getDirection().getTheta() + 90));	
        turretSprite.setRotation(playerOne.getTank().getTurret().getRotation());
        
        // Temporary, removes projectiles that are off screen
        List<AbstractProjectile> projs = playerOne.getTank().getProjectiles();
        for(int i = 0; i < projs.size(); i++){
        	AbstractProjectile proj = projs.get(i);
        	if(proj.isActive()){
        		proj.update(delta);
        		if(proj.getPosition().x > SCREEN_WIDTH || proj.getPosition().x < 0 || proj.getPosition().y > SCREEN_HEIGHT|| proj.getPosition().y < 0){
        			projs.remove(i);
        		}
        	} else {
        		projs.remove(i);
        	}
        }
		
	}

	@Override
	public int getID() {
		return 1;
	}

	public void render(GameContainer gc, Graphics g) throws SlickException {
		map.draw();
//		tankSprite.draw(playerOne.getTank().getImagePosition().x, playerOne.getTank().getImagePosition().y);
		tankSprite.drawCentered(playerOne.getTank().getPosition().x, playerOne.getTank().getPosition().y);	
		turretSprite.draw(playerOne.getTank().getTurret().getImagePosition().x , playerOne.getTank().getTurret().getImagePosition().y);

		g.setColor(Color.blue);
		g.draw(playerOne.getTank().getShape());

		g.setColor(Color.red);
		g.draw(obstacle.getShape());
		
		//Render projectiles:
		for(AbstractProjectile ap : playerOne.getTank().getProjectiles()){
			if(ap.isActive()){
				projectileSprite.setRotation((float)ap.getDirection().getTheta() + 90);
				projectileSprite.drawCentered(ap.getPosition().x, ap.getPosition().y);
			}
		}
		
		debugRender(g);
	}

	public void debugRender(Graphics g){
		g.setColor(Color.black);
		g.drawString("tankPosX:   " + playerOne.getTank().getPosition().x,  10, 30);
		g.drawString("tankPosY:   " + playerOne.getTank().getPosition().y,  10, 50);
		g.drawString("tankAng:    " + playerOne.getTank().getRotation(),	10, 70);
		g.drawString("tankImgAng: " + (tankSprite.getRotation()),			10, 90);

		g.drawString("turPosX:   " + playerOne.getTank().getTurret().getPosition().x, 300, 30);
		g.drawString("turPosY:   " + playerOne.getTank().getTurret().getPosition().y, 300, 50);
		g.drawString("turAng:    " + playerOne.getTank().getTurret().getRotation(),	  300, 70);
		g.drawString("turImgAng: " + turretSprite.getRotation(),		 			  300, 90);

		g.drawString("mouseX: " + mouseCoords.x, 530, 30);
		g.drawString("mouseY: " + mouseCoords.y, 530, 50);

		g.drawString("speed: " + Double.toString(playerOne.getTank().getSpeed()), 530, 90);
		g.drawString("projs: " + playerOne.getTank().getProjectiles().size(), 530, 130);

		
		if(!playerOne.getTank().getProjectiles().isEmpty()){
			g.drawString("projPos: "+playerOne.getTank()
				.getProjectiles().get(0).getPosition().x+" , "+playerOne.getTank()
				.getProjectiles().get(0).getPosition().y, 530, 110);
		}
	}

	public void mouseMoved(int oldx, int oldy, int newx, int newy){
		controller.setMouseCoordinates(newx, newy);
	}
	
	public void mouseDragged(int oldx, int oldy, int newx, int newy){
		controller.setMouseCoordinates(newx, newy);
	}
	
}
