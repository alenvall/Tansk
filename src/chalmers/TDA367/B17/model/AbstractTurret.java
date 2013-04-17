package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public abstract class AbstractTurret extends Entity {

	private Vector2f turretDirection;
	
	public AbstractTurret(int id) {
		super(id);
		turretDirection = new Vector2f();
	}

	public Vector2f getTurretDirection() {
		return turretDirection;
	}

	public void setTurretDirection(Vector2f turretDirection) {
		this.turretDirection = turretDirection;
		this.turretDirection = this.turretDirection.normalise();
	}
}