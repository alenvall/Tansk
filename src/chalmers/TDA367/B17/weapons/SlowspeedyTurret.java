package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;
import chalmers.TDA367.B17.states.*;

public class SlowspeedyTurret extends AbstractTurret {

	public SlowspeedyTurret(int id) {
		super(id);
		turretCenter = new Vector2f(22.5f, 22.5f);
		turretLength = 42f;
		setSize(new Vector2f(45f, 65f));
		fireRate = 750;
		projectileType = "default";
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank) {
		for(int i = 0; i < 5; i++){
//			AbstractProjectile projectile = spawnNewProjectile();
			AbstractProjectile projectile = createProjectile();
			projectile.setDirection(new Vector2f(getRotation() + 84 + (i * 3)));
//			tank.addProjectile(projectile);
		}
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new SlowspeedyProjectile(ServerState.getInstance().generateID(), getTurretNozzle());
	}

}
