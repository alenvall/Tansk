package chalmers.TDA367.B17.weaponPickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.weapons.BounceTurret;

public class BouncePickup extends AbstractWeaponPickup{

	public BouncePickup(int id, Vector2f position) {
		super(id, position);
		spriteID = "BounceGunIcon";
	}
	
	@Override
	public void activate(AbstractTank absTank){
		super.activate(absTank);
		absTank.setTurret(new BounceTurret(GameController.getInstance().generateID(), absTank));
	}
}