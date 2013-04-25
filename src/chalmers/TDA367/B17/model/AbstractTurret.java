package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public abstract class AbstractTurret extends Entity {
	protected float angle;
	protected Vector2f turretCenter;
	protected float turretLength;
	protected int fireRate;
	protected String projectileType;
	protected AbstractTank tank;

	/**
	 * Create a new AbstractTurret.
	 */
	public AbstractTurret(AbstractTank tank) {
		super();
		this.tank = tank;
	}
	
	public AbstractTank getTank(){
		return tank;
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
	
	public void setFireRate(int fireRate) {
		this.fireRate = fireRate;
	}

	/**
	 * Shoots projectiles.
	 * @param delta time in milliseconds since last game update
	 * @param tank the tank (owner of this turret) shooting
	 */
	public abstract void fireWeapon(int delta, AbstractTank tank);

	/**
	 * Defines which type of projectiles to create.
	 * @return a new projectile
	 */
	public abstract AbstractProjectile createProjectile();

	/**
	 * Creates and spawns a new projectile.
	 * @return the new projectile
	 */
	public AbstractProjectile spawnNewProjectile(){
		AbstractProjectile projectile = createProjectile();
		projectile.setDirection(new Vector2f(getRotation() + 90));
		projectile.setPosition(getTurretNozzle());
		return projectile;
	}
}