package chalmers.TDA367.B17.states;

import java.awt.Dimension;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.Map;
import java.util.Map.Entry;

import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Image;
import org.newdawn.slick.Input;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.SpriteSheet;
import org.newdawn.slick.geom.Shape;
import org.newdawn.slick.geom.Transform;
import org.newdawn.slick.state.BasicGameState;
import org.newdawn.slick.state.StateBasedGame;

import chalmers.TDA367.B17.MapLoader;
import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.console.Console;
import chalmers.TDA367.B17.console.Console.OutputLevel;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.controller.ImageHandler;
import chalmers.TDA367.B17.model.AbstractSpawnPoint;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;
import chalmers.TDA367.B17.model.Entity;
import chalmers.TDA367.B17.model.Entity.RenderLayer;
import chalmers.TDA367.B17.model.MovableEntity;
import chalmers.TDA367.B17.model.Player;
import chalmers.TDA367.B17.model.World;

public class OldPlay extends BasicGameState{
	
	public ArrayList<AbstractTurret> turrets;
	private ArrayList<Player> players;
	private Player playerOne;
	private Image map = null;
	private Input input;
	private SpriteSheet entSprite = null;
	private int state;
	private GameController controller;
	private ImageHandler imgHandler;

	private int delta;
	
	public OldPlay(int state) {
	    this.state = state;
		controller = GameController.getInstance();
		controller.setConsole(new Console(10, 565, 450, 192, OutputLevel.ALL, false));
	}

	@Override
	public void init(GameContainer gc, StateBasedGame sbg) throws SlickException {
		gc.setAlwaysRender(true);
		gc.setMouseCursor(new Image("data/crosshair.png"), 16, 16);

		imgHandler = new ImageHandler();
		players = new ArrayList<Player>();
		
		map = new Image("data/map.png");
		
		input = gc.getInput();
		input.addMouseListener(this);
	}
	
	@Override
	public void enter(GameContainer container, StateBasedGame game) throws SlickException {
		super.enter(container, game);
		controller.setWorld(new World(new Dimension(Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT), true));
		controller.getWorld().init();
		imgHandler.loadAllImages(Tansk.DATA_FOLDER);
		MapLoader.createEntities("whatever");
		input = container.getInput(); 
		
		playerOne = new Player("Hello");
		players.add(playerOne);
		
		startGame();
	}
	
	// only used for debug
	private void startGame() {
//		int x = 100;
		
//		AbstractTank tank = new DefaultTank(GameController.getInstance().generateID());
//		tank.setPosition(new Vector2f(x, 100));
//		playerOne.setTank(tank);

		// Start a new round
//		gameConditions.init(10, 2, 1, 5000, 500000, 1500000);
//		gameConditions.newRoundDelayTimer(3000);
    }
	
