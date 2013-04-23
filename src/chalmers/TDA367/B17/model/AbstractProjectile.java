package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public abstract class AbstractProjectile extends MovableEntity {
	
	protected double damage;
	protected int duration;
	protected int durationTimer;

	/**
	 * Create a new AbstractProjectile.
	 * @param velocity the initial velocity
	 * @param maxSpeed the maximum speed
	 * @param minSpeed the minimum speed
	 * @param damage the damage this projectile does
	 * @param duration the time in milliseconds this projectile will remain on the map
	 */
	public AbstractProjectile(Vector2f velocity,
			float maxSpeed, float minSpeed, double damage, int duration) {
		super(velocity, maxSpeed, minSpeed);
		this.damage = damage;
		this.duration = duration;
		this.durationTimer = duration;
	}
	/**
	 * Get the damage of this projectile.
	 * @return How much damage the projectile does
	 */
	public double getDamage() {
		return damage;
	}
	/**
	 * Get the duration of the projectile.
	 * @return The time in milliseconds that 
	 * the projectile will remain on the map
	 */
	public int getDuration() {
		return duration;
	}
	/**
	 * Set the damage of the projectile.
	 * @param damage How much damage the projectile does
	 */
	public void setDamage(double damage) {
		this.damage = damage;
	}
	/**
	 * Set the name of the powerUp.
	 * @param name The powerUps name
	 */
	public void setDuration(int duration) {
		this.duration = duration;
	}
	/**
	 * Update the projectile's state.
	 * @param delta The time that has passed since the last update
	 */
	public void update(int delta){
		durationTimer -= delta;
		if(durationTimer < 0){
			active = false;
			durationTimer = this.duration;
		}
		super.update(delta);
	}
}
