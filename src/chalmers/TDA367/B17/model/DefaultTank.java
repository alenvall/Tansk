package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;


public class DefaultTank extends AbstractTank {

	public DefaultTank(int id, Vector2f direction, double maxSpeed, double minSpeed, double reverseSpeed) {
		super(id, direction, maxSpeed, minSpeed, reverseSpeed);

		setPosition(new Vector2f(10, 10));
	}
}
