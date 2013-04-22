package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class DefaultProjectile extends AbstractProjectile {

	public DefaultProjectile(Vector2f velocity, float maxSpeed,
			float minSpeed, double damage, int duration) {
		super(new Vector2f(1,1), 100, 0, 5, 1000);
		speed = 20;
	}
	
	

}
