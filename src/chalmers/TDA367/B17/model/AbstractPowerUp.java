package chalmers.TDA367.B17.model;

public abstract class AbstractPowerUp extends Entity {

	private String name;
	private double duration;

	/**
	 * Create a new AbstractPowerUp.
	 */
	public AbstractPowerUp() {
		super();
		// TODO Auto-generated constructor stub
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
	public double getDuration() {
		return duration;
	}

	/**
	 * Set the duration of the power up.
	 * @param duration the time in milliseconds that the
	 * power up will remain on the map
	 */
	public void setDuration(double duration) {
		this.duration = duration;
	}

}
