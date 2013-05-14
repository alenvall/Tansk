package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;

public class ShotgunProjectile extends AbstractProjectile {

	public ShotgunProjectile(int id) {
		super(id, new Vector2f(1,1), 100, 0, 5, 350);
		setSpeed(0.35f);
		setSize(new Vector2f(3,6));
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
