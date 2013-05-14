package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;


public class FlamethrowerTurret extends AbstractTurret {
	private static int DEFAULT_AMMO = 500;
	private int ammoLeft;
	public FlamethrowerTurret(AbstractTank tank)  {
		super(tank);
		ammoLeft = DEFAULT_AMMO;
		turretCenter = new Vector2f(16.875f, 16.875f);
		turretLength = 31.5f;
		setSize(new Vector2f(45f, 65f));
		fireRate = 33;
		projectileType = "fire";
	}
	
	@Override
	public AbstractProjectile createProjectile() {
		return new FlamethrowerProjectile(getTank(), getTurretNozzle());
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank){
		for(int i = 0; i < 2; i++){
			tank.addProjectile(spawnNewProjectile());
			if(ammoLeft>0){
				ammoLeft--;
			}else{
				tank.setTurret(new DefaultTurret(getTank()));
			}
		}
	}
}
