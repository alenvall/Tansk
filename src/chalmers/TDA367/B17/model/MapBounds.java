package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Rectangle;

import java.awt.*;

public class MapBounds extends Entity {
	public MapBounds(Dimension size) {
		super();
		setShape(new Rectangle(-1, -1, size.width+2, size.height+1));
	}
}
