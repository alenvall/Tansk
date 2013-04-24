package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.*;

public abstract class MovableEntity extends Entity {

	protected Vector2f direction;
	private float speed;
	private float maxSpeed;
	private float minSpeed;
	private float acceleration;
	private float friction;
	
	/**
	 * Create a new MovableEntity based on the following parameters
	 * @param direction The default direction this object is facing
	 * @param maxSpeed The maximum speed of this object
	 * @param minSpeed The minimum speed of this object
	 */
	public MovableEntity(Vector2f direction, float maxSpeed, float minSpeed) {
		super();
		this.direction = direction;
		this.maxSpeed = maxSpeed;
		this.minSpeed = minSpeed;;
		this.acceleration = maxSpeed*0.1f;
		this.friction = maxSpeed*0.1f;
	}
	
	/**
	 * Check if the object is reversing
	 * @return True if the speed is less than 0
	 */
	public boolean isReversing(){
		return speed < 0;
	}
	
	/**
	 * Get the direction of this object
	 * @return The direction of this object
	 */
	public Vector2f getDirection(){
		return direction;
	}
	
	/**
	 * Set the direction of this object
	 * @param newDirection The new direction
	 */
	public void setDirection(Vector2f newDirection){
		this.direction = newDirection;
	}

	/**
	 * Get the speed of this object
	 * @return The speed of this object
	 */
	public float getSpeed(){
		return speed;
	}
	
	/**
	 * Get the maximum speed of this object
	 * @return The maximum speed
	 */
	public float getMaxSpeed(){
		return maxSpeed;
	}
	
	/**
	 * Get the minimum speed of this object
	 * @return The minimum speed
	 */
	public float getMinSpeed(){
		return minSpeed;
	}
	
	/**
	 * Set the speed of this object will be moving at
	 * @param speed The new speed
	 */
	public void setSpeed(float speed){
		if(speed > maxSpeed){
			this.speed = maxSpeed;
		}else if(speed < minSpeed){
			this.speed = minSpeed;
		}else{
			this.speed = speed;
		}
	}
	
	/**
	 * Set the maximum speed of this object
	 * @param maxSpeed The maximum speed this object can move at
	 */
	public void setMaxSpeed(float maxSpeed){
		this.maxSpeed = maxSpeed;
	}
	
	/**
	 * Set the minimum speed of this object
	 * @param minSpeed The minimum speed this object can move at
	 */
	public void setMinSpeed(float minSpeed){
		this.minSpeed = minSpeed;
	}

	/*Are these methods used at all? :O
	public double getReverseSpeed() {
		return reverseSpeed;
	}
	public void setReverseSpeed(float reverseSpeed) {
		this.reverseSpeed = reverseSpeed;
	}*/

	/**
	 * Get the acceleration of this object
	 * @return The acceleration of this object
	 */
	public double getAcceleration() {
		return acceleration;
	}

	/**
	 * Set the acceleration of this object
	 * @param acceleration The acceleration of this object
	 */
	public void setAcceleration(float acceleration) {
		this.acceleration = acceleration;
	}

	/**
	 * Get the friction of this object
	 * @return The friction of this object
	 */
	public float getFriction() {
		return friction;
	}

	/**
	 * Set the friction of this object
	 * @param friction The friction of this object
	 */
	public void setFriction(float friction) {
		this.friction = friction;
	}

	/**
	 * Updates the position of the object, uses speed and direction to calculate the new position.
	 * @param delta hmm...
	 */
	public void move(int delta){
		Vector2f tmp = new Vector2f(direction);
		Vector2f velocity = tmp.scale(speed);
		
		velocity.x = velocity.x * delta/60;
		velocity.y = velocity.y * delta/60;
		
		setPosition(getPosition().add(velocity));
	}

	/**
	 * Increases the speed of this object based on the acceleration.
	 * @param delta hmm...
	 */
	public void accelerate(int delta){
		setSpeed(speed + acceleration * delta/60);
	}
	
	/**
	 * Decreases the speed of this object based on the deceleration.
	 * @param delta wat to write...
	 */
	public void friction(int delta){ //TODO fix the name of this method (friction)
		//if the speed after deceleration is greater than zero the speed is decreased
		if(speed-friction * delta/60 > 0){
			setSpeed(speed - friction * delta/60);
			
		//if the speed after deceleration is less than zero the speed is increased
		}else if(speed+friction * delta/60 < 0){
			setSpeed(speed + friction * delta/60); //TODO Make sure there are no possible bugs
		}else{
			setSpeed(0);
		}
	}
	
	/**
	 * Decreases the speed of this object based on the deceleration.
	 * @param delta 
	 */
	public void reverse(int delta){
			setSpeed(speed - friction * delta/60);
	}

	/**
	 * The update method for the MovableEntity object. 
	 * Used for updating the position of this object.
	 * @param delta
	 */
	public void update(int delta){
		move(delta);
	}
}
