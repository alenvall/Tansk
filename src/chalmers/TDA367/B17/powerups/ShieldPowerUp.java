package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractTank;

public class ShieldPowerUp extends AbstractPowerUp {


	
	/**
	 * Create a new ShieldPowerUp at a position.
	 * @param position The position of this powerup
	 */
	public ShieldPowerUp(int id, Vector2f position) {
		super(id, position);
		effectDuration = 1;
		spriteID = "shield_powerup";
	}

	/**
	 * Create a new shield for the stored tank.
	 */
	private void createShield(){
		if(absTank.getShield() != null){
			absTank.getShield().destroy();
			absTank.setShield(null);
		}
		absTank.setShield(new Shield(GameController.getInstance().generateID(), absTank, 99999));
	}

	@Override
	public void effect() {
		createShield();
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
		effectActive = true;
		spriteID = "";
		//Decrease the powerup count
		GameController.getInstance().getWorld().getSpawner().setPowerupCount
		(GameController.getInstance().getWorld().getSpawner().getPowerupCount() - 1);
	}
}