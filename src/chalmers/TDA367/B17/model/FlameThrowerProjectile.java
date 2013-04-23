package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class FlameThrowerProjectile extends AbstractProjectile {

	public FlameThrowerProjectile() {
		super(new Vector2f(1,1), 100, 0, 5, 400);
		speed = 20;
	}

}
