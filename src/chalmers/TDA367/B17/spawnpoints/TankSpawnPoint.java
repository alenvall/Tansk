package chalmers.TDA367.B17.spawnpoints;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractSpawnPoint;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Entity;
import chalmers.TDA367.B17.model.Player;
import chalmers.TDA367.B17.powerups.Shield;
import chalmers.TDA367.B17.tanks.TankFactory;

public class TankSpawnPoint extends AbstractSpawnPoint {

	/**
	 * Create a new TankSpawnPoint.
	 * @param position The position of this spawn-point
	 */
	public TankSpawnPoint(int id, Vector2f position) {
		super(id, position);
		GameController.getInstance().getWorld().getTankSpawner().addTankSpawnPoint(this);
		spriteID = "tank_spawnpoint";
		Vector2f size = new Vector2f(40f, 55f);
		setShape(new Rectangle(position.getX()-size.getX()/2, position.getY()-size.getY()/2, size.getX(), size.getY()));
		GameController.getInstance().getWorld().addEntity(this);
	}

	/**
	 * Spawns a new tank at the position of this TankSpawnPoint. It also adds
	 * a 3 second shield to the tank.
	 * @param player The owner of the spawned tank
	 */
	public void spawnTank(Player player) {
		AbstractTank tank = TankFactory.getTank(player);
		player.setTank(tank);
		tank.setPosition(getPosition());
		tank.setDirection(new Vector2f(getRotation()+90));
		tank.setLastDir(tank.getDirection().getTheta());

		// Adds a shield for protection that lasts 3 seconds.
		Shield tmp = new Shield(GameController.getInstance().generateID(), tank, 3000);
		tmp.setHealth(9999);
		tank.setShield(tmp);
	}

	@Override
	public void update(int delta){
		setSpawnable(true);
	}

	@Override
	public void didCollideWith(Entity entity){
		if(entity instanceof AbstractTank){
			setSpawnable(false);
		}
	}
}