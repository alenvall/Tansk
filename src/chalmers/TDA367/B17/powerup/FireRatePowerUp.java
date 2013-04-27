package chalmers.TDA367.B17.powerup;

import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractTurret;

public class FireRatePowerUp extends AbstractPowerUp {

	public static float MULTIPLIER = 0.5f;
	private AbstractTurret turret;
	private int firerate;
	
	public FireRatePowerUp() {
		super();
		effectDuration = 7000;
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
		if(absTank.getTurret() == turret){
			turret.setFireRate(firerate);
		}
	}

	@Override
	public void updateEffect() {}
}
