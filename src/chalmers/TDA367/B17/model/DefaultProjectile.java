package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class DefaultProjectile extends AbstractProjectile {

	public DefaultProjectile(AbstractTank tank) {
		super(tank, new Vector2f(1,1), 100, 0, 5, 0);
		setSpeed(20);
	}
	
	

}
