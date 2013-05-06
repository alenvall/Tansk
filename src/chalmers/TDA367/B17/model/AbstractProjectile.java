package chalmers.TDA367.B17.model;

import org.newdawn.slick.SlickException;
import org.newdawn.slick.Sound;
import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.terrain.BrownWall;

public abstract class AbstractProjectile extends MovableEntity {
	
	private double damage;
	private int duration;
	private int durationTimer;
	private AbstractTank tank;
	private Sound debugWallHit;


	/**
	 * Create a new AbstractProjectile.
	 * @param velocity the initial velocity
	 * @param maxSpeed the maximum speed
	 * @param minSpeed the minimum speed
	 * @param damage the damage this projectile does
	 * @param duration the time in milliseconds this projectile will remain on the map
	 */
	public AbstractProjectile(AbstractTank tank, Vector2f position, Vector2f velocity,
			float maxSpeed, float minSpeed, double damage, int duration) {
		super(velocity, maxSpeed, minSpeed);
		this.damage = damage;
		this.duration = duration;
		this.durationTimer = duration;
		spriteID = "bullet";
		this.tank = tank;
		try {
	        debugWallHit = new Sound("data/bullet.wav");
        } catch (SlickException e) {
	        e.printStackTrace();
        }
		this.tank = tank;
		setShape(new Rectangle(position.x, position.y, 1,1));
		renderLayer = GameController.RenderLayer.TOP;
	}
	
	public AbstractTank getTank(){
		return tank;
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

	public void setTank(AbstractTank tank) {
		this.tank = tank;
	}
	
	public void didCollideWith(Entity entity){
		if(entity instanceof AbstractProjectile || entity == getTank()){
			return;
		}else if(entity instanceof MapBounds || entity instanceof BrownWall){
			debugWallHit.play();
			this.destroy();
		}else if(entity instanceof AbstractTank){
			damageTarget((AbstractTank)entity);
		}
	}


	@Override
	public void destroy(){
		super.destroy();
		getTank().getProjectiles().remove(this);
	}
	
	public void damageTarget(AbstractTank target){
		target.recieveDamage(this);
		debugWallHit.play();
		this.destroy();
	}
}
