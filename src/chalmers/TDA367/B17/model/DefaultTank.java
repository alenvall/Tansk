package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;


public class DefaultTank extends AbstractTank {

	public DefaultTank(int id, Vector2f direction, float maxSpeed, float minSpeed, float reverseSpeed) {
		super(id, direction, maxSpeed, minSpeed, reverseSpeed);
		setSize(new Vector2f(65f, 96f));
		setPosition(new Vector2f(100, 150));
		setTurretOffset(new Vector2f(32f, 55f));
		setTurretPosition(new Vector2f(getPosition().x+turretOffset.x, getPosition().y+turretOffset.y));
	}
}
