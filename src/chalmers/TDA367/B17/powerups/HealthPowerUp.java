package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractTank;

/**
 * A power-up that restores a portion of a tank's health.
 */
public class HealthPowerUp extends AbstractPowerUp {
	
	/**What the tank picking up the power up should be healed for.*/
	public static final int HEALTH_BOOST = 25;

	/**
	 * Create a new HealthPowerUp at a position.
	 * @param id The id
	 * @param position The position of this powerup
	 */
	public HealthPowerUp(int id, Vector2f position) {
		super(id, position);
		setEffectDuration(1);
		setSpriteID("health_powerup");
		GameController.getInstance().getWorld().addEntity(this);
	}

	@Override
	public void effect() {
		if(absTank.getHealth() >= 75){
			absTank.setHealth(100);
		}else{
			absTank.setHealth(absTank.getHealth() + HEALTH_BOOST);
		}
	}

	@Override
	public void endEffect() {}

	@Override
	public void updateEffect() {}
	
	@Override
	public void activate(AbstractTank absTank){
		this.absTank = absTank;
		effect();
		active = false;
		setEffectActive(true);
		spriteID = "";
		//Decrease the powerup count
		GameController.getInstance().getWorld().getSpawner().setPowerupCount
		(GameController.getInstance().getWorld().getSpawner().getPowerupCount() - 1);
		this.destroy();
	}
}
