package chalmers.TDA367.B17.model;

import java.util.*;

public class World {
	
	private Map<Integer, Entity> entities = new HashMap<Integer, Entity>();
	
	private static int latestID;
	
	public World() {
		
	}
	/**
	 * Adds a new entity.
	 * @param newEntity the entity to add
	 */
	public void addEntity(Entity newEntity){
		entities.put(newEntity.getId(), newEntity);
	}
	
	/**
	 * Get an entity from the World.
	 * @param id The id of the requested entity
	 * @return
	 */
	public Entity getEntity(int id){
		return entities.get(id);
	}
	
	/**
	 * Generate a new ID.
	 * @return The ID
	 */
	public static int generateID(){
		latestID += 1;
		return latestID;
	}
}
