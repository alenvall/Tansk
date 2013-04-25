package chalmers.TDA367.B17.powerup;

import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractTurret;

public class FireRatePowerUp extends AbstractPowerUp {

	public static int MULTIPLIER = 3;
	private AbstractTurret turret;
	
	public FireRatePowerUp() {
		super();
		effectDuration = 7000;
	}

	public void effect(){
		//if the turret has been changed
		turret = absTank.getTurret();
		
		//Increase the firerate for the current turret
		turret.setFireRate(turret.getFireRate()/MULTIPLIER);
	}
	
	public void endEffect(){
		if(absTank.getTurret() == turret){
			turret.setFireRate(turret.getFireRate()*MULTIPLIER);
		}
	}
}
