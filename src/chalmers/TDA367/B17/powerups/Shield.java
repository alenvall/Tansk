package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.Entity;

public class Shield extends Entity {

	public Shield(Vector2f pos) {
		setSize(new Vector2f(15,5));
		setPosition(position);
		setShape(new Rectangle(getPosition().getX() - getSize().getX() / 2, getPosition().getY() - getSize().getY() / 2, getSize().getX(), getSize().getY()));
		spriteID = "proj_energy";
		active = true;
	}
	
	public void didCollideWith(Entity entity){
		if(entity instanceof AbstractProjectile){
			active = false;
			this.destroy();
			entity.destroy();
		}
	}

}
