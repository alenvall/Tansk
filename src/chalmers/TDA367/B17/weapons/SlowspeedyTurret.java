package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;

public class SlowspeedyTurret extends AbstractTurret {

	public SlowspeedyTurret(AbstractTank tank) {
		super(tank);
		turretCenter = new Vector2f(16.875f, 16.875f);
		turretLength = 31.5f;
		fireRate = 750;
		projectileType = "default";
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank) {
		for(int i = 0; i < 5; i++){
			AbstractProjectile projectile = spawnNewProjectile();
			projectile.setDirection(new Vector2f(getRotation() + 84 + (i * 3)));
			tank.addProjectile(projectile);
		}
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new SlowspeedyProjectile(getTank(), getTurretNozzle());
	}

}
