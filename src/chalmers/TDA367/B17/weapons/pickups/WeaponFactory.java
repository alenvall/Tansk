package chalmers.TDA367.B17.weapons.pickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;

/**
 * A class used to generate a random weapon pickup.
 *
 */
public class WeaponFactory {

	private WeaponFactory() {}
	
	/**
	 * Get a new random weapon pickup.
	 * @param position The position of the new weapon pickup
	 * @return The new random weapon pickup
	 */
	public static AbstractWeaponPickup getRandomWeaponPickup(Vector2f position){
		int rand = (int)(Math.random()*5);
		if(rand == 1){
			return new FlamethrowerPickup(GameController.getInstance().generateID(), position);
		}else if(rand == 2){
			return new ShotgunPickup(GameController.getInstance().generateID(), position);
		}else if(rand == 3){
			return new ShockwavePickup(GameController.getInstance().generateID(), position);
		}else if(rand == 4){
			return new SlowspeedyPickup(GameController.getInstance().generateID(), position);
		}else{
			return new BouncePickup(GameController.getInstance().generateID(), position);
		}
	}

}
