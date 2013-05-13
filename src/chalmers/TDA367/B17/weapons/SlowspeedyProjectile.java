package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;

public class SlowspeedyProjectile extends AbstractProjectile {

	public SlowspeedyProjectile(int id, Vector2f position) {
		super(id, position, new Vector2f(1,1), 100, 0, 5, 3000);
		setSpeed(0.05f);
		
		setSize(new Vector2f(15,5));
		setPosition(position);
		spriteID = "proj_energy";
	}
	
	@Override
	public void update(int delta) {
		if(getSpeed() < 0.1){
			setSpeed(getSpeed()*1.01f);
		}else if(getSpeed() < 0.85){
			setSpeed(getSpeed()*1.5f);
		}
		super.update(delta);
	}
}
