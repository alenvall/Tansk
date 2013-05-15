package chalmers.TDA367.B17.event;


import chalmers.TDA367.B17.model.Entity;

public class GameEvent {

	private Entity source;
	private String eventType;

	/**
	 * Create a new GameEvent.
	 * @param source the Entity related to this event
	 * @param eventType the type of event
	 */
	public GameEvent(Entity source, String eventType) {
		this.source = source;
		this.eventType = eventType;
	}


	/**
	 * Get the source Entity.
	 * @return source Entity
	 */
	public Entity getSource() {
		return source;
	}

	/**
	 * Get the event type.
	 * @return event type.
	 */
	public String getEventType() {
		return eventType;
	}
}
