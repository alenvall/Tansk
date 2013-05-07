package chalmers.TDA367.B17.states;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.Map;
import java.util.Map.*;

import org.newdawn.slick.Color;
import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Image;
import org.newdawn.slick.Input;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.geom.Vector2f;
import org.newdawn.slick.state.BasicGameState;
import org.newdawn.slick.state.StateBasedGame;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractSpawnPoint;
import chalmers.TDA367.B17.model.Entity;
import chalmers.TDA367.B17.model.MovableEntity;
import chalmers.TDA367.B17.model.Player;
import chalmers.TDA367.B17.network.GameServer;
import chalmers.TDA367.B17.spawnpoints.PowerUpSpawnPoint;
import chalmers.TDA367.B17.spawnpoints.TankSpawnPoint;
import chalmers.TDA367.B17.terrain.BrownWall;

public class Server extends BasicGameState {
	private int state;
	private GameServer server;
	private GameController controller;
	private ArrayList<Player> players;
	private Input input;
	
	public Server(int state) {
	    this.state = state;
	    
    }
	
	@Override
    public void init(GameContainer gc, StateBasedGame game) throws SlickException {
		
		gc.setMouseCursor(new Image("data/crosshair.png"), 16, 16);
		controller = GameController.getInstance();

		controller.newGame(GameController.SCREEN_WIDTH, GameController.SCREEN_HEIGHT, 10, 2, 1, 5000, 500000, 1500000);

		//Players
		Player playerOne = new Player("Player One");
		players = new ArrayList<Player>();
		players.add(playerOne);
		
/*		playerTwo = new Player("Player Two");
		players.add(playerTwo);
		
		playerTwo.getTank().setPosition(new Vector2f(800, 200));
		playerTwo.getTank().setFriction(0);
		playerTwo.getTank().setSpeed(0.25f);
		
		
		playerThree = new Player("Player Three");
		players.add(playerThree);
		//playerThree.getTank().setPosition(new Vector2f(800, 500));
		
		playerFour = new Player("Player Four");
		players.add(playerFour);
		//playerFour.getTank().setPosition(new Vector2f(200, 500));
*/	
		input = gc.getInput();
		input.addMouseListener(this);
		
		//ObstacleTest
		new BrownWall(new Vector2f(150, 50), new Vector2f(700, 600));
		
		//PowerUpSpawnPoints
		new PowerUpSpawnPoint(new Vector2f(250, 100), 10000, "shield");
		new PowerUpSpawnPoint(new Vector2f(250, 500), 10000, "speed");
		new PowerUpSpawnPoint(new Vector2f(500, 100), 10000, "");
		
		//TankSpawnPoints
		new TankSpawnPoint(new Vector2f(100, 100));
		new TankSpawnPoint(new Vector2f(900, 100));
		new TankSpawnPoint(new Vector2f(100, 500));
		new TankSpawnPoint(new Vector2f(900, 500));

    }
	
	@Override
	public void enter(GameContainer container, StateBasedGame game) throws SlickException {
		super.enter(container, game);
		
		ArrayList<Entity> entityList = new ArrayList<Entity>();
		Iterator<Entry<Integer, Entity>> iterator = controller.getWorld().getEntities().entrySet().iterator();
		while(iterator.hasNext()){
			Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) iterator.next();
			Entity entity = entry.getValue();
			entityList.add(entity);
		}
		server.setEntityList(entityList);	



			
		//Start a new round
		controller.gameConditions.newRoundDelayTimer(3000);
	}
	
	@Override
    public void update(GameContainer container, StateBasedGame game, int delta) throws SlickException {
		//Update for tankspawner
		controller.getWorld().getTankSpawner().update(delta);
		
		//Update for gameconditions
		controller.gameConditions.update(delta);
		
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
		g.setColor(Color.blue);
		g.drawString("Running!", 120, 180);
	}
	
	@Override
    public int getID() {
	    return this.state;
    }

	public void setServer(GameServer server) {
	    this.server = server;	    
    }
}
