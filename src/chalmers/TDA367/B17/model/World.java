package chalmers.TDA367.B17.model;

import java.util.*;

public class World {
	
	Map<Integer, Entity> entities = new HashMap<Integer, Entity>();

	public World() {

		Entity hej = new Entity(createNewID());
		Entity apa = new Entity(createNewID());
		Entity kaka = new Entity(createNewID());
		
		entities.put(hej.getId(), hej);
		entities.put(apa.getId(), apa);
		entities.put(kaka.getId(), kaka);
		
	}
	
	
	private int createNewID(){
		return entities.size() + 1;
	}
}
