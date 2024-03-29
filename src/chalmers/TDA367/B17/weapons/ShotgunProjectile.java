package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;

public class ShotgunProjectile extends AbstractProjectile {

	/**
	 * Create a new ShotgunProjectile.
	 * @param id The id
	 * @param tank The tank it belongs to
	 * @param position The position
	 */
	public ShotgunProjectile(int id, AbstractTank tank, Vector2f position) {
		super(id, tank, position, new Vector2f(1,1), 50, 0, 3, 350);
		setSpeed(0.3f);
		setSize(new Vector2f(3,6));
		spriteID = "proj_shotgun";
		GameController.getInstance().getWorld().addEntity(this);
	}

	@Override
	public void update(int delta){
		// add spread
		int randomAngle = (int) (Math.random() * 10 - 5);
		direction.add(randomAngle);
		super.update(delta);
		direction.add(-randomAngle);
	}	
}