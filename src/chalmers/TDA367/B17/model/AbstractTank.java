package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.*;

public abstract class AbstractTank extends MovableEntity{

	private String name;
	private double health;
	private AbstractWeapon currentWeapon;
	private AbstractPowerUp currentPowerUp;
	private float turnSpeed; // How many degrees the tank will turn each update
	private AbstractTurret turret;
	private Vector2f turretPosition;
	protected Vector2f turretOffset;
	
	public AbstractTank(int id, Vector2f velocity, float maxSpeed, float minSpeed, float reverseSpeed) {
		super(id, velocity, maxSpeed, minSpeed, reverseSpeed);
		turnSpeed = 3f;
		//currentWeapon = Weapons.DEFAULT_WEAPON;
		turret = new DefaultTurret(1337);
		
		
	}
	
	public double getRotation(){
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

	public Vector2f getTurretPosition() {
		return turretPosition;
	}

	public void setTurretPosition(Vector2f turretPosition) {
		this.turretPosition = turretPosition;
	}

	public Vector2f getTurretOffset() {
	    return turretOffset;
    }

	public void setTurretOffset(Vector2f turretOffset) {
	    this.turretOffset = turretOffset;
    }
}
