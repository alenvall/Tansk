package chalmers.TDA367.B17.model;

import java.awt.Dimension;
import java.util.*;
import java.util.Map.Entry;
import java.util.concurrent.ConcurrentHashMap;

import chalmers.TDA367.B17.console.Console.MsgLevel;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.network.Network.*;
import chalmers.TDA367.B17.spawnpoints.PowerUpSpawnPoint;
import chalmers.TDA367.B17.spawnpoints.TankSpawnPoint;
import chalmers.TDA367.B17.states.ServerState;
import chalmers.TDA367.B17.terrain.BrownWall;

public class World {
	private Map<Integer, Entity> entities;
	private Dimension size;
	private TankSpawner tankSpawner;
	private boolean serverWorld = false;
	
	public World(Dimension size, boolean serverWorld) {
		this.size = size;
		this.serverWorld = serverWorld;
		this.entities = new ConcurrentHashMap<Integer, Entity>();
		this.tankSpawner = new TankSpawner();
	}

	public void init(){
//		new MapBounds(getSize());
	}

	/**
	 * Adds a new entity.
	 * @param newEntity the entity to add
	 */
	public void addEntity(Entity newEntity){
		if(serverWorld){
			if(!(newEntity instanceof MapBounds) || !(newEntity instanceof BrownWall) || !(newEntity instanceof PowerUpSpawnPoint) || !(newEntity instanceof TankSpawnPoint)){
				GameController.getInstance().getConsole().addMsg("Created (ID" + newEntity.getId() + "): "+  newEntity.getClass().getSimpleName(), MsgLevel.STANDARD);
				Pck9_EntityCreated packet = new Pck9_EntityCreated();
				packet.entityID = newEntity.getId();
				packet.identifier = newEntity.getClass().getSimpleName();
//				ServerState.getInstance().sendToAll(packet);
				ServerState.getInstance().addToAllClientsQueue(packet);
			}
		}
		entities.put(newEntity.getId(), newEntity);
	}

	public void removeEntity(Entity entity){
		removeEntity(entity.getId());
	}	
	
	public void removeEntity(int id){
		if(serverWorld){
			Entity deadEntity = getEntity(id);
			GameController.getInstance().getConsole().addMsg("Destroyed (ID" + deadEntity.getId() + "): "+  deadEntity.getClass().getSimpleName(), MsgLevel.STANDARD);
			Pck8_EntityDestroyed pck = new Pck8_EntityDestroyed();
			pck.entityID = id;
//			ServerState.getInstance().sendToAll(pck);
			ServerState.getInstance().addToAllClientsQueue(pck);
		}
		
		entities.remove(id);
	}
	
	/**
	 * Get an entity from the World.
	 * @param id The id of the requested entity
	 * @return entity
	 */
	public Entity getEntity(int id){
		return entities.get(id);
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
