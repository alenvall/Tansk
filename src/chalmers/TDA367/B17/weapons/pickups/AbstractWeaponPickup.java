package chalmers.TDA367.B17.weapons.pickups;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Entity;

public abstract class AbstractWeaponPickup extends Entity{
	private String name;
	protected AbstractTank absTank;
	protected String type;

	/**
	 * Create a new AbstractWeaponPickup.
	 * @param id The id
	 * @param position The position
	 */
	public AbstractWeaponPickup(int id, Vector2f position) {
		super(id);
		absTank = null;
		Vector2f size = new Vector2f(40f, 40f);
		setShape(new Rectangle(position.getX()-size.getX()/2, position.getY()-size.getY()/2, size.getX(), size.getY()));

		//Increase the weapon count
		GameController.getInstance().getWorld().getSpawner().setWeaponCount
		(GameController.getInstance().getWorld().getSpawner().getWeaponCount() + 1);
	}

	/**
	 * Get the name of this weapon pickup.
	 * @return the name
	 */
	public String getName() {
		return name;
	}

	/**
	 * Set the name of this weapon pickup.
	 * @param name the new name
	 */
	public void setName(String name) {
		this.name = name;
	}
	
	@Override
	public void update(int delta) {
		
	}
	
	/**
	 * Activate to change the weapon (turret) of a tank.
	 * @param absTank The tank to receive the weapon
	 */
	public void activate(AbstractTank absTank){
		this.absTank = absTank;
		spriteID = "";
		this.destroy();

		//Decrease the weapon count
		GameController.getInstance().getWorld().getSpawner().setWeaponCount
		(GameController.getInstance().getWorld().getSpawner().getWeaponCount() - 1);
	}
	
	@Override
	public void didCollideWith(Entity entity){
		if(entity instanceof AbstractTank){
			activate((AbstractTank)entity);
		}
	}
}
