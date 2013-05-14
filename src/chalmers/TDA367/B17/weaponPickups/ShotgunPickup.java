package chalmers.TDA367.B17.weaponPickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.weapons.ShotgunTurret;

public class ShotgunPickup extends AbstractWeaponPickup{

	public ShotgunPickup(Vector2f position) {
		super(position);
		spriteID = "ShotgunIcon";
	}
	
	@Override
	public void activate(AbstractTank absTank){
		super.activate(absTank);
		absTank.setTurret(new ShotgunTurret(absTank));
	}
}