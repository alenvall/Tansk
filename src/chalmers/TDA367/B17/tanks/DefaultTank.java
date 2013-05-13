package chalmers.TDA367.B17.tanks;

import org.newdawn.slick.geom.Point;
import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Player;
import chalmers.TDA367.B17.weapons.DefaultTurret;

import chalmers.TDA367.B17.states.*;

public class DefaultTank extends AbstractTank {
	//The default maxSpeed of DefaultTank
	public static final double DEFAULT_MAXSPEED = 0.1;
	//The default minSpeed of DefaultTank
	public static final double DEFAULT_MINSPEED = -0.06;

	public DefaultTank(int id, Vector2f direction) {
		super(id, direction, (float)DEFAULT_MAXSPEED, (float)DEFAULT_MINSPEED);
		setShape(new Rectangle(100, 150, 65, 85));
		turretOffset = 6;
		turret = new DefaultTurret(1000+id);
		turret.setShape(new Point(getPosition().x, getPosition().y+getTurretOffset()));
		spriteID = "tank";
		setHealth(100);
		GameController.getInstance().getWorld().addEntity(this);
	}
}
