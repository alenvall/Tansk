package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public abstract class AbstractTurret extends Entity {
	protected float angle;
	protected Vector2f turretCenter;
	protected float turretLength;
	protected int fireRate;

	/**
	 * Create a new AbstractTurret.
	 */
	public AbstractTurret() {
		super();
	}

	/**
	 * Get the fire rate.
	 * @return the delay between shots in milliseconds
	 */
	public int getFireRate(){
		return fireRate;
	}

	/**
	 * Get the turret image position.
	 * That is the top left corner of the turret.
	 * @return a new vector with the coordinates of that top left corner
	 */
	public Vector2f getImagePosition(){
		return new Vector2f(position.x - turretCenter.x, position.y - turretCenter.y);
	}

	/**
	 * Get the rotation angle of the turret.
	 * @return the angle
	 */
	public float getRotation() {
	    return angle;
    }

	/**
	 * Set the rotation angle of the turret.
	 * @param angle the new angle
	 */
	public void setRotation(float angle) {
		this.angle = angle % 360.0f;
    }

	/**
	 * Get the turret center position represented by a vector.
	 * @return a new vector with the coordinates of the center of the turret
	 */
	public Vector2f getTurretCenter(){
		return new Vector2f(turretCenter);
	}

	/**
	 * Get the position of the nozzle of the turret.
	 * @return the position of the nozzle
	 */
	public Vector2f getTurretNozzle(){
		double turretRotation = getRotation(); 
		float nozzleX = (float) (position.x - turretLength * Math.sin(Math.toRadians(turretRotation + 0)));
		float nozzleY = (float) (position.y + turretLength * Math.cos(Math.toRadians(turretRotation )));
		
		return new Vector2f(nozzleX, nozzleY);
	}
	
	
}