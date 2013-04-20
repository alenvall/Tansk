package chalmers.TDA367.B17.model;

public abstract class AbstractPowerUp extends Entity {

	private String name;
	private double duration;
	
	public AbstractPowerUp(int id) {
		super();
		// TODO Auto-generated constructor stub
	}

	public String getName() {
		return name;
	}

	public void setName(String name) {
		this.name = name;
	}

	public double getDuration() {
		return duration;
	}

	public void setDuration(double duration) {
		this.duration = duration;
	}

}
