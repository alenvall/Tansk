package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class SlowspeedyProjectile extends AbstractProjectile {

	public SlowspeedyProjectile() {
		super(new Vector2f(1,1), 100, 0, 5, 3000);
		setSpeed(2);
		spriteID = "proj_energy";
		setSize(new Vector2f(15,5));
	}

	@Override
	public void update(int delta) {
		if(getSpeed() < 10){
			setSpeed(getSpeed()*1.02f);
		}else if(getSpeed() < 150){
			setSpeed(getSpeed()*2f);
		}
		super.update(delta);
	}
}
