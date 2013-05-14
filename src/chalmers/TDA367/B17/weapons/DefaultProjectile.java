package chalmers.TDA367.B17.weapons;

import org.newdawn.slick.geom.*;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractTank;

public class DefaultProjectile extends AbstractProjectile {

	public DefaultProjectile(int id) {
		super(id, new Vector2f(1,1), 100, 0, 0.35, 10000);
		setSpeed(0.35f);
		setSize(new Vector2f(5f, 10f));
		spriteID = "bullet";
		setDamage(5);
		GameController.getInstance().getWorld().addEntity(this);
	}
}
