package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;

public class ShockwaveSecondaryProjectile extends AbstractProjectile {

	public ShockwaveSecondaryProjectile(int id, Vector2f position) {
		super(id, position, new Vector2f(1,1), 100, 0, 5, 2000);
		setSpeed(0.05f);
		setSize(new Vector2f(2,2));
		setPosition(position);
		spriteID = "proj_energy";
	}
}
