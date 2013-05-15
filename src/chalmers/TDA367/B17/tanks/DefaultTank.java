package chalmers.TDA367.B17.tanks;

import org.newdawn.slick.geom.Rectangle;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.weapons.DefaultTurret;

public class DefaultTank extends AbstractTank {
	//The default maxSpeed of DefaultTank
	public static final double DEFAULT_MAXSPEED = 0.07;
	//The default minSpeed of DefaultTank
	public static final double DEFAULT_MINSPEED = -0.04;
	
	/**
	 * Create a new DefaultTank.
	 * @param direction The direction the tank will face.
	 * @param player The owner of this tank.
	 */
	public DefaultTank(int id) {
		super(id, (float)DEFAULT_MAXSPEED, (float)DEFAULT_MINSPEED);
		setShape(new Rectangle(100, 150, 48, 63));
		setTurretOffset(6);
		turret = new DefaultTurret(1000+id, this);
		spriteID = "tank_blue";
		setHealth(100);
		GameController.getInstance().getWorld().addEntity(this);
	}
}
