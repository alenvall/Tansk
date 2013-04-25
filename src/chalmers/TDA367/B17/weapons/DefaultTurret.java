package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;

public class DefaultTurret extends AbstractTurret {

	public DefaultTurret() {
		super();
		turretCenter = new Vector2f(22.5f, 22.5f);
		turretLength = 42f;
		size = new Vector2f(45f, 65f);
		fireRate = 200;
		projectileType = "default";
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new DefaultProjectile();
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank){
		tank.addProjectile(spawnNewProjectile());
	}
}
