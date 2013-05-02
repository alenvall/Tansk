package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

public class AbstractObstacle extends Entity {

	public AbstractObstacle(Vector2f size, Vector2f position) {
		setSize(size);
		setShape(new Rectangle(position.getX()-size.getX()/2, position.getY()-size.getY()/2, size.getX(), size.getY()));
	}

}
