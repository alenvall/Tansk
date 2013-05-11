package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractObstacle;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Entity;
import chalmers.TDA367.B17.model.MapBounds;

public class ShockwaveProjectile extends AbstractProjectile {

	private boolean activated = false;
	
	public ShockwaveProjectile(AbstractTank tank, Vector2f position) {
		super(tank, position, new Vector2f(1,1), 100, 0, 0, 1500);
		setSpeed(0.2f);
		setSize(new Vector2f(8,10));
		setPosition(position);
		spriteID = "shockwave_proj";
	}
	
	public void detonate(){
		activated = true;
		for(int i = 0; i < 40; i++){
			AbstractProjectile projectile = new ShockwaveSecondaryProjectile(getTank(), new Vector2f(getPosition()));
			projectile.setDirection(new Vector2f(getRotation() + i*9));
			
			getTank().addProjectile(projectile);
		}
		this.destroy();
	}
	
	@Override
	public void update(int delta){
		if(getDurationTimer() <= 20 && !activated){
			detonate();
		}
		super.update(delta);
	}
	
	public void didCollideWith(Entity entity){
		if(entity instanceof MapBounds || entity instanceof AbstractObstacle || entity instanceof AbstractTank){
			if(entity instanceof AbstractTank){
				if(!activated && (AbstractTank)entity != getTank()){
					detonate();
				}
			} else if(!activated){
				detonate();
			}
		}
	}
}
