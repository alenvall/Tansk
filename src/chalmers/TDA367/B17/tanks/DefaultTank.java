package chalmers.TDA367.B17.tanks;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Player;
import chalmers.TDA367.B17.weapons.DefaultTurret;

public class DefaultTank extends AbstractTank {
	//The default maxSpeed of DefaultTank
	public static final double DEFAULT_MAXSPEED = 0.1;
	//The default minSpeed of DefaultTank
	public static final double DEFAULT_MINSPEED = -0.06;

	public DefaultTank(Vector2f direction, Player player) {
		super(direction, (float)DEFAULT_MAXSPEED, (float)DEFAULT_MINSPEED, player);
		setShape(new Rectangle(100, 150, 65, 85));
		turretOffset = 6;
		turret = new DefaultTurret(this);
		spriteID = "tank";
		setHealth(100);
	}
}
