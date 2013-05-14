package chalmers.TDA367.B17.weaponPickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.weapons.ShockwaveTurret;

public class ShockwavePickup extends AbstractWeaponPickup{

	public ShockwavePickup(Vector2f position) {
		super(position);
		spriteID = "ShockwaveIcon";
	}
	
	@Override
	public void activate(AbstractTank absTank){
		super.activate(absTank);
		absTank.setTurret(new ShockwaveTurret(absTank));
	}
}