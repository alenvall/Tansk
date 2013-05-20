package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.event.GameEvent.EventType;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;

public class SlowspeedyProjectile extends AbstractProjectile {
	
	private static int counter = 0;

	/**
	 * Create a new SlowspeedyProjectile.
	 * @param id The id
	 * @param tank The tank it belongs to
	 * @param position The position
	 */
	public SlowspeedyProjectile(int id, AbstractTank tank, Vector2f position) {
		super(id, tank, position, new Vector2f(1,1), 100, 0, 5, 3000);
		setSpeed(0.05f);
		setSize(new Vector2f(15,5));
		spriteID = "proj_energy";
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	@Override
	public void update(int delta) {
		if(getSpeed() < 0.1){
			setSpeed(getSpeed()*1.02f);
		}else if(getSpeed() < 0.35){
			setSpeed(0.55f);
			if(counter%5 == 0)
				GameController.getInstance().getWorld().handleEvent(new GameEvent(EventType.SOUND,this, "SLOWSPEEDY_FIRE_SECONDARY_EVENT"));
			counter++;
		}
		super.update(delta);
	}
}
