package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Circle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Entity;

public class Shield extends Entity {
	
	//The health of this shield.
	private double health;
	private double MAX_HEALTH = 50;
	
	private AbstractTank absTank;

	/**
	 * Create a new shield for a tank.
	 * @param position The position of this shield
	 * @param absTank The tank that will receive the shield
	 */
	public Shield(Vector2f position, AbstractTank absTank) {
		setSize(new Vector2f(15, 5));
		setPosition(position);
		setShape(new Circle(position.x, position.y, 100));
		setSize(new Vector2f(100,100));
		spriteID = "shield";
		active = true;
		
		this.health = 50;
		this.absTank = absTank;
	}

	/**
	 * Get the health of this shield.
	 * @return The health
	 */
	public double getHealth() {
		return health;
	}
	
	public double getMaxHealth(){
		return MAX_HEALTH;
	}

	/**
	 * Set the health of this shield.
	 * @param health The new health
	 */
	public void setHealth(double health) {
		this.health = health;
	}

	@Override
	public void didCollideWith(Entity entity){
		if(entity instanceof AbstractProjectile){
			if(!(((AbstractProjectile)entity).getTank() == absTank)){
				setHealth(getHealth() - ((AbstractProjectile)entity).getDamage());
				((AbstractProjectile)entity).destroy();
				
				if(getHealth() <= 0){
					active = false;
				}
			}
		}
	}
}
