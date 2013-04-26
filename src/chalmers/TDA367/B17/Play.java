package chalmers.TDA367.B17;

import org.newdawn.slick.*;

import chalmers.TDA367.B17.controller.*;
import chalmers.TDA367.B17.model.*;
import chalmers.TDA367.B17.powerup.*;
import chalmers.TDA367.B17.weapons.*;

import org.newdawn.slick.geom.*;
import org.newdawn.slick.state.*;
import java.awt.Point;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.Map;
import java.util.Map.Entry;

public class Play extends BasicGameState{
	
	public ArrayList<AbstractTurret> turrets;
	
	private TanskController controller;
	private ArrayList<Player> players;
	private Player playerOne;
	private Entity obstacle;
	private Image map = null;
	private Point mouseCoords;
	private Input input;
	private SpriteSheet entSprite = null;
	
	public Play(int state) {
		
	}

	@Override
	public void init(GameContainer gc, StateBasedGame sbg) throws SlickException {
		gc.setMouseCursor(new Image("data/crosshair.png"), 16, 16);
		controller = TanskController.getInstance();

		controller.newGame(TanskController.SCREEN_WIDTH, TanskController.SCREEN_HEIGHT);

		playerOne = new Player("Player One");
		players = new ArrayList<Player>();
		players.add(playerOne);
		
		map = new Image("data/map.png");
		
		input = gc.getInput();
		input.addMouseListener(this);
		mouseCoords = new Point();


	//	turretSprite.setCenterOfRotation(playerOne.getTank().getTurret().getTurretCenter().x, playerOne.getTank().getTurret().getTurretCenter().y);

	//	obstacle = new Entity() {};
	//	obstacle.setShape(new Rectangle(75, 250, 40, 40));
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
			//turretSprite = new SpriteSheet("data/quaketurr.png", 45, 65);
			//turretSprite.setCenterOfRotation(22.5f, 22.5f);
		}
		
		//Go back to the menu
		if(input.isKeyDown(Input.KEY_M) && input.isKeyDown(Input.KEY_LSHIFT)){
			sbg.enterState(0);
		}
		
		//Weapons
		if(input.isKeyDown(Input.KEY_1)){
			Vector2f temp = playerOne.getTank().getTurret().getPosition();
			playerOne.getTank().getTurret().destroy();
			playerOne.getTank().setTurret(new DefaultTurret(playerOne.getTank()));
			playerOne.getTank().getTurret().setPosition(temp);
		}
		if(input.isKeyDown(Input.KEY_2)){
			Vector2f temp = playerOne.getTank().getTurret().getPosition();
			playerOne.getTank().getTurret().destroy();
			playerOne.getTank().setTurret(new FlamethrowerTurret(playerOne.getTank()));
			playerOne.getTank().getTurret().setPosition(temp);
		}
		if(input.isKeyDown(Input.KEY_3)){
			Vector2f temp = playerOne.getTank().getTurret().getPosition();
			playerOne.getTank().getTurret().destroy();
			playerOne.getTank().setTurret(new ShotgunTurret(playerOne.getTank()));
			playerOne.getTank().getTurret().setPosition(temp);
		}
		if(input.isKeyDown(Input.KEY_4)){
			Vector2f temp = playerOne.getTank().getTurret().getPosition();
			playerOne.getTank().getTurret().destroy();
			playerOne.getTank().setTurret(new SlowspeedyTurret(playerOne.getTank()));
			playerOne.getTank().getTurret().setPosition(temp);
		}
		
		//Powerups
		if(input.isKeyDown(Input.KEY_COMMA)){
			SpeedPowerUp spu = new SpeedPowerUp();
			playerOne.getTank().setCurrentPowerUp(spu);
		}if(input.isKeyDown(Input.KEY_PERIOD)){
			FireRatePowerUp spu = new FireRatePowerUp();
			playerOne.getTank().setCurrentPowerUp(spu);
		}
		
		if(input.isKeyDown(Input.KEY_ESCAPE)){
			gc.exit();
		}
		
		Iterator<Entry<Integer, Entity>> iterator = controller.getWorld().getEntities().entrySet().iterator();
		while(iterator.hasNext()){
			Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) iterator.next();
			Entity entity = entry.getValue();
			entity.update(delta);

			if(entity instanceof MovableEntity)
				if(((MovableEntity) entity).getSpeed() != 0)
					controller.getWorld().checkCollisionsFor((MovableEntity)entity);			
		}
	}
	
	@Override
	public void render(GameContainer container, StateBasedGame sbg, Graphics g) throws SlickException {	
		map.draw();	
		
		Iterator<Entry<Integer, Entity>> iterator = controller.getWorld().getEntities().entrySet().iterator();
		while(iterator.hasNext()){
			Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) iterator.next();
			Entity entity = entry.getValue();
			
			if(!entity.getSpriteID().equals("")){
				entSprite = TanskController.getInstance().getImageHandler().getSprite(entity.getSpriteID());
				
				if(entSprite != null){
					if(entity instanceof AbstractTurret){
						entSprite.setCenterOfRotation(entity.getCenter().x, entity.getCenter().y);
					}
					entSprite.setRotation((float) entity.getRotation());
					entSprite.draw(entity.getSpritePosition().x, entity.getSpritePosition().y);	
				}
			}
		}
		debugRender(g);
	}

	@Override
	public int getID() {
		return 1;
	}

	public void debugRender(Graphics g){
		g.setColor(Color.black);
		g.drawString("tankPosX:   " + playerOne.getTank().getPosition().x,  10, 30);
		g.drawString("tankPosY:   " + playerOne.getTank().getPosition().y,  10, 50);
		g.drawString("tankAng:    " + playerOne.getTank().getRotation(),	10, 70);
//		g.drawString("tankImgAng: " + (tankSprite.getRotation()),			10, 90);

		g.drawString("turPosX:   " + playerOne.getTank().getTurret().getPosition().x, 300, 30);
		g.drawString("turPosY:   " + playerOne.getTank().getTurret().getPosition().y, 300, 50);
		g.drawString("turAng:    " + playerOne.getTank().getTurret().getRotation(),	  300, 70);
//		g.drawString("turImgAng: " + turretSprite.getRotation(),		 			  300, 90);

		g.drawString("mouseX: " + mouseCoords.x, 530, 30);
		g.drawString("mouseY: " + mouseCoords.y, 530, 50);

		g.drawString("speed: " + Double.toString(playerOne.getTank().getSpeed()), 530, 90);
		g.drawString("projs: " + playerOne.getTank().getProjectiles().size(), 530, 130);
	
	//	g.setColor(Color.blue);
	//	g.draw(playerOne.getTank().getShape());

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
