package chalmers.TDA367.B17.model;

public abstract class AbstractPowerUp extends Entity {

	private String name;
	private double duration;
	
	public AbstractPowerUp(int id) {
		super();
		// TODO Auto-generated constructor stub
	}
	/**
	 * Get the name of the powerUp.
	 * @return The name of the powerUp
	 */
	public String getName() {
		return name;
	}
	/**
	 * Set the name of the powerUp.
	 * @param name The powerUps name
	 */
	public void setName(String name) {
		this.name = name;
	}
	/**
	 * Get the name of the powerUp.
	 * @return The name of the powerUp
	 */
	public double getDuration() {
		return duration;
	}
	/**
	 * Set the duration of the powerUp.
	 * @param duration The time in milliseconds that the 
	 * PowerUp will remain on the map
	 */
	public void setDuration(double duration) {
		this.duration = duration;
	}

}
