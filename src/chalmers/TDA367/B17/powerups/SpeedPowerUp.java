package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.*;

public class SpeedPowerUp extends AbstractPowerUp {
	
	private final int MULTIPLIER = 2;
	private float tmpSpeed;

	public SpeedPowerUp(Vector2f position) {
		super(position);
		effectDuration = 7000;
		spriteID = "speed_powerup";
	}

	@Override
	public void effect() {
		tmpSpeed = absTank.getMaxSpeed();
		absTank.setMaxSpeed(absTank.getMaxSpeed()*MULTIPLIER);
	}

	@Override
	public void endEffect() {
		absTank.setMaxSpeed(tmpSpeed);
	}

	@Override
	public void updateEffect() {}

}
