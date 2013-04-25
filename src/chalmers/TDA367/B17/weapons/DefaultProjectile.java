package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;

public class DefaultProjectile extends AbstractProjectile {

	public DefaultProjectile() {
		super(new Vector2f(1,1), 100, 0, 5, 0);
		setSpeed(20);
	}
	
	

}
