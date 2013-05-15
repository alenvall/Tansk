package chalmers.TDA367.B17.model;

import chalmers.TDA367.B17.controller.GameController;
//import chalmers.TDA367.B17.event.GameEvent;
import org.newdawn.slick.geom.*;

public abstract class Entity{
	protected int id; // The id of this object
	protected boolean active;
	protected Shape shape;
	protected String spriteID;
	private double rotation;
	protected RenderLayer renderLayer;	
	
	public static enum RenderLayer{
		FIRST, SECOND, THIRD, FOURTH
	}
	
	public static final String ENTITY_REMOVED_EVENT = "ENTITY_REMOVED_EVENT";

	/**
	 * Create a new Entity.
	 */
	public Entity(int id){
		this.id = id;
//		this.id = GameController.getInstance().getWorld().generateID();
//		GameController.getInstance().getWorld().addEntity(this);
		active = true;
		shape = new Point(-1, -1);
		rotation = 0;
		spriteID = "";
		renderLayer = RenderLayer.FOURTH;
	}
	
	public RenderLayer getRenderLayer(){
		return renderLayer;
	}
	
	/**
	 * Returns true of the object is active.
	 * @return True if active
	 */
	public boolean isActive(){
		return active;
	}
	
	/**
	 * Return the rotation of this entity.
	 * @return The rotation
	 */
	public double getRotation(){
		return rotation;
	}
	
	/**
	 * Set the rotation of this entity
	 * @param rotation The new rotation
	 */
	public void setRotation(double rotation){
		this.rotation = rotation;
	}
	
	/**
	 * Get the position of this entity
	 * @return A vector containing the position
	 */
	public Vector2f getPosition(){
		return new Vector2f(shape.getCenterX(), shape.getCenterY());
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
		return new Vector2f(shape.getWidth(), shape.getHeight());
	}


	/**
	 * Set the size of this entity
	 * @param size The new size of this entity
	 */
	public void setSize(Vector2f size){
		this.shape = this.shape.transform(Transform.createScaleTransform(size.getX()/this.shape.getWidth(), size.getY()/this.shape.getHeight()));
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
		if(this.shape instanceof Point)
			shape.setLocation(position.x, position.y);
		else
			this.shape = this.shape.transform(Transform.createTranslateTransform(position.getX() - getPosition().getX(), position.getY() - getPosition().getY()));
	}

	/**
	 * Get the shape of this entity.
	 * @return The shape
	 */
	public Shape getShape() {
		return shape;
	}

	/**
	 * Set the shape of this entity.
	 * @param shape The new shape
	 */
	public void setShape(Shape shape) {
		this.shape = shape;
	}
	
	/**
	 * Used for updating this entity.
	 * @param delta The time that has passed since the last update
	 */
	public void update(int delta){
		
	}

	/**
	 * Get the position of where the entity's sprite is to be drawn.
	 * @return The position of the entity's sprite.
	 */
	public Vector2f getSpritePosition(){
		return new Vector2f(getPosition().x - getShape().getWidth()/2, getPosition().y - getShape().getHeight()/2);
	}

	/**
	 * Callback method for collisions.
	 * This gets called when two game objects (entities) collides.
	 * @param entity the other object
	 */
	public void didCollideWith(Entity entity){}

	/**
	 * Remove this entity from the World.
	 */
	public void destroy(){
		GameController.getInstance().getWorld().removeEntity(this);
//		GameController.getInstance().getWorld().handleEvent(new GameEvent(this, ENTITY_REMOVED_EVENT));
	}
	
	/**
	 * Return the spriteID of this entity.
	 * @return The spriteID
	 */
	public String getSpriteID(){
		return spriteID;
	}

	/**
	 * Set the spriteID of this entity.
	 * @param spriteID The new spriteID
	 */
	public void setSpriteID(String spriteID) {
		this.spriteID = spriteID;
	}
}
