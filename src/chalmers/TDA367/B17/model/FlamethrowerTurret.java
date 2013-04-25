package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class FlamethrowerTurret extends AbstractTurret {
	
	
	public FlamethrowerTurret(AbstractTank tank)  {
		super(tank);
		turretCenter = new Vector2f(22.5f, 22.5f);
		turretLength = 42f;
		size = new Vector2f(45f, 65f);
		fireRate = 1;
		projectileType = "fire";
	}
	
	@Override
	public AbstractProjectile createProjectile() {
		return new FlamethrowerProjectile(getTank(), getTurretNozzle());
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank){
		tank.addProjectile(spawnNewProjectile());
	}
}
