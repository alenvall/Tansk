package chalmers.TDA367.B17.model;

public abstract class AbstractTurret extends Entity {

	private float direction;
	
	public AbstractTurret(int id) {
		super(id);
	}

	public float getDirection() {
		return direction;
	}

	public void setDirection(float direction) {
		this.direction = direction % 360f;
	}
}