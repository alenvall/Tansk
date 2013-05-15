package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Circle;
import org.newdawn.slick.geom.Vector2f;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Entity;

public class Shield extends Entity {

	private double health;

	public Shield(int id, Vector2f position, AbstractTank absTank) {
		super(id);
		setSize(new Vector2f(15, 5));
		setPosition(position);
		setShape(new Circle(position.x, position.y, 100));
		setSize(new Vector2f(100,100));
		spriteID = "shield";
		active = true;

		this.health = 50;
//		this.absTank = absTank;
	}

	public double getHealth() {
		return health;
	}

	public void setHealth(double health) {
		this.health = health;
	}

	public void didCollideWith(Entity entity){
//		TODO: fix so that it no longer requires absTank, mayde do the check in World?
//		if(entity instanceof AbstractProjectile){
//			if(!(((AbstractProjectile)entity).getTank() == absTank)){
//				setHealth(getHealth() - ((AbstractProjectile)entity).getDamage());
//				((AbstractProjectile)entity).destroy();
//				
//				if(getHealth() <= 0){
//					active = false;
//				}
//			}
//		}
	}
}