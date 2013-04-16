package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;


public class DefaultTank extends AbstractTank {

	public DefaultTank(int id, Vector2f direction, float maxSpeed, float minSpeed, float reverseSpeed) {
		super(id, direction, maxSpeed, minSpeed, reverseSpeed);

		setPosition(new Vector2f(100, 150));
	}
}
