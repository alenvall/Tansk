package chalmers.TDA367.B17.weaponPickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.weapons.SlowspeedyTurret;

public class SlowspeedyPickup extends AbstractWeaponPickup{

	public SlowspeedyPickup(int id, Vector2f position) {
		super(id, position);
		spriteID = "SlowspeedyIcon";
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	@Override
	public void activate(AbstractTank absTank){
		super.activate(absTank);
		absTank.setTurret(new SlowspeedyTurret(GameController.getInstance().generateID(), absTank.getTurret().getPosition(), absTank.getTurret().getRotation(), absTank));
	}
}