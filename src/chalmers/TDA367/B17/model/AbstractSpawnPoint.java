package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public abstract class AbstractSpawnPoint extends Entity{
	
	//Used to check if the spawnpoint isn't colliding with anything 
	//preventing it from spawning.
	private boolean spawnable;

	/**
	 * Create a new AbstractSpawnPoint.
	 * @param position The position of this spawn-point.
	 */
	public AbstractSpawnPoint(int id, Vector2f position) {
		super(id);
		setPosition(position);
		spawnable = false;
		renderLayer = RenderLayer.FIRST;
	}
	
	/**
	 * Check if the spawn-point is blocked.
	 * @return True if this isn't blocked
	 */
	public boolean isSpawnable(){
		return this.spawnable;
	}
	
	/**
	 * Set the state of spawnable.
	 * @param bool The new state
	 */
	public void setSpawnable(boolean bool){
		spawnable = bool;
	}
	
	@Override
	public abstract void update(int delta);
}
