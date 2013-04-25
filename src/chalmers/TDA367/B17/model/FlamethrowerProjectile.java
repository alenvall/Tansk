package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class FlamethrowerProjectile extends AbstractProjectile {

	public FlamethrowerProjectile() {
		super(new Vector2f(1,1), 100, 0, 5, 500);
		setSpeed(15);
		spriteID = "proj_fire";
	}

	
	public void update(int delta){
		// add spread
		int randomAngle = (int) (Math.random() * 80 - 40);
		direction.add(randomAngle);
		super.update(delta);
		direction.add(-randomAngle);
	}	
}