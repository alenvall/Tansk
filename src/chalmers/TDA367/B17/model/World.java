package chalmers.TDA367.B17.model;

import java.awt.Dimension;
import java.util.*;
import java.util.Map.Entry;
import java.util.concurrent.ConcurrentHashMap;


public class World {
	
	//A map holding all entities with an ID
	private Map<Integer, Entity> entities;
	//The size of the map (world)
	private Dimension size;
	
	private TankSpawner tankSpawner;
	
	private int latestID;
	
	public World(Dimension size) {
		this.size = size;
		this.entities = new ConcurrentHashMap<Integer, Entity>();
		this.tankSpawner = new TankSpawner();
	}

	public void init(){
		new MapBounds(getSize());
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
	 * @return entity.
	 */
	public Entity getEntity(int id){
		return entities.get(id);
	}
	
	/**
	 * Generate a new ID.
	 * @return The ID
	 */
	public int generateID(){
		latestID += 1;
		return latestID;
	}
	
	/**
	 * Return the entities in the world.
	 * @return entities in world.
	 */
	public Map<Integer, Entity> getEntities(){
		return entities;
	}

	public void checkCollisionsFor(Entity movableEntity){
		Iterator<Entry<Integer, Entity>> iterator = entities.entrySet().iterator();
		while(iterator.hasNext()){
			Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) iterator.next();
			Entity entity = entry.getValue();

			if(entity!=movableEntity && entity.getShape().intersects(movableEntity.getShape())){
				movableEntity.didCollideWith(entity);
				// prevent double method calls
				if(!(entity instanceof MovableEntity))
					entity.didCollideWith(movableEntity);
			}
		}
	}

	public void removeEntity(Entity entity){
		entities.remove(entity.getId());
	}

	public Dimension getSize() {
		return size;
	}

	public TankSpawner getTankSpawner() {
		return tankSpawner;
	}

	public void setTankSpawner(TankSpawner tankSpawner) {
		this.tankSpawner = tankSpawner;
	}
}
