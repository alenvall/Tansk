package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class ShotgunTurret extends AbstractTurret {

	public ShotgunTurret() {
		super();
		turretCenter = new Vector2f(22.5f, 22.5f);
		turretLength = 42f;
		size = new Vector2f(45f, 65f);
		fireRate = 1000;
		projectileType = "default";
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new ShotgunProjectile();
	}
	
	/*
	 * Should fireWeapon be moved to AbstractTurret?
	 * Would make some things easier.
	 */

}
