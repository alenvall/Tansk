package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public abstract class AbstractTurret extends Entity {
	protected float angle;
	protected Vector2f turretCenter;
	protected float turretLength;
	protected int fireRate;

	public AbstractTurret() {
		super();
	}
	
	public int getFireRate(){
		return fireRate;
	}

	public Vector2f getImagePosition(){
		return new Vector2f(position.x - turretCenter.x, position.y - turretCenter.y);
	}

	public float getRotation() {
	    return angle;
    }

	public void setRotation(float angle) {
		this.angle = angle % 360.0f;
    }
	
	public Vector2f getTurretCenter(){
		return new Vector2f(turretCenter);
	}
	
	public Vector2f getTurretNozzle(){
		double turretRotation = getRotation(); 
		float nozzleX = (float) (position.x - turretLength * Math.sin(Math.toRadians(turretRotation + 0)));
		float nozzleY = (float) (position.y + turretLength * Math.cos(Math.toRadians(turretRotation )));
		
		return new Vector2f(nozzleX, nozzleY);
	}
	
	
}