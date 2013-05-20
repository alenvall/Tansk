package chalmers.TDA367.B17.weaponPickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.weapons.ShotgunTurret;

public class ShotgunPickup extends AbstractWeaponPickup{

	public ShotgunPickup(int id, Vector2f position) {
		super(id, position);
		spriteID = "ShotgunIcon";
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	@Override
	public void activate(AbstractTank absTank){
		super.activate(absTank);
		absTank.setTurret(new ShotgunTurret(GameController.getInstance().generateID(), absTank.getTurret().getPosition(), absTank.getTurret().getRotation(), absTank));
	}
}