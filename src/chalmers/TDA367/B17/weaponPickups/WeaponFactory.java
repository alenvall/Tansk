package chalmers.TDA367.B17.weaponPickups;

import org.newdawn.slick.geom.Vector2f;

public class WeaponFactory {

	private WeaponFactory() {}
	
	public static AbstractWeaponPickup getRandomWeaponPickup(Vector2f position){
		int rand = (int)(Math.random()*5);
		if(rand == 1){
			return new FlamethrowerPickup(position);
		}else if(rand == 2){
			return new ShotgunPickup(position);
		}else if(rand == 3){
			return new ShockwavePickup(position);
		}else if(rand == 4){
			return new SlowspeedyPickup(position);
		}else{
			return new BouncePickup(position);
		}
	}

}
