package chalmers.TDA367.B17;

import java.awt.Dimension;
import java.util.ArrayList;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.Entity;
import chalmers.TDA367.B17.model.MapBounds;
import chalmers.TDA367.B17.spawnpoints.PowerUpSpawnPoint;
import chalmers.TDA367.B17.spawnpoints.TankSpawnPoint;
import chalmers.TDA367.B17.terrain.BrownWall;

/** Loads static entities from the map
 *  //TODO no hard coding!
 */
public class MapLoader {

	public static void createEntities(String mapName){
		// TODO: no hard code
		// should load dynamically given mapname	
		ArrayList<Entity> entityList = new ArrayList<Entity>();
		
		entityList.add(new MapBounds(GameController.getInstance().generateID(), new Dimension(Tansk.SCREEN_WIDTH, Tansk.SCREEN_HEIGHT)));
		
		//ObstacleTest
		entityList.add(new BrownWall(GameController.getInstance().generateID(), new Vector2f(150, 50), new Vector2f(700, 600)));
		
		//PowerUpSpawnPoints
		entityList.add(new PowerUpSpawnPoint(GameController.getInstance().generateID(), new Vector2f(250, 100), 10000, "shield"));
		entityList.add(new PowerUpSpawnPoint(GameController.getInstance().generateID(), new Vector2f(250, 500), 10000, "speed"));
		entityList.add(new PowerUpSpawnPoint(GameController.getInstance().generateID(), new Vector2f(500, 100), 10000, ""));
		
		//TankSpawnPoints
		entityList.add(new TankSpawnPoint(GameController.getInstance().generateID(), new Vector2f(100, 100)));
		entityList.add(new TankSpawnPoint(GameController.getInstance().generateID(), new Vector2f(900, 100)));
		entityList.add(new TankSpawnPoint(GameController.getInstance().generateID(), new Vector2f(100, 500)));
		entityList.add(new TankSpawnPoint(GameController.getInstance().generateID(), new Vector2f(900, 500)));	
	}
}
