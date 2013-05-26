package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.*;

import chalmers.TDA367.B17.controller.GameController;

/**
 * KingOfTheHillZone represents the zone that rewards points in KingOfTheHillMode.
 */
public class KingOfTheHillZone extends Entity {
	
	/**
	 * Create a new KingOfTheHillZone.
	 * @param id The id
	 * @param position The position
	 */
	
	private Vector2f position;
	
	public KingOfTheHillZone(int id, Vector2f position){
		super(id);
		this.position = position;
		shape = new Circle(position.x, position.y, 45);
		spriteID = "king_of_the_hill_zone";
		GameController.getInstance().getWorld().addEntity(this);
		renderLayer = RenderLayer.FIRST;
	}
	
	public Vector2f getPosition(){
		return position;
	}
}
