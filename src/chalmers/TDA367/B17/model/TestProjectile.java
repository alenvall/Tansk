package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class TestProjectile extends AbstractProjectile {

	public TestProjectile(int id, Vector2f velocity, float maxSpeed,
			float minSpeed, double damage, int duration) {
		super(id, new Vector2f(5,5), 50, 0, 5, 10);
	}
	
	

}
