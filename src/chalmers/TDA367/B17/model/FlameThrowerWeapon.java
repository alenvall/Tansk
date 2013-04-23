package chalmers.TDA367.B17.model;

public class FlameThrowerWeapon extends AbstractWeapon {

	public FlameThrowerWeapon() {
		super(new FlameThrowerProjectile(), 400);
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new FlameThrowerProjectile();
	}

}
