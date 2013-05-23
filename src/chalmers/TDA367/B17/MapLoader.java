package chalmers.TDA367.B17;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.spawnpoints.TankSpawnPoint;
import chalmers.TDA367.B17.terrain.HorizontalWall;
import chalmers.TDA367.B17.terrain.VerticalWall;

/** Loads static entities from the map
 *  //TODO no hard coding!
 */
public class MapLoader {

	public static void createEntities(String mapName){
		// TODO: no hard code
		// should load dynamically given mapname	
	
//		new BrownWall(GameController.getInstance().generateID(), new Vector2f(150, 50), new Vector2f(700, 600));
		
		new HorizontalWall(GameController.getInstance().generateID(), new Vector2f(225, 256)); 
		new HorizontalWall(GameController.getInstance().generateID(), new Vector2f(75, 384));
		new HorizontalWall(GameController.getInstance().generateID(), new Vector2f(225, 512));

		new VerticalWall(GameController.getInstance().generateID(), new Vector2f(386, 384));
		
		new VerticalWall(GameController.getInstance().generateID(), new Vector2f(256, 225));
		new VerticalWall(GameController.getInstance().generateID(), new Vector2f(386, 75));
		new VerticalWall(GameController.getInstance().generateID(), new Vector2f(638, 75));
		new VerticalWall(GameController.getInstance().generateID(), new Vector2f(768, 225));
		
		new HorizontalWall(GameController.getInstance().generateID(), new Vector2f(799, 256));
		new HorizontalWall(GameController.getInstance().generateID(), new Vector2f(949, 387));
		new HorizontalWall(GameController.getInstance().generateID(), new Vector2f(799, 512));
		
		new VerticalWall(GameController.getInstance().generateID(), new Vector2f(638, 384));
				
		new VerticalWall(GameController.getInstance().generateID(), new Vector2f(256, 543));
		new VerticalWall(GameController.getInstance().generateID(), new Vector2f(386, 693));
		new VerticalWall(GameController.getInstance().generateID(), new Vector2f(638, 693));
		new VerticalWall(GameController.getInstance().generateID(), new Vector2f(768, 543));

		
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
