package chalmers.TDA367.B17.model;

import java.awt.Point;
import org.newdawn.slick.geom.*;

public abstract class AbstractTank extends MovableEntity {

	private String name;
	private double health;
	private AbstractWeapon currentWeapon;
	private AbstractPowerUp currentPowerUp;
	private float turnSpeed; // How many degrees the tank will turn each update
	protected AbstractTurret turret;
	protected float turretOffset;
	Point mouseCoords;
	
	public AbstractTank(int id, Vector2f velocity, float maxSpeed, float minSpeed, float reverseSpeed) {
		super(id, velocity, maxSpeed, minSpeed, reverseSpeed);
		turnSpeed = 3f;
	}
	
	public Vector2f getImagePosition(){
		return new Vector2f(position.x - size.x/2, position.y - size.y/2);
	}
	
	public double getRotation(){;
		return direction.getTheta();
	}
	
	public String getName() {
		return name;
	}

	public void setName(String name) {
		this.name = name;
	}

	public double getHealth() {
		return health;
	}

	public void setHealth(double health) {
		this.health = health;
	}

	public AbstractWeapon getCurrentWeapon() {
		return currentWeapon;
	}

	public void setCurrentWeapon(AbstractWeapon currentWeapon) {
		this.currentWeapon = currentWeapon;
	}

	public AbstractPowerUp getCurrentPowerUp() {
		return currentPowerUp;
	}

	public void setCurrentPowerUp(AbstractPowerUp currentPowerUp) {
		this.currentPowerUp = currentPowerUp;
	}

	public float getTurnSpeed() {
		return turnSpeed;
	}

	public void setTurnSpeed(float turnSpeed) {
		this.turnSpeed = turnSpeed;
	}
	
	public AbstractTurret getTurret() {
		return turret;
	}

	public void setTurret(AbstractTurret turret) {
		this.turret = turret;
	}

	public void turnLeft(int delta){
		setDirection(getDirection().add(-turnSpeed * delta/60 * (Math.abs(speed)*0.2f + 0.7)));
	}
	
	public void turnRight(int delta){
		setDirection(getDirection().add(turnSpeed * delta/60 * (Math.abs(speed)*0.2f + 0.7)));
	}

	public float getTurretOffset() {
	    return turretOffset;
    }
	
	public void update(int delta, Point mouseCoords){
		super.update(delta);
		updateTurret(mouseCoords);
	}

	private void updateTurret(Point mouseCoords) {
		float rotation = (float) Math.toDegrees(Math.atan2(turret.getPosition().x - mouseCoords.x + 0, turret.getPosition().y - mouseCoords.y + 0)* -1)+180;
        turret.setRotation(rotation);  
        
        double tankRotation = getRotation(); 
        float newTurX = (float) (position.x + turretOffset * Math.cos(Math.toRadians(tankRotation + 180)));
        float newTurY = (float) ((float) position.y - turretOffset * Math.sin(Math.toRadians(tankRotation)));
      	
		turret.setPosition(new Vector2f(newTurX, newTurY));
    }
}
