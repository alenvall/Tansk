package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;

public class ShockwaveTurret extends AbstractTurret{

	/**
	 * Create a new ShockwaveTurret.
	 * @param id The id
	 * @param tank The tank it belongs to
	 */
	public ShockwaveTurret(int id, Vector2f position, double startingRotation, AbstractTank tank) {
	super(id, position, startingRotation,  tank);
		turretCenter = new Vector2f(16.875f, 16.875f);
		turretLength = 31.5f;
		fireRate = 3000;
		projectileType = "";
		GameController.getInstance().getWorld().addEntity(this);
	}

	@Override
	public AbstractProjectile createProjectile() {	
		return new ShockwaveProjectile(GameController.getInstance().generateID(), getTank(), getTurretNozzle());
	}
	
	@Override
	public void fireWeapon(int delta, AbstractTank tank) {
		AbstractProjectile projectile = spawnNewProjectile();
		Vector2f angle = new Vector2f(getRotation() + 90);

		projectile.setDirection(angle);

		tank.addProjectile(projectile);
	}

}
