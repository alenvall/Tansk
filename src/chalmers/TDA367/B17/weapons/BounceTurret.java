package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;
import chalmers.TDA367.B17.states.*;

public class BounceTurret extends AbstractTurret {

	
	public BounceTurret(int id, AbstractTank tank) {
		super(id);
		turretCenter = new Vector2f(22.5f, 22.5f);
		turretLength = 42f;
		fireRate = 200;
		projectileType = "default";
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new BounceProjectile(ServerState.getInstance().generateID(), getTurretNozzle());
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank){
//		tank.addProjectile(spawnNewProjectile());
		spawnNewProjectile();
	}
}
