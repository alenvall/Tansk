package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.powerups.powerupPickups.AbstractPowerUpPickup;
import chalmers.TDA367.B17.powerups.powerupPickups.DamagePowerUpPickup;
import chalmers.TDA367.B17.powerups.powerupPickups.FireRatePowerUpPickup;
import chalmers.TDA367.B17.powerups.powerupPickups.HealthPowerUpPickup;
import chalmers.TDA367.B17.powerups.powerupPickups.ShieldPowerUpPickup;
import chalmers.TDA367.B17.powerups.powerupPickups.SpeedPowerUpPickup;

public class PowerUpFactory {

	private PowerUpFactory() {}

	/**
	 * Get a random powerup pickup.
	 * @param position The position of the new powerup pickup.
	 * @return A random powerup pickup.
	 */
	public static AbstractPowerUpPickup getRandomPowerUpPickup(Vector2f position){
		int rand = (int)(Math.random()*5);
		if(rand == 1){
			return new FireRatePowerUpPickup(GameController.getInstance().generateID(), position);
		}else if(rand == 2){
			return new SpeedPowerUpPickup(GameController.getInstance().generateID(), position);
		}else if(rand == 3){
			return new ShieldPowerUpPickup(GameController.getInstance().generateID(), position);
		}else if(rand == 4){
			return new DamagePowerUpPickup(GameController.getInstance().generateID(), position);
		}else{
			return new HealthPowerUpPickup(GameController.getInstance().generateID(), position);
		}
	}
}