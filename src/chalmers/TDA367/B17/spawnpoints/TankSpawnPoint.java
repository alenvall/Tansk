package chalmers.TDA367.B17.spawnpoints;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractSpawnPoint;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Entity;
import chalmers.TDA367.B17.model.Player;
import chalmers.TDA367.B17.powerups.ShieldPowerUp;
import chalmers.TDA367.B17.tanks.TankFactory;

public class TankSpawnPoint extends AbstractSpawnPoint {
	
	public TankSpawnPoint(Vector2f position) {
		super(position);
		GameController.getInstance().getWorld().getTankSpawner().addTankSpawnPoint(this);
		spriteID = "tank_spawnpoint";
		Vector2f size = new Vector2f(65f, 85f);
		setShape(new Rectangle(position.getX()-size.getX()/2, position.getY()-size.getY()/2, size.getX(), size.getY()));
	}
	
	public void spawnTank(Player player) {
		AbstractTank tank = TankFactory.getTank(player);
		player.setTank(tank);
		tank.setPosition(getPosition());
		tank.setDirection(new Vector2f(getRotation()+90));
		tank.setLastDir(tank.getDirection().getTheta());
		ShieldPowerUp tmp = new ShieldPowerUp(new Vector2f(0,0));
		tmp.getShield().setHealth(9999);
		tmp.setDuration(3000);
		tmp.activate(tank);
	}
	
	@Override
	public void update(int delta){
		setSpawnable(true);
	}

	@Override
	public void spawnEntity() {}
	
	@Override
	public void didCollideWith(Entity entity){
		if(entity instanceof AbstractTank){
			setSpawnable(false);
		}
	}
	
}
