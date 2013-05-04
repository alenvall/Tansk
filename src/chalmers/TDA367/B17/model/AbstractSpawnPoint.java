package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public abstract class AbstractSpawnPoint extends Entity{
	
	private boolean spawnable;

	public AbstractSpawnPoint(Vector2f position) {
		setPosition(position);
		spawnable = false;
	}
	
	public boolean isSpawnable(){
		return this.spawnable;
	}
	
	public void setSpawnable(boolean bool){
		spawnable = bool;
	}
	
	public void spawnEntity(){}
	
	@Override
	public abstract void update(int delta);
}
