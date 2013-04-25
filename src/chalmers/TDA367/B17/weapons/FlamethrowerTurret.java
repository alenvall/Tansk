package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;

public class FlamethrowerTurret extends AbstractTurret {

	public FlamethrowerTurret()  {
		super();
		turretCenter = new Vector2f(22.5f, 22.5f);
		turretLength = 42f;
		size = new Vector2f(45f, 65f);
		fireRate = 32;
		projectileType = "fire";
	}
	
	@Override
	public AbstractProjectile createProjectile() {
		return new FlamethrowerProjectile();
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank){
		for(int i = 0; i < 4; i++)
			tank.addProjectile(spawnNewProjectile());
	}
}
