package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;

/**
 * An abstract class representing powerups.
 */
public abstract class AbstractPowerUp extends Entity {

	private String name;
	protected int duration;
	protected int effectDuration;
	protected boolean effectActive;
	protected AbstractTank absTank;

	/**
	 * Create a new AbstractPowerUp.
	 * @param id The id
	 * @param position THe position
	 */
	public AbstractPowerUp(int id, Vector2f position) {
		super(id);
		effectActive = false;
		absTank = null;

		//Increase the powerup count
		GameController.getInstance().getWorld().getSpawner().setPowerupCount
		(GameController.getInstance().getWorld().getSpawner().getPowerupCount() + 1);
	}

	/**
	 * Get the name of this power up.
	 * @return the name
	 */
	public String getName() {
		return name;
	}

	/**
	 * Set the name of this power up.
	 * @param name the new name
	 */
	public void setName(String name) {
		this.name = name;
	}

	/**
	 * Get the duration of this power up.
	 * @return the time in milliseconds that the
	 * power up will remain on the map
	 */
	public int getDuration() {
		return duration;
	}

	/**
	 * Set the duration of the power up.
	 * @param duration the time in milliseconds that the
	 * power up will remain on the map
	 */
	public void setDuration(int duration) {
		this.duration = duration;
	}
	
	/**
	 * Get the remaining time the effect is active.
	 * @return The remaining time on the effect
	 */
	public int getEffectDuration() {
		return effectDuration;
	}

	/**
	 * Set the time the effect should last for.
	 * @param effectDuration The time in milliseconds the effect should last for
	 */
	public void setEffectDuration(int effectDuration) {
		this.effectDuration = effectDuration;
	}

	@Override
	public void update(int delta) {
		if(duration != 0){
			duration -= delta;
			if(duration <= 0){
				active = false;
			}
		}
		
		if(effectActive){
			if(effectDuration > 0){
				effectDuration -= delta;
				updateEffect();
			}else{
				deactivate();
				
			}
		}else if(effectActive && absTank != null && effectDuration <= 0){
			deactivate();
		}
	}
	
	/**
	 * Run to activate the powerup for a specific tank. The powerup will no
	 * longer be rendered. 
	 * @param absTank The tank that will receive the effect
	 */
	public void activate(AbstractTank absTank){
		this.absTank = absTank;
		absTank.setCurrentPowerUp(this);
		effect();
		active = false;
		effectActive = true;
		spriteID = "";
	}
	
	/**
	 * Run to deactivate the powerup. The powerup will be removed from the 
	 * tank and the effect will wear off.
	 */
	public void deactivate(){
		if(absTank == null || !absTank.isActive())
			return;
		effectActive = false;
		updateEffect();
		endEffect();
		active = false;
		this.destroy();
		absTank.setCurrentPowerUp(null);
	}
	
	/**
	 * Runs at the activation of the powerup.
	 */
	public abstract void effect();

	/**
	 * Runs at the deactivation of the powerup.
	 */
	public abstract void endEffect();

	/**
	 * Runs every update, includes what will happen during the effect.
	 */
	public abstract void updateEffect();
}
