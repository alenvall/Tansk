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
	protected Shape shape;
	protected String spriteID;
	private double rotation;

	/**
	 * 
	 * 
	 */
	public Entity(){
		this.id = TanskController.getInstance().getWorld().generateID();
		TanskController.getInstance().getWorld().addEntity(this);
		active = true;
		shape = new Point(-1, -1);
		rotation = 0;
		position = new Vector2f(0,0);
		size = new Vector2f(0,0);
		spriteID = "";
	}
	
	public boolean isActive(){
		return active;
	}
	
	public double getRotation(){
		return rotation;
	}
	
	public Vector2f getCenter(){
		return new Vector2f(shape.getCenterX(), shape.getCenterY());
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
	 * Get the position of where the entity's sprite is to be drawn.
	 * @return The position of the entity's sprite.
	 */
	public Vector2f getSpritePosition(){
		return new Vector2f(position.x - size.x/2, position.y - size.y/2);
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
	
	public String getSpriteID(){
		return spriteID;
	}
}
