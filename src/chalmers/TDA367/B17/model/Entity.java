package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class Entity {
	private int id;
	private Vector2f position;
	//private type
	
	public Entity(int id) {
		this.id = id;
	}
	
	private Vector2f getPosition(){
		return position;
	}
	
	public int getId(){
		return id;
	}
	
	public void setId(int id){
		this.id = id;
	}
	
	public void setPosition(Vector2f position){
		this.position = position;
	}
}
