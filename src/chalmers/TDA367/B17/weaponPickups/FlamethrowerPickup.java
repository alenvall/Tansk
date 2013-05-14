package chalmers.TDA367.B17.weaponPickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.weapons.FlamethrowerTurret;

public class FlamethrowerPickup extends AbstractWeaponPickup{

	public FlamethrowerPickup(Vector2f position) {
		super(position);
		spriteID = "FlamethrowerIcon";
	}
	
	@Override
	public void activate(AbstractTank absTank){
		super.activate(absTank);
		absTank.setTurret(new FlamethrowerTurret(absTank));
	}
}
