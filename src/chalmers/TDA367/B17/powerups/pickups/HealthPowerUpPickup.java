package chalmers.TDA367.B17.powerups.pickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.powerups.HealthPowerUp;

public class HealthPowerUpPickup extends AbstractPowerUpPickup {
	
	/**
	 * Create a new HealthPowerUpPickup.
	 * @param id The id
	 * @param position The position
	 */
	public HealthPowerUpPickup(int id, Vector2f position) {
		super(id, position);
		spriteID = "health_powerup";
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	@Override
	public void activate(AbstractTank tank){
		super.activate(tank);
		AbstractPowerUp tmp = new HealthPowerUp(GameController.getInstance().generateID(), tank.getPosition());
		tmp.activate(tank);
	}
}
