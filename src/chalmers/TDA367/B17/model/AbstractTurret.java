package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public abstract class AbstractTurret extends Entity {
	private float angle;

	public AbstractTurret(int id) {
		super(id);
	}

	public Vector2f getImagePosition(){
		return new Vector2f(position.x - 23, position.y - 22);
	}

	public float getRotation() {
	    return angle;
    }

	public void setRotation(float angle) {
		this.angle = angle % 360.0f;
    }
}