package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.event.GameEvent.EventType;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;

public class SlowspeedyTurret extends AbstractTurret {

	/**
	 * Create a new SlowspeedyTurret.
	 * @param id The id
	 * @param tank The tank it belongs to
	 */
	public SlowspeedyTurret(int id, Vector2f position, double startingRotation, AbstractTank tank) {
		super(id, position, startingRotation,  tank);
		turretCenter = new Vector2f(16.875f, 16.875f);
		turretLength = 31.5f;
		fireRate = 750;
		projectileType = "default";
		GameController.getInstance().getWorld().addEntity(this);
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank) {
		for(int i = 0; i < 5; i++){
			AbstractProjectile projectile = spawnNewProjectile();
			projectile.setDirection(new Vector2f(getRotation() + 86 + (i * 2)));
			tank.addProjectile(projectile);
		}
		GameController.getInstance().getWorld().handleEvent(new GameEvent(EventType.SOUND, this, "SLOWSPEEDY_FIRE_EVENT"));
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new SlowspeedyProjectile(GameController.getInstance().generateID(), getTank(), getTurretNozzle());
	}

}
