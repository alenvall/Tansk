package chalmers.TDA367.B17.weapons.pickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractWeaponPickup;
import chalmers.TDA367.B17.weapons.ShockwaveTurret;

public class ShockwavePickup extends AbstractWeaponPickup{

	/**
	 * Create a new ShockwavePickup.
	 * @param id The id
	 * @param position The position
	 */
	public ShockwavePickup(int id, Vector2f position) {
		super(id, position);
		spriteID = "ShockwaveIcon";
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	@Override
	public void activate(AbstractTank absTank){
		super.activate(absTank);
		absTank.setTurret(new ShockwaveTurret(GameController.getInstance().generateID(), absTank.getTurret().getPosition(), absTank.getTurret().getRotation(), absTank, absTank.getColor()));
	}
}