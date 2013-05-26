package chalmers.TDA367.B17.powerups.pickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractPowerUpPickup;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.powerups.FireRatePowerUp;
/**
 * A PowerUpPickup for the FireRatePowerUp.
 */
public class FireRatePowerUpPickup extends AbstractPowerUpPickup{
	
	/**
	 * Create a new FireRatePowerUpPickup.
	 * @param id The id
	 * @param position The position
	 */
	public FireRatePowerUpPickup(int id, Vector2f position) {
		super(id, position);
		spriteID = "firerate_powerup";
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	@Override
	public void activate(AbstractTank tank){
		super.activate(tank);
		AbstractPowerUp tmp = new FireRatePowerUp(GameController.getInstance().generateID(), tank.getPosition());
		tmp.activate(tank);
	}
}
