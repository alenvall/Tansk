package chalmers.TDA367.B17.model;

public abstract class AbstractPowerUp extends Entity {

	private String name;
	
	public AbstractPowerUp(int id) {
		super(id);
		// TODO Auto-generated constructor stub
	}

	public String getName() {
		return name;
	}

	public void setName(String name) {
		this.name = name;
	}

}
