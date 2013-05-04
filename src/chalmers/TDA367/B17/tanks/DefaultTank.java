package chalmers.TDA367.B17.tanks;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Player;
import chalmers.TDA367.B17.weapons.DefaultTurret;

public class DefaultTank extends AbstractTank {

	public DefaultTank(Vector2f direction, float maxSpeed, float minSpeed, Player player) {
		super(direction, maxSpeed, minSpeed, player);
		setShape(new Rectangle(100, 150, 65, 85));
		turretOffset = 6;
		turret = new DefaultTurret(this);
		spriteID = "tank";
		setHealth(100);
	}

	@Override
//	public void didCollideWith(Entity entity){
//		Toolkit.getDefaultToolkit().beep();
//}
	
	public String getSpriteID(){
		return spriteID;
	}
}
