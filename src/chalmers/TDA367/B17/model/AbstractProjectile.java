package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public abstract class AbstractProjectile extends MovableEntity {
	
	protected double damage;
	protected int duration;
	protected int durationTimer;

	public AbstractProjectile(Vector2f velocity,
			float maxSpeed, float minSpeed, double damage, int duration) {
		super(velocity, maxSpeed, minSpeed);
		this.damage = damage;
		this.duration = duration;
		this.durationTimer = duration;
	}

	public double getDamage() {
		return damage;
	}

	public int getDuration() {
		return duration;
	}

	public void setDamage(double damage) {
		this.damage = damage;
	}

	public void setDuration(int duration) {
		this.duration = duration;
	}
	
	public void update(int delta){
		durationTimer -= delta;
		if(durationTimer < 0){
			active = false;
			durationTimer = this.duration;
		}
		super.update(delta);
	}
}
