package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;

public class ShotgunTurret extends AbstractTurret {

	public ShotgunTurret(AbstractTank tank) {
		super(tank);
		turretCenter = new Vector2f(22.5f, 22.5f);
		turretLength = 42f;
		size = new Vector2f(45f, 65f);
		fireRate = 1000;
		projectileType = "shotgun";
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new ShotgunProjectile(getTank(), getTurretNozzle());
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank) {
		for(int i = -8; i < 8; i++){
			AbstractProjectile projectile = spawnNewProjectile();
			Vector2f angle;
			
			if(i%2 == 0){
				angle = new Vector2f(getRotation() + 90 - i*Math.random()*2);
			}else{
				angle = new Vector2f(getRotation() + 90 + i*Math.random()*2);
			}
			
			projectile.setDirection(angle);
			projectile.setSpeed(Math.abs(projectile.getSpeed() * (i * 0.2f)));

			tank.addProjectile(projectile);
		}
	}
	
	/*
	 * Should fireWeapon be moved to AbstractTurret?
	 * Would make some things easier.
	 */

}
