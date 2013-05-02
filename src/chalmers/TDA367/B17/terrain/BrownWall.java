package chalmers.TDA367.B17.terrain;

import org.newdawn.slick.Sound;
import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.*;

public class BrownWall extends AbstractObstacle {

	Sound debugWallHit;
	
	public BrownWall(Vector2f size, Vector2f position) {
		super(size, position);
		try{
			debugWallHit = new Sound("data/wall.wav");
		}catch(Exception e){
			;
		}
		spriteID = "obstacle";
	}
	
	public void didCollideWith(Entity entity){
		if(entity instanceof AbstractProjectile){
			entity.destroy();
			debugWallHit.play();
		}
	}
}