	@Override
	public void update(GameContainer gc, StateBasedGame sbg, int delta)
			throws SlickException {
		this.delta = delta;
		if(input.isKeyDown(Input.KEY_W)){
			if(playerOne.getTank() != null)
				playerOne.getTank().accelerate(delta);
		} else if (input.isKeyDown(Input.KEY_S)){
			if(playerOne.getTank() != null)
			playerOne.getTank().reverse(delta);
		} else {
			if(playerOne.getTank() != null)
				playerOne.getTank().friction(delta);
		}

		if(input.isKeyDown(Input.KEY_A) && !input.isKeyDown(Input.KEY_D)){
			if(input.isKeyDown(Input.KEY_S)){
				if(playerOne.getTank() != null)
				playerOne.getTank().turnRight(delta);
			} else {
				if(playerOne.getTank() != null)
				playerOne.getTank().turnLeft(delta);
			}
		}

		if(input.isKeyDown(Input.KEY_D) && !input.isKeyDown(Input.KEY_A)){
			if(input.isKeyDown(Input.KEY_S)){
				if(playerOne.getTank() != null)
				playerOne.getTank().turnLeft(delta);
			} else {
				if(playerOne.getTank() != null)
				playerOne.getTank().turnRight(delta);
			}
		}

		if(input.isMouseButtonDown(0)){
			if(playerOne.getTank() != null)
				playerOne.getTank().fireWeapon(delta);
		}
		
		AbstractTurret turret = playerOne.getTank().getTurret();
		turret.setRotation((float) Math.toDegrees(Math.atan2(turret.getPosition().x - input.getMouseX() + 0, turret.getPosition().y - input.getMouseY() + 0)* -1)+180);		
		
		//Weapons
//		if(input.isKeyDown(Input.KEY_1)){
//			if(playerOne.getTank() != null)
//				playerOne.getTank().setTurret(new DefaultTurret(playerOne.getTank()));
//		}
//		if(input.isKeyDown(Input.KEY_2)){
//			if(playerOne.getTank() != null)
//				playerOne.getTank().setTurret(new FlamethrowerTurret(playerOne.getTank()));
//		}
//		if(input.isKeyDown(Input.KEY_3)){
//			if(playerOne.getTank() != null)
//				playerOne.getTank().setTurret(new ShotgunTurret(playerOne.getTank()));
//		}
//		if(input.isKeyDown(Input.KEY_4)){
//			if(playerOne.getTank() != null)
//				playerOne.getTank().setTurret(new SlowspeedyTurret(playerOne.getTank()));
//		}
//		if(input.isKeyDown(Input.KEY_5)){
//			if(playerOne.getTank() != null)
//				playerOne.getTank().setTurret(new ShockwaveTurret(playerOne.getTank()));
//		}
//		if(input.isKeyDown(Input.KEY_6)){
//			if(playerOne.getTank() != null)
//				playerOne.getTank().setTurret(new BounceTurret(playerOne.getTank()));
//		}
//		
//		if(input.isKeyDown(Input.KEY_ESCAPE)){
//			gc.exit();
//		}
		
//		//Update for tankspawner
//		controller.getWorld().getTankSpawner().update(delta);
//		
//		//Update for gameconditions
//		controller.gameConditions.update(delta);

		Iterator<Entry<Integer, Entity>> iterator = controller.getWorld().getEntities().entrySet().iterator();
		while(iterator.hasNext()){
			Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) iterator.next();
			Entity entity = entry.getValue();
			
			entity.update(delta);
			
			if(entity instanceof MovableEntity)
				controller.getWorld().checkCollisionsFor((MovableEntity)entity);
			if(entity instanceof AbstractSpawnPoint)
				controller.getWorld().checkCollisionsFor(entity);
		}
	}
	
	@Override
    public void render(GameContainer container, StateBasedGame game, Graphics g) throws SlickException {
		g.drawImage(map, 0, 0);
		g.drawString("Delta: " + delta, 18, 400);
		if(controller.getWorld().getEntities() != null){
			ArrayList<Entity> firstLayerEnts = new ArrayList<Entity>();
			ArrayList<Entity> secondLayerEnts = new ArrayList<Entity>();
			ArrayList<Entity> thirdLayerEnts = new ArrayList<Entity>();
			ArrayList<Entity> fourthLayerEnts = new ArrayList<Entity>();

			Iterator<Entry<Integer, Entity>> iterator = controller.getWorld().getEntities().entrySet().iterator();
			while(iterator.hasNext()){
				Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) iterator.next();
				Entity entity = entry.getValue();
				
				if(!entity.getSpriteID().equals("")){
					if(entity.getRenderLayer() == RenderLayer.FIRST)
						firstLayerEnts.add(entity);
					else if(entity.getRenderLayer() == RenderLayer.SECOND)
						secondLayerEnts.add(entity);
					else if(entity.getRenderLayer() == RenderLayer.THIRD)
						thirdLayerEnts.add(entity);
					else if(entity.getRenderLayer() == RenderLayer.FOURTH)
						fourthLayerEnts.add(entity);
				}
			}
			renderEntities(firstLayerEnts);
			renderEntities(secondLayerEnts);
			renderEntities(thirdLayerEnts);
			renderEntities(fourthLayerEnts);
		}
		controller.getConsole().renderMessages(g);
		debugRender(g);
    }
	
	private void renderEntities(ArrayList<Entity> entities){
		for(Entity entity : entities){
			entSprite = imgHandler.getSprite(entity.getSpriteID());
			
			if(entSprite != null){
				if(entity instanceof AbstractTank){
					entSprite = imgHandler.getSprite(entity.getSpriteID());
					if(entity.getRotation()!=0){
						entSprite.setRotation((float) entity.getRotation());
						// draw sprite at the coordinates of the top left corner of tank when it is not rotated
						Shape nonRotatedShape = entity.getShape().transform(Transform.createRotateTransform((float)Math.toRadians(-entity.getRotation()), entity.getPosition().x, entity.getPosition().y));
						entSprite.draw(nonRotatedShape.getMinX(), nonRotatedShape.getMinY());
					} else {
						entSprite.draw(entity.getShape().getMinX(), entity.getShape().getMinY());
					}
				} else {
					if(entity instanceof AbstractTurret){
						entSprite.setCenterOfRotation(((AbstractTurret) entity).getTurretCenter().x, ((AbstractTurret) entity).getTurretCenter().y);
					}
					entSprite.setRotation((float) entity.getRotation());
					entSprite.draw(entity.getSpritePosition().x, entity.getSpritePosition().y);						
				}
			}
		}
	}
	
	public void debugRender(Graphics g){
		g.drawString("Entities: " + controller.getWorld().getEntities().size(), 18, 545);
	}
	
	@Override
	public int getID() {
		return this.state;
	}
	
	public void mouseMoved(int oldx, int oldy, int newx, int newy){
//		controller.setMouseCoordinates(newx, newy);
	}
	
	public void mouseDragged(int oldx, int oldy, int newx, int newy){
//		controller.setMouseCoordinates(newx, newy);
	}
	
}
