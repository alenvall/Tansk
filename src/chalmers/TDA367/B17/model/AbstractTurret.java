package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Point;
import org.newdawn.slick.geom.Vector2f;

/**
 * AbstractTurret is the superclass to all turrets.
 */
public abstract class AbstractTurret extends Entity {
	private double rotation;
	protected Vector2f turretCenter;
	protected float turretLength;
	protected int fireRate;
	protected String projectileType;
	protected AbstractTank tank;
	private String color;
	
	/**
	 * Create a new AbstractTurret.
	 * @param id The id
	 * @param position The position
	 * @param startingRotation The starting rotation
	 * @param tank The tank
	 * @param color The color
	 */
	public AbstractTurret(int id, Vector2f position, double startingRotation, AbstractTank tank, String color) {
		super(id);
		rotation = startingRotation;
		this.tank = tank;
		setShape(new Point(position.x, position.y));
		renderLayer = RenderLayer.THIRD;
		this.color = color;
		setSpriteID("turret_" + color);
	}
	
	/**
	 * Get the tank of this turret.
	 * @return The tank
	 */
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
	
	@Override
	public Vector2f getSpritePosition(){
		return new Vector2f(getPosition().x - turretCenter.x, getPosition().y - turretCenter.y);
	}
	
	@Override
	public double getRotation() {
	    return rotation;
    }

	/**
	 * Set the rotation angle of the turret.
	 * @param angle the new angle
	 */
	public void setRotation(float angle) {
		this.rotation = angle % 360.0f;
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
		float nozzleX = (float) (getPosition().x - turretLength * Math.sin(Math.toRadians(turretRotation + 0)));
		float nozzleY = (float) (getPosition().y + turretLength * Math.cos(Math.toRadians(turretRotation )));
		
		return new Vector2f(nozzleX, nozzleY);
	}
	
	/**
	 * Set the fire rate of the turret. (the amount of time between each shot.)
	 * @param fireRate
	 */
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
	
	/**
	 * Get the color of this turret.
	 * @return The color
	 */
	public String getColor(){
		return color;
	}
}