package chalmers.TDA367.B17.powerups.pickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.powerups.ShieldPowerUp;

/**
 * A PowerUpPickup for the ShieldPowerUp.
 */
public class ShieldPowerUpPickup extends AbstractPowerUpPickup {
	
	/**
	 * Create a new ShieldPowerUpPickup.
	 * @param id The id
	 * @param position The position
	 */
	public ShieldPowerUpPickup(int id, Vector2f position) {
		super(id, position);
		spriteID = "shield_powerup";
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	@Override
	public void activate(AbstractTank tank){
		super.activate(tank);
		AbstractPowerUp tmp = new ShieldPowerUp(GameController.getInstance().generateID(), tank.getPosition());
		tmp.activate(tank);
	}
}
