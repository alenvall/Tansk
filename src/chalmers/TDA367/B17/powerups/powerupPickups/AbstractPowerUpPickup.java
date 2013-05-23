package chalmers.TDA367.B17.powerups.powerupPickups;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Entity;

public abstract class AbstractPowerUpPickup extends Entity{

	/**
	 * Create a new AbstractPowerUpPickup.
	 * @param id The id
	 * @param position The position
	 */
	public AbstractPowerUpPickup(int id, Vector2f position) {
		super(id);
		Vector2f size = new Vector2f(35f, 35f);
		setShape(new Rectangle(position.getX()-size.getX()/2, position.getY()-size.getY()/2, size.getX(), size.getY()));

		renderLayer = RenderLayer.THIRD;
		
		//Increase the powerup count
		GameController.getInstance().getWorld().getSpawner().setPowerupCount
		(GameController.getInstance().getWorld().getSpawner().getPowerupCount() + 1);
	}
	
	/**
	 * Activate an AbstractPowerUpPickup on a tank.
	 * @param tank The tank
	 */
	public void activate(AbstractTank tank){
		this.destroy();

		//Decrease the weapon count
		GameController.getInstance().getWorld().getSpawner().setPowerupCount
		(GameController.getInstance().getWorld().getSpawner().getPowerupCount() - 1);
	}
	
	@Override
	public void didCollideWith(Entity entity){
		if(entity instanceof AbstractTank){
			activate((AbstractTank)entity);
		}
	}
}
