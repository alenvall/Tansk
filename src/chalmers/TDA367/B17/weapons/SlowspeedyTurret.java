package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;

public class SlowspeedyTurret extends AbstractTurret {

	/**
	 * Create a new SlowspeedyTurret.
	 * @param id The id
	 * @param tank The tank it belongs to
	 */
	public SlowspeedyTurret(int id, AbstractTank tank) {
		super(id, tank);
		turretCenter = new Vector2f(16.875f, 16.875f);
		turretLength = 31.5f;
		fireRate = 750;
		projectileType = "default";
		GameController.getInstance().getWorld().addEntity(this);
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
		return new SlowspeedyProjectile(GameController.getInstance().generateID(), getTank(), getTurretNozzle());
	}

}
