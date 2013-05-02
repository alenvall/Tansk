package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Circle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.Entity;

public class Shield extends Entity {

	public Shield(Vector2f position) {
		setSize(new Vector2f(15, 5));
		setPosition(position);
		setShape(new Circle(position.x, position.y, 100));
		setSize(new Vector2f(100,100));
		spriteID = "shield";
		active = true;
	}
	
	public void didCollideWith(Entity entity){
		if(entity instanceof AbstractProjectile){
			entity.destroy();
		}
	}

}
