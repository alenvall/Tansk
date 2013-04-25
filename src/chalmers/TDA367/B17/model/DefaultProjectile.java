package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

public class DefaultProjectile extends AbstractProjectile {

	public DefaultProjectile(AbstractTank tank, Vector2f position) {
 		super(tank, new Vector2f(1,1), 100, 0, 5, 0);
		setSpeed(20);
		setSize(new Vector2f(5f, 10f));
		setPosition(position);
		setShape(new Rectangle(getPosition().getX() - getSize().getX() / 2, getPosition().getY() - getSize().getY() / 2, getSize().getX(), getSize().getY()));		
	}
}
