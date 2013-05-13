package chalmers.TDA367.B17.weaponPickups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.weapons.FlamethrowerTurret;

public class FlamethrowerPickup extends AbstractWeaponPickup{

	public FlamethrowerPickup(Vector2f position, AbstractTank tank) {
		super(position);
		this.turret = new FlamethrowerTurret(tank);
	}
}
