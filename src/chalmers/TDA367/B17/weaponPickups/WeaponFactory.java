package chalmers.TDA367.B17.weaponPickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;

public class WeaponFactory {

	private WeaponFactory() {}
	
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
