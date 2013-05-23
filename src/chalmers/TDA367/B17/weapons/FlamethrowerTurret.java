package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.event.GameEvent;
import chalmers.TDA367.B17.event.GameEvent.EventType;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;

public class FlamethrowerTurret extends AbstractTurret {
	
	private static final int DEFAULT_AMMO = 400;
	private int ammoLeft;

	/**
	 * Create a new FlamethrowerTurret.
	 * @param id The id
	 * @param startingRotation The starting rotation
	 * @param tank The tank it belongs to
	 * @param color The color
	 */
	public FlamethrowerTurret(int id, Vector2f position, double startingRotation, AbstractTank tank, String color)  {
		super(id, position, startingRotation,  tank, color);
		ammoLeft = DEFAULT_AMMO;
		turretCenter = new Vector2f(16.875f, 16.875f);
		turretLength = 31.5f;
		setSize(new Vector2f(45f, 65f));
		fireRate = 33;
		projectileType = "fire";
		GameController.getInstance().getWorld().addEntity(this);
	}
	
	@Override
	public AbstractProjectile createProjectile() {	
		return new FlamethrowerProjectile(GameController.getInstance().generateID(), getTank(), getTurretNozzle());
	}

	@Override
	public void fireWeapon(int delta, AbstractTank tank){
		for(int i = 0; i < 2; i++){
			if(ammoLeft>0){
				tank.addProjectile(spawnNewProjectile());
				ammoLeft--;
				if(ammoLeft % 10 == 0){ // only play the sound every ten projectile
					GameController.getInstance().getWorld().handleEvent(new GameEvent(EventType.SOUND, this, "FLAMETHROWER_EVENT"));
				}
			}else{
				tank.setTurret(new DefaultTurret(GameController.getInstance().generateID(), getPosition(), getRotation(), getTank(), getColor()));
			}
		}
	}
}
