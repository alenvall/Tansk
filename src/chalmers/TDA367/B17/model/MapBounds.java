package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Rectangle;

import chalmers.TDA367.B17.controller.GameController;

import java.awt.*;

public class MapBounds extends Entity {

	/**
	 * Create a new MapBounds object. 
	 * This entity is used to check collision with the edge of the map.
	 */
	public MapBounds(int id, Dimension size) {
		super(id);
		setShape(new Rectangle(-1, -1, size.width+2, size.height+1));
		GameController.getInstance().getWorld().addEntity(this);
	}
}
