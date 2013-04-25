package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class SlowspeedyTurret extends AbstractTurret {

	public SlowspeedyTurret() {
		super();
		turretCenter = new Vector2f(22.5f, 22.5f);
		turretLength = 42f;
		size = new Vector2f(45f, 65f);
		fireRate = 750;
		projectileType = "default";
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank) {
		for(int i = 0; i < 5; i++){
			AbstractProjectile projectile = spawnNewProjectile();
			projectile.setDirection(new Vector2f(getRotation() + 80 + (i * 5)));
			tank.addProjectile(projectile);
		}
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new SlowspeedyProjectile();
	}

}
