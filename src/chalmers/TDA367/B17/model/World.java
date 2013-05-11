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
	
	/**
	 * Create a new World.
	 * @param size The size of the world
	 */
	public World(Dimension size) {
		this.size = size;
		this.entities = new ConcurrentHashMap<Integer, Entity>();
		this.tankSpawner = new TankSpawner();
	}

	/**
	 * Initiate the world, giving it a MapBounds for border collision.
	 */
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

	/**
	 * Checks for collision between all entities that the world contains. Calls
	 * the objects respective "didCollideWith"-methods if any entities intersects.
	 * @param paramEntity The entity to check collision for
	 */
	public void checkCollisionsFor(Entity paramEntity){
		Iterator<Entry<Integer, Entity>> iterator = entities.entrySet().iterator();
		while(iterator.hasNext()){
			Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) iterator.next();
			Entity entity = entry.getValue();

			if(entity!=paramEntity && entity.getShape().intersects(paramEntity.getShape())){
				paramEntity.didCollideWith(entity);
				// prevent double method calls
				if(!(entity instanceof MovableEntity))
					entity.didCollideWith(paramEntity);
			}
		}
	}

	/**
	 * Remove an entity from the world.
	 * @param entity The entity to be removed
	 */
	public void removeEntity(Entity entity){
		entities.remove(entity.getId());
	}

	/**
	 * Get the size of the world.
	 * @return The size of the world
	 */
	public Dimension getSize() {
		return size;
	}

	/**
	 * Get the TankSpawner of this world.
	 * @return The TankSpawner of this world
	 */
	public TankSpawner getTankSpawner() {
		return tankSpawner;
	}

	/**
	 * Set the TankSpawner of this world.
	 * @param tankSpawner The new TankSpawner
	 */
	public void setTankSpawner(TankSpawner tankSpawner) {
		this.tankSpawner = tankSpawner;
	}
}
