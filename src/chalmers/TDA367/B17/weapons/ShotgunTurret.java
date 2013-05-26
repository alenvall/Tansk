package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.event.GameEvent.EventType;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;

public class ShotgunTurret extends AbstractTurret {

	private static final int DEFAULT_AMMO = 8;
	private int ammoLeft;
	
	/**
	 * Create a new ShotgunTurret.
	 * @param id The id
	 * @param startingRotation The starting rotation
	 * @param tank The tank it belongs to
	 * @param color The color
	 */
	public ShotgunTurret(int id, Vector2f position, double startingRotation, AbstractTank tank, String color) {
		super(id, position, startingRotation,  tank, color);
		ammoLeft = DEFAULT_AMMO;
		setFireRate(1000);
		setProjectileType("shotgun");
		GameController.getInstance().getWorld().addEntity(this);
	}

	@Override
	public AbstractProjectile createProjectile() {	
		return new ShotgunProjectile(GameController.getInstance().generateID(), getTank(), getTurretNozzle());
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank) {
		if(ammoLeft > 0){
			for(int i = -8; i < 8; i++){
				AbstractProjectile projectile = spawnNewProjectile();
				Vector2f angle;
				
				if(i%2 == 0){
					angle = new Vector2f(getRotation() + 90 - i*Math.random()*2);
				}else{
					angle = new Vector2f(getRotation() + 90 + i*Math.random()*2);
				}
				
				projectile.setDirection(angle);
				//The spawned projectiles have different speed.
				projectile.setSpeed(Math.abs(projectile.getSpeed() - (i * 0.025f)));
	
				tank.addProjectile(projectile);
			}
			ammoLeft--;
			GameController.getInstance().getWorld().handleEvent(new GameEvent(EventType.SOUND, this, "SHOTGUN_FIRE_EVENT"));
		}else{
			tank.setTurret(new DefaultTurret(GameController.getInstance().generateID(), getPosition(), getRotation(), getTank(), getColor()));
		}
	}
}
