package chalmers.TDA367.B17.terrain;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.*;

public class BrownWall extends AbstractObstacle {

	/**
	 * Create a new BrownWall.
	 * @param id The id
	 * @param size The size
	 * @param position The position
	 */
	public BrownWall(int id, Vector2f size, Vector2f position) {
		super(id, size, position);
		spriteID = "obstacle";
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	@Override
	public void didCollideWith(Entity entity){
		if(entity instanceof AbstractProjectile){
			// removed sound
		}
	}
}
