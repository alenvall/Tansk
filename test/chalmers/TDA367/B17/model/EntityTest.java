package chalmers.TDA367.B17.model;

import static org.junit.Assert.*;

import org.junit.Test;
import org.newdawn.slick.geom.Vector2f;

public class EntityTest {

	class EntityMock extends Entity{
		protected EntityMock(){
			super();
		}
	}
	
	@Test
	public void testSetSize() {
		Entity entity = new EntityMock();
		entity.setSize(new Vector2f(10, 15));
		assertTrue(entity.getSize().x == 10 && entity.getSize().y == 15);
	}

	@Test
	public void testSetId() {
		Entity entity = new EntityMock();
		entity.setId(123);
		assertTrue(entity.getId() == 123);
	}

	@Test
	public void testSetPosition() {
		Entity entity = new EntityMock();
		entity.setPosition(new Vector2f(12, 34));
		assertTrue(entity.getPosition().x == 12 && 
				entity.getPosition().y == 34);
	}
	
	@Test
	public void testIsActive(){
		Entity entity = new EntityMock();
		entity.active = false;
		assertTrue(!entity.isActive());
	}
	
	@Test
	public void testSetRotation(){
		Entity entity = new EntityMock();
		entity.setRotation(180);
		assertTrue((entity.getRotation())==(180.0));
	}
}
