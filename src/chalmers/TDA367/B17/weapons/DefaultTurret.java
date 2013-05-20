package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.event.GameEvent.EventType;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;

public class DefaultTurret extends AbstractTurret {
	
	/**
	 * Create a new DefaultTurret.
	 * @param id The id
	 * @param tank The tank it belongs to
	 */
	public DefaultTurret(int id, AbstractTank tank) {
		super(id, tank);
		turretCenter = new Vector2f(17f, 17f);
		turretLength = 31.5f;
		fireRate = 500;
		projectileType = "default";
		GameController.getInstance().getWorld().addEntity(this);
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new DefaultProjectile(GameController.getInstance().generateID(), getTank(), getTurretNozzle());
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank){
		tank.addProjectile(spawnNewProjectile());
		GameController.getInstance().getWorld().handleEvent(new GameEvent(EventType.SOUND, this, "DEFAULTTURRET_FIRE_EVENT"));
	}
}
