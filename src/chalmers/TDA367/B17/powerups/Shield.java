package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Circle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Entity;

public class Shield extends Entity {

	private static int shieldRadius = 53;
	private AbstractTank absTank;
	
	private double health;

	private int duration;
	private boolean countDuration = true;

	/**
	 * Create a new shield for a tank.
	 * @param id The id
	 * @param absTank The tank that will receive the shield
	 * @param duration The duration of the shield (0 for unlimited time)
	 */
	public Shield(int id, AbstractTank absTank, int duration) {
		super(id);
		if(absTank != null){
			Vector2f position = absTank.getPosition();
			setShape(new Circle(position.x, position.y, shieldRadius));
			this.absTank = absTank;
			setHealth(absTank.getMaxShieldHealth());
		}
		spriteID = "shield";
		active = true;


		if(duration <= 0){
			countDuration = false;
		}
		this.duration = duration;
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	public AbstractTank getTank(){
		return absTank;
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
				damageShield(((AbstractProjectile)entity).getDamage());
				
				((AbstractProjectile)entity).destroy();
			}
		}
	}

	/**
	 * Get the health of this shield.
	 * @return
	 */
	public double getHealth() {
		return health;
	}

	/**
	 * Set the health of this shield.
	 * @param health The new health
	 */
	public void setHealth(double health) {
		this.health = health;
	}
	
	/**
	 * Deal damage to the shield. Any leftover damage will be dealt to the tank.
	 * @param damage The damage
	 */
	public void damageShield(double damage){
		double overkill = (getHealth() - damage);
		if(overkill <= 0){
			absTank.setHealth(absTank.getHealth() + overkill);
		}
		setHealth(getHealth() - damage);
	}
}
