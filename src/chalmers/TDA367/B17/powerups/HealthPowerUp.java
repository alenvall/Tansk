package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractPowerUp;

public class HealthPowerUp extends AbstractPowerUp {
	
	//What the tank that picks up the power up should be healed for.
	public static final int HEALTH_BOOST = 25;

	public HealthPowerUp(Vector2f position) {
		super(position);
		setEffectDuration(1);
		setSpriteID("health_powerup");
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
}
