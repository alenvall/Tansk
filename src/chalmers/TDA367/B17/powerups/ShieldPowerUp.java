package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractPowerUp;

public class ShieldPowerUp extends AbstractPowerUp {
	
	private Shield shield;

	/**
	 * Create a new ShieldPowerUp at a position.
	 * @param position The position of this powerup
	 */
	public ShieldPowerUp(Vector2f position) {
		super(position);
		effectDuration = 10000;
		spriteID = "shield_powerup";
	}
	
	/**
	 * Create a new shield for the stored tank.
	 */
	private void createShield(){
		shield = new Shield(absTank.getPosition(), absTank);
	}
	
	/**
	 * Move the shield to the tank's position.
	 */
	private void moveShield(){
		shield.setPosition(absTank.getPosition());
	}

	@Override
	public void effect() {
		createShield();
	}

	@Override
	public void endEffect() {
		shield.destroy();
	}

	@Override
	public void updateEffect() {
		if(!shield.isActive() && effectActive){
			deactivate();
		}else if(shield.isActive()){
			moveShield();
		}
	}

	/**
	 * Get the shield of this powerup.
	 * @return The shield
	 */
	public Shield getShield(){
		return shield;
	}
}
