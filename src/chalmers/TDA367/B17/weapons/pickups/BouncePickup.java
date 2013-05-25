package chalmers.TDA367.B17.weapons.pickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.weapons.BounceTurret;

public class BouncePickup extends AbstractWeaponPickup{

	/**
	 * Create a new BouncePickup.
	 * @param id The id
	 * @param position The position
	 */
	public BouncePickup(int id, Vector2f position) {
		super(id, position);
		spriteID = "BounceGunIcon";
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	@Override
	public void activate(AbstractTank absTank){
		super.activate(absTank);
		absTank.setTurret(new BounceTurret(GameController.getInstance().generateID(), absTank.getTurret().getPosition(), absTank.getTurret().getRotation(), absTank, absTank.getColor()));
	}
}