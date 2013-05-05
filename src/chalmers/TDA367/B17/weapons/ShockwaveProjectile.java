package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;

public class ShockwaveProjectile extends AbstractProjectile {

	private boolean activated = false;
	
	public ShockwaveProjectile(AbstractTank tank, Vector2f position) {
		super(tank, position, new Vector2f(1,1), 100, 0, 0.5, 1500);
		setSpeed(0.2f);
		setSize(new Vector2f(8,10));
		setPosition(position);
		spriteID = "shockwave_proj";
	}
	
	@Override
	public void update(int delta){
		if(getDurationTimer() <= 20 && !activated){
			activated = true;
			for(int i = 0; i < 40; i++){
				AbstractProjectile projectile = new ShockwaveSecondaryProjectile(getTank(), new Vector2f(getPosition()));
				projectile.setDirection(new Vector2f(getRotation() + i*9));
				
				getTank().addProjectile(projectile);
			}
		}
		super.update(delta);
	}
}
