package chalmers.TDA367.B17;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.spawnpoints.TankSpawnPoint;
import chalmers.TDA367.B17.terrain.BrownWall;

/** Loads static entities from the map
 *  //TODO no hard coding!
 */
public class MapLoader {

	public static void createEntities(String mapName){
		// TODO: no hard code
		// should load dynamically given mapname	
		
		new BrownWall(GameController.getInstance().generateID(), new Vector2f(150, 50), new Vector2f(700, 600));
		
		//TankSpawnPoints
		TankSpawnPoint tsp = new TankSpawnPoint(GameController.getInstance().generateID(), new Vector2f(100, 100));
		tsp.setRotation(315);
		tsp = new TankSpawnPoint(GameController.getInstance().generateID(), new Vector2f(900, 100));
		tsp.setRotation(45);
		tsp = new TankSpawnPoint(GameController.getInstance().generateID(), new Vector2f(100, 650));
		tsp.setRotation(225);
		tsp = new TankSpawnPoint(GameController.getInstance().generateID(), new Vector2f(900, 650));
		tsp.setRotation(135);
	}
}
