package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;


public class DefaultTank extends AbstractTank {

	public DefaultTank(int id, Vector2f velocity, double maxSpeed, double minSpeed, double reverseSpeed) {
		super(id, velocity, maxSpeed, minSpeed, reverseSpeed);

		setSpeed(0);
		setAcceleration(1.0);
		setPosition(new Vector2f(10, 10));
	}
}
