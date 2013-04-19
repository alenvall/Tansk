package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;


public class DefaultTank extends AbstractTank {

	public DefaultTank(int id, Vector2f direction, float maxSpeed, float minSpeed) {
		super(id, direction, maxSpeed, minSpeed);
		setSize(new Vector2f(64f, 96f));
		setPosition(new Vector2f(100, 150));
		setTurretPosition(new Vector2f(getPosition().x+getSize().x/2, getPosition().y+getSize().y/2-5));
	}
}