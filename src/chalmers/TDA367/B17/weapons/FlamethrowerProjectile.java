package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.*;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;

public class FlamethrowerProjectile extends AbstractProjectile {

	public FlamethrowerProjectile(int id, AbstractTank tank, Vector2f position) {
		super(id, tank, position, new Vector2f(1,1), 100, 0, 0.07, 300);
		setSpeed(0.25f);
		setSize(new Vector2f(5f, 10f));
		spriteID = "proj_fire";
		setDamage(2);
	}
	
	public void update(int delta){
		// add spread
		int randomAngle = (int) (Math.random() * 80 - 40);
		direction.add(randomAngle);
		super.update(delta);
		direction.add(-randomAngle);
	}	
}