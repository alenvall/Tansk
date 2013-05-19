package chalmers.TDA367.B17.event;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.Entity;

public class GameEvent {

	private int sourceID;
	private String eventDesc;
	private EventType eventType;
	
	public enum EventType {
		SOUND,
		ANIM
	}

	/**
	 * Create a new GameEvent.
	 * @param source the Entity related to this event
	 * @param eventType the type of event
	 */
	public GameEvent(EventType eventType, Entity source, String eventDesc) {
		this(eventType, source.getId(), eventDesc);
	}	
	
	public GameEvent(int eventTypeOrdinal, int sourceID, String eventDesc) {
		this(EventType.values()[eventTypeOrdinal], sourceID, eventDesc);
    }
	
	public GameEvent(EventType eventType, int sourceID, String eventDesc) {
		this.eventType = eventType;
		this.sourceID = sourceID;
		this.eventDesc = eventDesc;
	}

	/**
	 * Get the source Entity.
	 * @return source Entity
	 */
	public Entity getSource() {
		return GameController.getInstance().getWorld().getEntity(sourceID);
	}
	
	public int getSourceID() {
		return sourceID;
	}	

	/**
	 * Get the event description.
	 * @return event description.
	 */
	public String getEventDesc() {
		return eventDesc;
	}
	
	public EventType getEventType() {
		return eventType;
	}
}
