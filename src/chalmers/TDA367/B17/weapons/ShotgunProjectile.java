package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;

public class ShotgunProjectile extends AbstractProjectile {

	public ShotgunProjectile(AbstractTank tank, Vector2f position) {
		super(tank, new Vector2f(1,1), 100, 0, 5, 350);
		setSpeed(30);
		setSize(new Vector2f(3,6));
		setPosition(position);
		setShape(new Rectangle(getPosition().getX() - getSize().getX() / 2, getPosition().getY() - getSize().getY() / 2, getSize().getX(), getSize().getY()));
		spriteID = "proj_shotgun";
	}

	public void update(int delta){
		// add spread
		int randomAngle = (int) (Math.random() * 20 - 10);
		direction.add(randomAngle);
		super.update(delta);
		direction.add(-randomAngle);
	}	
}