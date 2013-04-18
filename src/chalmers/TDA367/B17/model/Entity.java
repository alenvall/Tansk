package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class Entity{
	private int id; // The if of this object
	private Vector2f position; // The position of the Entity
	private Vector2f size; // The size of the entity
	
	/**
	 * Create an Entity based on an id
	 * @param id The id of the entity
	 */
	public Entity(int id){
		this.id = id;
	}
	
	/**
	 * Get the position of this entity
	 * @return A vector containing the position
	 */
	public Vector2f getPosition(){
		return position;
	}
	
	/**
	 * Get the id of this entity
	 * @return The id
	 */
	public int getId(){
		return id;
	}
	
	/**
	 * Get the size of this entity
	 * @return The size of this entity
	 */
	public Vector2f getSize(){
		return size;
	}

	/**
	 * Set the size of this entity
	 * @param size The new size of this entity
	 */
	public void setSize(Vector2f size){
		this.size = size;
	}

	/**
	 * Set the id of this entity
	 * @param id The new id
	 */
	public void setId(int id){
		this.id = id;
	}
	
	/**
	 * Set the position of this entity
	 * @param position The values for the new position
	 */
	public void setPosition(Vector2f position){
		this.position = position;
	}
}
