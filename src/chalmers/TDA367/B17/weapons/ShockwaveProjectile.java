package chalmers.TDA367.B17.weapons;

import java.util.HashMap;
import java.util.Map;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.event.GameEvent.EventType;
import chalmers.TDA367.B17.model.AbstractObstacle;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Entity;
import chalmers.TDA367.B17.model.MapBounds;

public class ShockwaveProjectile extends AbstractProjectile {

	private boolean activated = false;

	//Used to handle the amount of times a single tank has been hit by a shockwave.
	protected Map<AbstractTank, Integer> tankMap = new HashMap<AbstractTank, Integer>();

	/**
	 * Create a new ShockwaveProjectile.
	 * @param id The id
	 * @param tank The tank it belongs to
	 * @param position The position
	 */
	public ShockwaveProjectile(int id, AbstractTank tank, Vector2f position) {
		super(id, tank, position, new Vector2f(1,1), 100, 0, 0.5, 1500);
		setSpeed(0.2f);
		setSize(new Vector2f(8,10));
		spriteID = "shockwave_proj";
		GameController.getInstance().getWorld().addEntity(this);
	}

	/**
	 * Launch a shockwave of ShockwaveSecondaryProjectiles.
	 */
	public void detonate(){
		GameController.getInstance().getWorld().handleEvent(new GameEvent(EventType.SOUND,this, "SHOCKWAVE_DETONATE_EVENT"));
		activated = true;
		for(int i = 0; i < 40; i++){
			ShockwaveSecondaryProjectile projectile = 
					new ShockwaveSecondaryProjectile(GameController.getInstance().generateID(), getTank(), new Vector2f(getPosition()), this);
			projectile.setDirection(new Vector2f(getRotation() + i*9));

			projectile.setPosition(getPosition());
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

	@Override
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

	/**
	 * Handle the amount of times a tank can be hit.
	 * @param tank The tank that's hit
	 * @return How many times the tank has been damaged
	 */
	public int tankDamaged(AbstractTank tank){
		if(!tankMap.containsKey(tank)){
			tankMap.put(tank, 1);
			return 1;
		}else{
			int tmp = tankMap.get(tank) + 1;
			tankMap.remove(tank);
			tankMap.put(tank, tmp);
			return tmp;
		}
	}
}
