package chalmers.TDA367.B17.weaponPickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.weapons.ShotgunTurret;

public class ShotgunPickup extends AbstractWeaponPickup{

	/**
	 * Create a new ShotgunPickup.
	 * @param id The id
	 * @param position The position
	 */
	public ShotgunPickup(int id, Vector2f position) {
		super(id, position);
		spriteID = "ShotgunIcon";
	}
	
	@Override
	public void activate(AbstractTank absTank){
		super.activate(absTank);
		absTank.setTurret(new ShotgunTurret(GameController.getInstance().generateID(), absTank));
	}
}