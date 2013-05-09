package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.*;

public class SpeedPowerUp extends AbstractPowerUp {
	
	private final int MULTIPLIER = 2;
	private float tmpMaxSpeed;
	private float tmpMinSpeed;

	public SpeedPowerUp(Vector2f position) {
		super(position);
		effectDuration = 7000;
		spriteID = "speed_powerup";
	}

	@Override
	public void effect() {
		tmpMaxSpeed = absTank.getMaxSpeed();
		tmpMinSpeed = absTank.getMinSpeed();
		absTank.setMaxSpeed(absTank.getMaxSpeed()*MULTIPLIER);
		absTank.setMinSpeed(absTank.getMinSpeed()*MULTIPLIER);
	}

	@Override
	public void endEffect() {
		absTank.setMaxSpeed(tmpMaxSpeed);
		absTank.setMinSpeed(tmpMinSpeed);
	}

	@Override
	public void updateEffect() {}

}
