package chalmers.TDA367.B17;

import java.io.File;
import java.io.FileNotFoundException;
import java.util.Scanner;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.console.Console.MsgLevel;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.spawnpoints.TankSpawnPoint;
import chalmers.TDA367.B17.terrain.HorizontalWall;
import chalmers.TDA367.B17.terrain.VerticalWall;

public class MapLoader {

	public static void createEntities(String mapName){
		Scanner scanner = null;
		try {
	        scanner = new Scanner(new File(Tansk.DATA_FOLDER + "/" + mapName + ".map"));
        } catch (FileNotFoundException e) {
	        System.out.println("Couldn't find map: " + mapName + ".map");
	        GameController.getInstance().getConsole().addMsg("Couldn't find map: " + mapName + ".map", MsgLevel.INFO);
	        e.printStackTrace();
        }
		
		if(scanner != null){
			while (scanner.hasNextLine()) {
				String nextLine = scanner.nextLine();        
		         
				if (nextLine.startsWith("+")){
		        	String[] params = nextLine.substring(1).split(", ");
		        	
		       		String entityKind = params[0].trim();
		       		String posX = params[1].trim();
		       		String posY = params[2].trim();

		        	String rotation = "";
		        	if(params.length > 3)
		        		rotation = params[3].trim();
		         
		        	if(entityKind.equals("1")){
		        		new HorizontalWall(GameController.getInstance().generateID(), new Vector2f(Integer.parseInt(posX), Integer.parseInt(posY)));
		        	} else if(entityKind.equals("2")){
		        		new VerticalWall(GameController.getInstance().generateID(), new Vector2f(Integer.parseInt(posX), Integer.parseInt(posY)));
		        	} else if(entityKind.equals("3")){
		        		TankSpawnPoint tsp = new TankSpawnPoint(GameController.getInstance().generateID(), new Vector2f(Integer.parseInt(posX), Integer.parseInt(posY)));
		        		tsp.setRotation(Double.parseDouble(rotation)); 
		        	}
				} else {
					// line doesn't start with '+' and is not a valid "entity creation line"
					continue;
				}
			}
		}
	}
}
