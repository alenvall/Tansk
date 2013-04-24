package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class ShotgunProjectile extends AbstractProjectile {

	public ShotgunProjectile() {
		super(new Vector2f(1,1), 100, 0, 5, 350);
		speed = 30;
	}
	
	public void update(int delta){
		// add spread
		int randomAngle = (int) (Math.random() * 20 - 10);
		direction.add(randomAngle);
		super.update(delta);
		direction.add(-randomAngle);
	}	
}
