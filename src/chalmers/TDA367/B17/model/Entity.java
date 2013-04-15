package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class Entity {
	private int id;
	private Vector2f position;
	
	public Entity(int id) {
		this.id = id;
	}
	
	public int getID(){
		return id;
	}
}
