package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;
import chalmers.TDA367.B17.terrain.BrownWall;

public abstract class AbstractProjectile extends MovableEntity {
	
	private double damage;
	private int duration;
	private int durationTimer;

	/**
	 * Create a new AbstractProjectile.
	 * @param velocity the initial velocity
	 * @param maxSpeed the maximum speed
	 * @param minSpeed the minimum speed
	 * @param damage the damage this projectile does
	 * @param duration the time in milliseconds this projectile will remain on the map
	 */
	public AbstractProjectile(int id, Vector2f velocity,
			float maxSpeed, float minSpeed, double damage, int duration) {
		super(id, maxSpeed, minSpeed);
		this.damage = damage;
		this.duration = duration;
		this.durationTimer = duration;
		spriteID = "bullet";
		renderLayer = RenderLayer.FOURTH;
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
	 * Set the duration of the power up.
	 * @param duration The time in milliseconds that
	 * the projectile will remain on the map
	 */
	public void setDuration(int duration) {
		this.duration = duration;
	}
	
	/**
	 * Update the projectile's state.
	 * @param delta The time that has passed since the last update
	 */
	public void update(int delta){
		if(duration != 0){
			durationTimer -= delta;
			if(durationTimer < 0)
				destroy();
		}
		super.update(delta);
	}
	
	public double getRotation(){
		return direction.getTheta() + 90;
	}
	
	public int getDurationTimer() {
		return durationTimer;
	}

	public void setDurationTimer(int durationTimer) {
		this.durationTimer = durationTimer;
	}
	
	public void didCollideWith(Entity entity){
//		TODO: check this in World
//		if(entity instanceof AbstractProjectile || entity == getTank()){
//			return;
//		}else if(entity instanceof MapBounds || entity instanceof BrownWall){
		if(entity instanceof MapBounds || entity instanceof BrownWall){
			this.destroy();
		}else if(entity instanceof AbstractTank){
			damageTarget((AbstractTank)entity);
		}
	}


	@Override
	public void destroy(){
		super.destroy();
//		getTank().getProjectiles().remove(this);
	}
	
	public void damageTarget(AbstractTank target){
		target.recieveDamage(this);
		this.destroy();
	}
}
