package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;


public class DefaultTank extends AbstractTank {

	public DefaultTank(int id, Vector2f direction, float maxSpeed, float minSpeed) {
		super(id, direction, maxSpeed, minSpeed);
		setSize(new Vector2f(65f, 85f));
		setPosition(new Vector2f(100, 150));
		turret = new DefaultTurret(1337);
		turretOffset = 6;
		turret.setPosition(new Vector2f(position.x, turretOffset));
	}
}
