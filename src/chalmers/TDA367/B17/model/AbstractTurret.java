package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public abstract class AbstractTurret extends Entity {
	private float angle;
	protected Vector2f turretCenter;

	public AbstractTurret(int id) {
		super();
	}

	public Vector2f getImagePosition(){
		return new Vector2f(position.x - turretCenter.x, position.y - turretCenter.y);
	}

	public float getRotation() {
	    return angle;
    }

	public void setRotation(float angle) {
		this.angle = angle % 360.0f;
    }
	
	public Vector2f getTurretCenter(){
		return new Vector2f(turretCenter);
	}
}