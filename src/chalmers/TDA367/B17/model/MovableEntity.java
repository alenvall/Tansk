package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.*;

public abstract class MovableEntity extends Entity {

	private Vector2f velocity;
	private double maxSpeed;
	private double minSpeed;
	private double reverseSpeed;
	private double acceleration;
	
	public MovableEntity(int id, Vector2f velocity, double maxSpeed, double minSpeed, double reverseSpeed) {
		super(id);
		this.velocity = velocity;
		this.maxSpeed = maxSpeed;
		this.minSpeed = minSpeed;
		this.reverseSpeed = reverseSpeed;
	}
	
	public Vector2f getVelocity(){
		return velocity;
	}

	public double getSpeed(){
		return velocity.length();
	}
	
	public double getMaxSpeed(){
		return maxSpeed;
	}
	
	public double getMinSpeed(){
		return minSpeed;
	}
	
	public void setVelocity(Vector2f v){
		velocity = v;
	}
	
	public void setSpeed(double speed){
		this.velocity.scale((float)(speed/this.velocity.length()));
	}
	
	public void setMaxSpeed(double maxSpeed){
		this.maxSpeed = maxSpeed;
	}
	
	public void setMinSpeed(double minSpeed){
		this.minSpeed = minSpeed;
	}

	public double getReverseSpeed() {
		return reverseSpeed;
	}

	public void setReverseSpeed(double reverseSpeed) {
		this.reverseSpeed = reverseSpeed;
	}

	public double getAcceleration() {
		return acceleration;
	}

	public void setAcceleration(double acceleration) {
		this.acceleration = acceleration;
	}

	public void move(){
		// compute velocity
		Vector2f velocity = getVelocity().scale((float) getSpeed());

		// add velocity
		setPosition(getPosition().add(velocity));
	}
}
