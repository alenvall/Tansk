package chalmers.TDA367.B17.powerup;

import chalmers.TDA367.B17.model.*;

public class SpeedPowerUp extends AbstractPowerUp {
	
	public static int MAXSPEED = 10;

	public SpeedPowerUp() {
		super();
		effectDuration = 7000;
	}

	@Override
	public void effect() {
		absTank.setMaxSpeed(MAXSPEED);
	}

	@Override
	public void endEffect() {
		absTank.setMaxSpeed(5);
	}

	@Override
	public void updateEffect() {}

}
