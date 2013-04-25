package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

public class FlamethrowerProjectile extends AbstractProjectile {

	public FlamethrowerProjectile(AbstractTank tank, Vector2f position) {
		super(tank, new Vector2f(1,1), 100, 0, 5, 500);
		setSize(new Vector2f(5f, 10f));
		setPosition(position);
		setShape(new Rectangle(getPosition().getX()-getSize().getX()/2, getPosition().getY()-getSize().getY()/2, getSize().getX(), getSize().getY()));
		setSpeed(15);
	}

	
	public void update(int delta){
		// add spread
		int randomAngle = (int) (Math.random() * 80 - 40);
		direction.add(randomAngle);
		super.update(delta);
		direction.add(-randomAngle);
	}	
}