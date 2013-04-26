package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;

public class SlowspeedyProjectile extends AbstractProjectile {

	public SlowspeedyProjectile(AbstractTank tank, Vector2f position) {
		super(tank, new Vector2f(1,1), 100, 0, 5, 3000);
		setSpeed(2);
		setSize(new Vector2f(15,5));
		setPosition(position);
		setShape(new Rectangle(getPosition().getX() - getSize().getX() / 2, getPosition().getY() - getSize().getY() / 2, getSize().getX(), getSize().getY()));
		spriteID = "proj_energy";
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
