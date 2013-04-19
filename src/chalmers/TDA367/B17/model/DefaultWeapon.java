package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class DefaultWeapon extends AbstractWeapon {

	public DefaultWeapon() {
		super(new DefaultProjectile(420, new Vector2f(5,5), 10, 5, 5, 10), 1000);
	}

	@Override
	public AbstractProjectile createProjectile() {
		return new DefaultProjectile(420, new Vector2f(5,5), 10, 5, 5, 10);
	}

}
