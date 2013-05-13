package chalmers.TDA367.B17.weaponPickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.weapons.SlowspeedyTurret;

public class SlowspeedyPickup extends AbstractWeaponPickup{

	public SlowspeedyPickup(Vector2f position, AbstractTank tank) {
		super(position);
		spriteID = "SlowspeedyIcon";
	}
	
	@Override
	public void activate(AbstractTank absTank){
		super.activate(absTank);
		absTank.setTurret(new SlowspeedyTurret(absTank));
	}
}