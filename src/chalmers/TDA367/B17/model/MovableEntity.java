package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.*;

public abstract class MovableEntity extends Entity {

	private Vector2f direction;
	private double speed;
	private double maxSpeed;
	private double minSpeed;
	private double reverseSpeed;
	private double acceleration;
	private double deacceleration;
	
	public MovableEntity(int id, Vector2f direction, double maxSpeed, double minSpeed, double reverseSpeed) {
		super(id);
		this.direction = direction;
		this.maxSpeed = maxSpeed;
		this.minSpeed = minSpeed;
		this.reverseSpeed = reverseSpeed;
		this.acceleration = maxSpeed*0.01;
		this.deacceleration = maxSpeed*0.05;
	}
	
	public Vector2f getDirection(){
		return direction;
	}
	public void setDirection(Vector2f newDirection){
		this.direction = newDirection;
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
	
	public void setVelocity(Vector2f v){
		direction = v;
	}
	
	public void setSpeed(double speed){
		if(speed>maxSpeed){
			this.speed = maxSpeed;
		}else if(speed<minSpeed){
			this.speed = minSpeed;
		}else{
			this.speed = speed;
		}
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
		Vector2f tmp = new Vector2f(getDirection());
		Vector2f velocity = tmp.scale((float) getSpeed());

		// add velocity
		setPosition(getPosition().add(velocity));
	}

	public void accelerate(){ // TODO change name
		setSpeed(getSpeed() + getAcceleration());
	}
	public void deaccelerate(){
		setSpeed(getSpeed() - deacceleration);
	}

	public void update(){
		move();
	}
}
