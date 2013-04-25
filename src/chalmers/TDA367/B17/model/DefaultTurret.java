package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class DefaultTurret extends AbstractTurret {

	
	public DefaultTurret(AbstractTank tank) {
		super(tank);
		turretCenter = new Vector2f(22.5f, 22.5f);
		turretLength = 42f;
		size = new Vector2f(45f, 65f);
		fireRate = 200;
		projectileType = "default";
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new DefaultProjectile(getTank(), getTurretNozzle());
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank){
		tank.addProjectile(spawnNewProjectile());
	}
}
