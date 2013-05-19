package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;

public class ShockwaveTurret extends AbstractTurret{

	public ShockwaveTurret(AbstractTank tank) {
		super(tank);
		turretCenter = new Vector2f(16.875f, 16.875f);
		turretLength = 31.5f;
		fireRate = 3000;
		projectileType = "";
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new ShockwaveProjectile(getTank(), getTurretNozzle());
	}
	
	@Override
	public void fireWeapon(int delta, AbstractTank tank) {
		AbstractProjectile projectile = spawnNewProjectile();
		Vector2f angle = new Vector2f(getRotation() + 90);

		projectile.setDirection(angle);

		tank.addProjectile(projectile);
		GameController.getInstance().getWorld().handleEvent(new GameEvent(this, "SHOCKWAVE_FIRE_EVENT"));
	}

}
