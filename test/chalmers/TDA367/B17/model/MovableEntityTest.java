package chalmers.TDA367.B17.model;

import static org.junit.Assert.*;

import org.junit.After;
import org.junit.AfterClass;
import org.junit.Before;
import org.junit.BeforeClass;
import org.junit.Test;
import org.newdawn.slick.geom.Vector2f;

public class MovableEntityTest {
	
	class MovableEntityMock extends MovableEntity{
		protected MovableEntityMock(){
			super(new Vector2f(), 0, 0);
		}
	}

	@Test
	public void testIsReversing() {
		MovableEntity mEntity = new MovableEntityMock();
		mEntity.setSpeed(1);
		assertTrue(!mEntity.isReversing()); //speed < 0?
	}

	@Test
	public void testSetDirection() {
		MovableEntity mEntity = new MovableEntityMock();
		mEntity.setDirection(new Vector2f(123, 456));
		assertTrue((mEntity.getDirection()).equals(new Vector2f(123, 456)));
	}

	@Test
	public void testSetSpeed() { //Not quite sure how to properly test this one
		MovableEntity mEntity = new MovableEntityMock();
		mEntity.setMaxSpeed(12);
		mEntity.setSpeed(11); //can't be higher than maxSpeed
		//-------------------//
		MovableEntity mEntity2 = new MovableEntityMock();
		mEntity2.setMaxSpeed(12);
		mEntity2.setSpeed(13);
		//-------------------//
		MovableEntity mEntity3 = new MovableEntityMock();
		mEntity3.setMinSpeed(-5);
		mEntity3.setSpeed(-6);
		
		assertTrue(mEntity.getSpeed() == 11 && mEntity2.getSpeed() == 12 && 
				mEntity3.getSpeed() == -5);
	}

	@Test
	public void testSetMaxSpeed() {
		MovableEntity mEntity = new MovableEntityMock();
		mEntity.setMaxSpeed(50);
		assertTrue(mEntity.getMaxSpeed() == 50);
	}

	@Test
	public void testSetMinSpeed() {
		MovableEntity mEntity = new MovableEntityMock();
		mEntity.setMinSpeed(50);
		assertTrue(mEntity.getMinSpeed() == 50);
	}

	@Test
	public void testSetAcceleration() {
		MovableEntity mEntity = new MovableEntityMock();
		mEntity.setAcceleration(5);
		assertTrue(mEntity.getAcceleration() == 5);
	}

	@Test
	public void testSetFriction() {
		MovableEntity mEntity = new MovableEntityMock();
		mEntity.setFriction(3);
		assertTrue(mEntity.getFriction() == 3);
	}
	
	@Test
	public void testMove() {
		MovableEntity mEntity = new MovableEntityMock();
		mEntity.setMaxSpeed(25);
		mEntity.setSpeed(10);
		mEntity.setPosition(new Vector2f(5,5));
		mEntity.setDirection(new Vector2f(1,1));
		mEntity.move(60); //(direction*speed) + position
		assertTrue(mEntity.getPosition().equals(new Vector2f(15, 15)));
	}

	@Test
	public void testAccelerate() {
		MovableEntity mEntity = new MovableEntityMock();
		mEntity.setMaxSpeed(10);
		mEntity.setSpeed(5);
		mEntity.setAcceleration(2);
		mEntity.accelerate(60);
		assertTrue(mEntity.getSpeed() == 7);
	}

	@Test
	public void testFriction() { // again, not really sure if this method is tested properly
		MovableEntity mEntity = new MovableEntityMock();
		mEntity.setMaxSpeed(10);
		mEntity.setMinSpeed(-5);
		mEntity.setSpeed(5);
		mEntity.setFriction(2);
		mEntity.friction(60);
		//-------------------//
		MovableEntity mEntity2 = new MovableEntityMock();
		mEntity2.setMaxSpeed(10);
		mEntity2.setMinSpeed(-5);
		mEntity2.setSpeed(-3);
		mEntity2.setFriction(2);
		mEntity2.friction(60);
		//-------------------//
		MovableEntity mEntity3 = new MovableEntityMock();
		mEntity3.setMaxSpeed(10);
		mEntity3.setMinSpeed(-5);
		mEntity3.setSpeed(-4);
		mEntity3.setFriction(5);
		mEntity3.friction(60);
		//-------------------//
		assertTrue(mEntity.getSpeed() == 3 && mEntity2.getSpeed() == -1 &&
				mEntity3.getSpeed() == 0);
	}

	@Test
	public void testReverse() {
		MovableEntity mEntity = new MovableEntityMock();
		mEntity.setMaxSpeed(5);
		mEntity.setSpeed(4);
		mEntity.setFriction(1.5f);
		mEntity.reverse(60);
		assertTrue(mEntity.getSpeed() == 2.5f);
	}
	
	@BeforeClass
	public static void beforeClass(){
		System.out.println("Before class!");
	}
	
	@AfterClass
	public static void afterClass(){
		System.out.println("After class!");
	}
	
	@Before
	public void before(){
		System.out.println("Before");
	}
	
	@After
	public void after(){
		System.out.println("After");
	}

}
