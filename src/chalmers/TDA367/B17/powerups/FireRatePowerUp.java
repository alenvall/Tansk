package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractTurret;

public class FireRatePowerUp extends AbstractPowerUp {

	private final float MULTIPLIER = 0.5f;
	private AbstractTurret turret;
	private int firerate;
	
	public FireRatePowerUp(Vector2f position) {
		super(position);
		effectDuration = 7000;
		spriteID = "firerate_powerup";
	}

	@Override
	public void effect(){
		//if the turret has been changed
		turret = absTank.getTurret();

		firerate = turret.getFireRate();
		
		//Increase the firerate for the current turret
		turret.setFireRate((int)(firerate*MULTIPLIER));
	}
	
	@Override
	public void endEffect(){
		if(turret == absTank.getTurret())
			turret.setFireRate(firerate);
	}

	@Override
	public void updateEffect() {
		//TODO fix so that the effect remains after changing turret
	}
}
