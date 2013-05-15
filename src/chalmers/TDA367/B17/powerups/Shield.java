package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Circle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Entity;

public class Shield extends Entity {
	
	private AbstractTank absTank;
	
	private double health;
	
	private int duration;
	private boolean countDuration = true;

	/**
	 * Create a new shield for a tank.
	 * @param absTank The tank that will receive the shield
	 * @param duration The duration of the shield (0 for unlimited time)
	 */
	public Shield(AbstractTank absTank, int duration) {
		setSize(new Vector2f(15, 5));
		Vector2f position = absTank.getPosition();
		setPosition(position);
		setShape(new Circle(position.x, position.y, 100));
		setSize(new Vector2f(100,100));
		spriteID = "shield";
		active = true;

		this.absTank = absTank;
		setHealth(absTank.getMaxShieldHealth());
		
		if(duration <= 0){
			countDuration = false;
		}
		this.duration = duration;
	}
	
	@Override
	public void update(int delta){
		if(countDuration){
			duration -= delta;
		}
		
		setPosition(absTank.getPosition());
		
		if(getHealth() <= 0 || (duration <= 0 && countDuration)){
			absTank.setShield(null);
			destroy();
		}
	}

	@Override
	public void didCollideWith(Entity entity){
		if(entity instanceof AbstractProjectile){
			if(!(((AbstractProjectile)entity).getTank() == absTank)){
				setHealth(getHealth() - ((AbstractProjectile)entity).getDamage());
				((AbstractProjectile)entity).destroy();
			}
		}
	}

	public double getHealth() {
		return health;
	}

	public void setHealth(double health) {
		this.health = health;
	}
}
