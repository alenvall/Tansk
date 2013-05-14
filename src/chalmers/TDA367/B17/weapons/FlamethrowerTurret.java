package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;
import chalmers.TDA367.B17.states.*;

public class FlamethrowerTurret extends AbstractTurret {
		
	public FlamethrowerTurret(int id)  {
		super(id);
		turretCenter = new Vector2f(22.5f, 22.5f);
		turretLength = 42f;
		setSize(new Vector2f(45f, 65f));
		fireRate = 33;
		projectileType = "fire";
	}
	
	@Override
	public AbstractProjectile createProjectile() {	
		FlamethrowerProjectile proj = new FlamethrowerProjectile(ServerState.getInstance().generateID());
		proj.setPosition(getTurretNozzle());
		return proj;
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank){
		for(int i = 0; i < 2; i++)
//			tank.addProjectile(spawnNewProjectile());
			spawnNewProjectile();
	}
}
