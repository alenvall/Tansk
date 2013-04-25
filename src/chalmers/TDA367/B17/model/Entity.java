package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Point;
import org.newdawn.slick.geom.Shape;
import org.newdawn.slick.geom.Transform;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.TanskController;

public abstract class Entity{
	protected int id; // The id of this object
	protected Vector2f position; // The position of the Entity
	protected Vector2f size; // The size of the entity
	protected boolean active;
	private Shape shape;

	/**
	 * 
	 * 
	 */
	public Entity(){
		this.id = TanskController.getInstance().getWorld().generateID();
		TanskController.getInstance().getWorld().addEntity(this);
		active = true;
		shape = new Point(-1, -1);
	}
	
	public boolean isActive(){
		return active;
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
	 * @return The id of this object
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
	
	public Vector2f getCenter(){
		return new Vector2f(size.x/2, size.y/2);
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
	 * @param id The new id for this object
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

	public Shape getShape() {
		return shape;
	}

	public void setShape(Shape shape) {
		this.shape = shape;
	}

	public void update(int delta){
		
	}

	/**
	 * Callback method for collisions.
	 * This gets called when two game objects (entities) collides.
	 * @param entity the other object
	 */
	public void didCollideWith(Entity entity){}

	public void destroy(){
		TanskController.getInstance().getWorld().removeEntity(this);
	}
}
