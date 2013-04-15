package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.*;

public abstract class MovableEntity extends Entity {

	private Vector2f direction;
	private double speed;
	private double maxSpeed;
	private double minSpeed;
	
	public MovableEntity(int id, Vector2f direction, double speed, double maxSpeed, double minSpeed) {
		super(id);
		this.direction = direction;
		this.speed = speed;
		this.maxSpeed = maxSpeed;
		this.minSpeed = minSpeed;
	}
	
	public Vector2f getDirection(){
		return direction;
	}

	public double getSpeed(){
		return speed;
	}
	
	public double getMaxSpeed(){
		return maxSpeed;
	}
	
	public double getMinSpeed(){
		return minSpeed;
	}
	
	public void setDirection(Vector2f v){
		direction = v;
	}
	
	public void setSpeed(double speed){
		this.speed = speed;
	}
	
	public void setMaxSpeed(double maxSpeed){
		this.maxSpeed = maxSpeed;
	}
	
	public void setMinSpeed(double minSpeed){
		this.minSpeed = minSpeed;
	}
}
