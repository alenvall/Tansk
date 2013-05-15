package chalmers.TDA367.B17.weaponPickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.weapons.BounceTurret;

public class BouncePickup extends AbstractWeaponPickup{

	public BouncePickup(Vector2f position) {
		super(position);
		spriteID = "BounceGunIcon";
	}
	
	@Override
	public void activate(AbstractTank absTank){
		super.activate(absTank);
		absTank.setTurret(new BounceTurret(absTank));
	}
}