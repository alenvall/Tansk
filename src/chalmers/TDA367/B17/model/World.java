package chalmers.TDA367.B17.model;

import java.awt.Dimension;
import java.util.*;
import java.util.Map.Entry;
import java.util.concurrent.ConcurrentHashMap;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.sound.SoundHandler;
import chalmers.TDA367.B17.spawnpoints.Spawner;


public class World {
	
	//A map holding all entities with an ID
	private Map<Integer, Entity> entities;
	//The size of the map (world)
	private Dimension size;
	
	//A spawner that spawns/respawns tanks.
	private TankSpawner tankSpawner;
	
	//A spawner that spawns powerup and weapons.
	private Spawner spawner;

	private int latestID;
	
	/**
	 * Create a new World.
	 * @param size The size of the world
	 */
	public World(Dimension size) {
		this.size = size;
		this.entities = new ConcurrentHashMap<Integer, Entity>();
		this.tankSpawner = new TankSpawner();
		spawner = new Spawner();
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

			if(entity!=paramEntity && (entity.getShape().intersects(paramEntity.getShape())
					|| (entity.getShape().contains(paramEntity.getShape()) && !(entity instanceof MapBounds)))){
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
	
	/**
	 * Get the spawner of this world.
	 * @return The spawner
	 */
	public Spawner getSpawner() {
		return spawner;
	}

	public void handleEvent(GameEvent event){
		GameController.getInstance().getSoundHandler().handleEvent(event);
	}
}
