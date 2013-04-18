package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.*;

public abstract class MovableEntity extends Entity {

	protected Vector2f direction;
	protected float speed;
	private float maxSpeed;
	private float minSpeed;
	private float reverseSpeed;
	private float acceleration;
	private float deacceleration;
	
	public MovableEntity(int id, Vector2f direction, float maxSpeed, float minSpeed, float reverseSpeed) {
		super(id);
		this.direction = direction;
		this.maxSpeed = maxSpeed;
		this.minSpeed = minSpeed;
		this.reverseSpeed = reverseSpeed;
		this.acceleration = maxSpeed*0.1f;
		this.deacceleration = maxSpeed*0.1f;
	}
	
	public boolean isReversing(){
		return speed<0;
	}
	
	public Vector2f getDirection(){
		return direction;
	}
	public void setDirection(Vector2f newDirection){
		this.direction = newDirection;
	}

	public float getSpeed(){
		return speed;
	}
	
	public float getMaxSpeed(){
		return maxSpeed;
	}
	
	public float getMinSpeed(){
		return minSpeed;
	}
	
	public void setVelocity(Vector2f v){
		direction = v;
	}
	
	public void setSpeed(float speed){
		if(speed > maxSpeed){
			this.speed = maxSpeed;
		}else if(speed < minSpeed){
			this.speed = minSpeed;
		}else{
			this.speed = speed;
		}
	}
	
	public void setMaxSpeed(float maxSpeed){
		this.maxSpeed = maxSpeed;
	}
	
	public void setMinSpeed(float minSpeed){
		this.minSpeed = minSpeed;
	}

	public double getReverseSpeed() {
		return reverseSpeed;
	}

	public void setReverseSpeed(float reverseSpeed) {
		this.reverseSpeed = reverseSpeed;
	}

	public double getAcceleration() {
		return acceleration;
	}

	public void setAcceleration(float acceleration) {
		this.acceleration = acceleration;
	}

	public void move(int delta){
		Vector2f tmp = new Vector2f(direction);
		Vector2f velocity = tmp.scale(speed);
		
		velocity.x = velocity.x * delta/60;
		velocity.y = velocity.y * delta/60;
		
		setPosition(position.add(velocity));
	}

	public void accelerate(int delta){
//		setSpeed(speed + acceleration);
		setSpeed(speed + acceleration * delta/60);
	}
	public void deaccelerate(int delta){
		if(speed-deacceleration * delta/60 > 0){
			setSpeed(speed - deacceleration * delta/60);
		}else if(speed+deacceleration * delta/60 < 0){
			setSpeed(speed + deacceleration * delta/60); //TODO Make sure there are no pssible bugs
		}else{
			setSpeed(0);
		}
	}
	public void reverse(int delta){
			setSpeed(speed - deacceleration * delta/60);
	}


	public void update(int delta){
		move(delta);
	}
}
