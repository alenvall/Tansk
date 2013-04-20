package chalmers.TDA367.B17.model;

import java.util.*;

public class World {
	
	private Map<Integer, Entity> entities = new HashMap<Integer, Entity>();
	
	private static int latestID;
	
	public World() {
		
	}
	
	public void addEntity(Entity newEntity){
		entities.put(newEntity.getId(), newEntity);
	}
	
	public Entity getEntity(int id){
		return entities.get(id);
	}
	
	public static int generateID(){
		latestID += 1;
		return latestID;
	}
}
