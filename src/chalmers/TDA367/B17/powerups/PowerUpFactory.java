package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractPowerUp;

public class PowerUpFactory {

	private PowerUpFactory() {}

	/**
	 * Get a powerup based on a type (String) and a position.
	 * @param powup The type of powerup
	 * @param position The position of the new powerup
	 * @return A powerup based on a type and a position
	 */
	public static AbstractPowerUp getPowerUp(String powup, Vector2f position){
		if(powup.equals("damage")){
			return new DamagePowerUp(GameController.getInstance().generateID(), position);
		}else if(powup.equals("firerate")){
			return new FireRatePowerUp(GameController.getInstance().generateID(), position);
		}else if(powup.equals("shield")){
			return new ShieldPowerUp(GameController.getInstance().generateID(), position);
		}else if(powup.equals("speed")){
			return new SpeedPowerUp(GameController.getInstance().generateID(), position);
		}else if(powup.equals("health")){
			return new HealthPowerUp(GameController.getInstance().generateID(), position);
		}else{
			return getRandomPowerUp(position);
		}
	}

	/**
	 * Get a random powerup.
	 * @param position The position of the new powerup.
	 * @return A random powerup.
	 */
	public static AbstractPowerUp getRandomPowerUp(Vector2f position){
		int rand = (int)(Math.random()*5);
		if(rand == 1){
			return new DamagePowerUp(GameController.getInstance().generateID(), position);
		}else if(rand == 2){
			return new FireRatePowerUp(GameController.getInstance().generateID(), position);
		}else if(rand == 3){
			return new ShieldPowerUp(GameController.getInstance().generateID(), position);
		}else if(rand == 4){
			return new SpeedPowerUp(GameController.getInstance().generateID(), position);
		}else{
			return new HealthPowerUp(GameController.getInstance().generateID(), position);
		}
	}
}