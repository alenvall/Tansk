package chalmers.TDA367.B17.model;

public abstract class AbstractWeapon {

	private final AbstractProjectile projectileType;
	private int fireRate; // milliseconds
	
	public AbstractWeapon(AbstractProjectile projectileType, int fireRate) {
		this.fireRate = fireRate;
		this.projectileType = projectileType;
	}

	public AbstractProjectile getProjectileType() {
		return projectileType;
	}

	protected void setProjectileType(AbstractProjectile projectileType) {
		//this.projectileType = projectileType;
		//Shouldn't do anything.
	}

	public double getFireRate( ) {
		return fireRate;
	}

	public void setFireRate(int fireRate) {
		this.fireRate = fireRate;
	}

	public abstract AbstractProjectile createProjectile();
}
