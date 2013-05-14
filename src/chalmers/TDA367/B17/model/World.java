package chalmers.TDA367.B17.model;

import java.awt.Dimension;
import java.util.*;
import java.util.Map.Entry;
import java.util.concurrent.ConcurrentHashMap;
import chalmers.TDA367.B17.console.Console.MsgLevel;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.network.Network.*;
import chalmers.TDA367.B17.states.ServerState;

public class World {
	private Map<Integer, Entity> entities;
	private Dimension size;
	private TankSpawner tankSpawner;
	private boolean serverWorld = false;
	Entity entity;
	
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
			GameController.getInstance().getConsole().addMsg("Created (ID" + newEntity.getId() + "): " + newEntity.getClass().getSimpleName(), MsgLevel.STANDARD);
			Pck9_EntityCreated packet = new Pck9_EntityCreated();
			packet.entityID = newEntity.getId();
			packet.identifier = newEntity.getClass().getSimpleName();
			ServerState.getInstance().sendToAll(packet);
//			ServerState.getInstance().addToClientQueue(packet);
		}
		entities.put(newEntity.getId(), newEntity);
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

	public void removeEntity(Entity entity){
		removeEntity(entity.getId());
	}	
	
	public void removeEntity(int id){
		if(serverWorld){
			try{
				entity = getEntity(id);
				String msg = "Destroyed (ID" + id + "): " + entity.getClass().getSimpleName();
				GameController.getInstance().getConsole().addMsg(msg, MsgLevel.STANDARD);
			} catch (java.lang.NullPointerException e){
				GameController.getInstance().getConsole().addMsg("Shiiiiet (ID" + id + ")", MsgLevel.ERROR);
			}
			Pck8_EntityDestroyed pck = new Pck8_EntityDestroyed();
			pck.entityID = id;
			ServerState.getInstance().sendToAll(pck);
//			ServerState.getInstance().addToClientQueue(pck);
		}
		
		entities.remove(id);
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
