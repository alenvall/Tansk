package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractPowerUp;

public class ShieldPowerUp extends AbstractPowerUp {
	
	private Shield shield;

	public ShieldPowerUp(Vector2f position) {
		super(position);
		effectDuration = 10000;
		spriteID = "shield_powerup";
	}
	
	private void createShield(){
		shield = new Shield(absTank.getPosition());
	}
	
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
		moveShield();
	}

}
