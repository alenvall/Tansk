package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.geom.Vector2f;

import java.awt.*;


public class DefaultTank extends AbstractTank {

	public DefaultTank(Vector2f direction, float maxSpeed, float minSpeed) {
		super(direction, maxSpeed, minSpeed);
		setSize(new Vector2f(65f, 85f));
		setPosition(new Vector2f(100, 150));
		setShape(new Rectangle(getPosition().getX()-getSize().getX()/2, getPosition().getY()-getSize().getY()/2, getSize().getX(), getSize().getY()));
		turret = new DefaultTurret(this);
		turretOffset = 6;
		turret.setPosition(new Vector2f(position.x, turretOffset));
		spriteID = "tank";
	}

	@Override
//	public void didCollideWith(Entity entity){
//		Toolkit.getDefaultToolkit().beep();
//}
	
	public String getSpriteID(){
		return spriteID;
	}
}
