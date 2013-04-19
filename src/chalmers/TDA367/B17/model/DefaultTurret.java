package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class DefaultTurret extends AbstractTurret {

	public DefaultTurret(int id) {
		super(id);
		setSize(new Vector2f(45f, 65f));
	}

}
