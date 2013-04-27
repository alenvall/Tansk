package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;

public class ShockwaveSecondaryProjectile extends AbstractProjectile {

	public ShockwaveSecondaryProjectile(AbstractTank tank, Vector2f position) {
		super(tank, new Vector2f(1,1), 100, 0, 5, 2000);
		setSpeed(3);
		setSize(new Vector2f(2,2));
		setPosition(position);
		setShape(new Rectangle(getPosition().getX() - getSize().getX() / 2, getPosition().getY() - getSize().getY() / 2, getSize().getX(), getSize().getY()));
		spriteID = "proj_energy";
	}
}
