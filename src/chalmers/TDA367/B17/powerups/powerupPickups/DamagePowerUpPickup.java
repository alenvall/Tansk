package chalmers.TDA367.B17.powerups.powerupPickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.powerups.DamagePowerUp;

public class DamagePowerUpPickup extends AbstractPowerUpPickup {

	/**
	 * Create a new DamagePowerUpPickup.
	 * @param id The id
	 * @param position The position
	 */
	public DamagePowerUpPickup(int id, Vector2f position) {
		super(id, position);
		spriteID = "damage_powerup";
		GameController.getInstance().getWorld().addEntity(this);
	}

	@Override
	public void activate(AbstractTank tank){
		super.activate(tank);
		AbstractPowerUp tmp = new DamagePowerUp(GameController.getInstance().generateID(), tank.getPosition());
		tmp.activate(tank);
	}
}
