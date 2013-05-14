package chalmers.TDA367.B17.terrain;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.*;

public class BrownWall extends AbstractObstacle {

	
	public BrownWall(int id, Vector2f size, Vector2f position) {
		super(id, size, position);
		spriteID = "obstacle";
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	public void didCollideWith(Entity entity){
		if(entity instanceof AbstractProjectile){
			// removed sound
		}
	}
}
