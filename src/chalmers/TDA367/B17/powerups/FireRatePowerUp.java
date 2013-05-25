package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractTurret;

/**
 * A power-up that increases the fire-rate of a tank's weapon.
 */
public class FireRatePowerUp extends AbstractPowerUp {

	/** The firerate multiplier.*/
	private final float MULTIPLIER = 0.5f;
	
	/** The affected turret.*/
	private AbstractTurret turret;
	
	/** The default firerate. */
	private int firerate;

	/**
	 * Create a new FireRatePowerUp at a position.
	 * @param id The id
	 * @param position The position of this powerup
	 */
	public FireRatePowerUp(int id, Vector2f position) {
		super(id, position);
		setEffectDuration(7000);
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