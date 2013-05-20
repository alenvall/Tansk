package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractTurret;

public class FireRatePowerUp extends AbstractPowerUp {

	private final float MULTIPLIER = 0.5f;
	private AbstractTurret turret;
	private int firerate;

	/**
	 * Create a new FireRatePowerUp at a position.
	 * @param position The position of this powerup
	 */
	public FireRatePowerUp(int id, Vector2f position) {
		super(id, position);
		effectDuration = 7000;
		spriteID = "firerate_powerup";
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	@Override
	public void effect(){
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
		//if the turret is changed while the powerup is active
		if(turret != absTank.getTurret()){
			effect();
		}
	}
}