package chalmers.TDA367.B17.model;

public class DefaultWeapon extends AbstractWeapon {

	public DefaultWeapon() {
		super(new DefaultProjectile(), 300);
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new DefaultProjectile();
	}

}
