package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class ShotgunTurret extends AbstractTurret {

	public ShotgunTurret() {
		super();
		turretCenter = new Vector2f(22.5f, 22.5f);
		turretLength = 42f;
		size = new Vector2f(45f, 65f);
		fireRate = 1000;
		projectileType = "default";
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new ShotgunProjectile();
	}

	@Override
	public void fireWeapon(int delta, AbstractTank a) {
		for(int i = -8; i < 8; i++){
			AbstractProjectile proj = createProjectile();
			Vector2f angle;
			
			if(i%2 == 0){
				angle = new Vector2f(getRotation() + 90 - i*Math.random()*2);
			}else{
				angle = new Vector2f(getRotation() + 90 + i*Math.random()*2);
			}
			
			proj.setDirection(angle);
			proj.speed = (Math.abs(proj.getSpeed()+i*2));
			
			proj.setPosition(getTurretNozzle());
			a.addProj(proj);
		}
	}
	
	/*
	 * Should fireWeapon be moved to AbstractTurret?
	 * Would make some things easier.
	 */

}
