package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.*;

public class KingOfTheHillZone extends Entity {
	
	private Shape shape;
	
	public KingOfTheHillZone(int id, Vector2f position){
		super(id);
		shape = new Circle(position.x, position.y, 20);
		spriteID = "king_of_the_hill_zone";
	}
}
