package chalmers.TDA367.B17.powerup;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.*;

public class SpeedPowerUp extends AbstractPowerUp {
	
	public static int MAXSPEED = 10;

	public SpeedPowerUp(Vector2f position) {
		super(position);
		effectDuration = 7000;
		spriteID = "speed_powerup";
		type = "speed";
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
