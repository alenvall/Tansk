package chalmers.TDA367.B17.model;

public abstract class AbstractPowerUp extends Entity {

	private String name;
	protected int duration;
	protected int effectDuration;
	protected boolean effectActive;
	protected boolean activated;
	protected AbstractTank absTank;

	/**
	 * Create a new AbstractPowerUp.
	 */
	public AbstractPowerUp() {
		super();
		effectActive = false;
		activated = false;
		absTank = null;
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
	
	public void update(int delta) {
		if(duration != 0){
			duration -= delta;
			if(duration <= 0){
				active = false;
			}
		}
		
		if(activated){
			effect();
			activated = false;
			effectActive = true;
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
	
	public void activate(AbstractTank absTank){
		this.absTank = absTank;
		activated = true;
	}
	
	public void deactivate(){
		if(absTank == null || !absTank.isActive())
			return;
		endEffect();
		effectActive = false;
		active = false;
		absTank.setCurrentPowerUp(null);
	}
	
	public abstract void effect();

	public abstract void endEffect();

	public abstract void updateEffect();
}
