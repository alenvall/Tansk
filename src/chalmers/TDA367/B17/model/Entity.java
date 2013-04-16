package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class Entity {
	private int id;
	private Vector2f position;
	//private type
	private Vector2f size;
	
	public Entity(int id) {
		this.id = id;
	}
	
	public Vector2f getPosition(){
		return position;
	}
	
	public int getId(){
		return id;
	}
	
	public Vector2f getSize() {
		return size;
	}

	public void setSize(Vector2f size) {
		this.size = size;
	}

	public void setId(int id){
		this.id = id;
	}
	
	public void setPosition(Vector2f position){
		this.position = position;
	}
}
