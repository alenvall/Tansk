package chalmers.TDA367.B17.model;

import static org.junit.Assert.*;

import org.junit.After;
import org.junit.AfterClass;
import org.junit.Before;
import org.junit.BeforeClass;
import org.junit.Test;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.MovableEntityTest.MovableEntityMock;

public class AbstractTankTest {
	
	class AbstractTankMock extends AbstractTank{
		protected AbstractTankMock(){
			super(0, new Vector2f(), 5, -3, null, "red");
		}
	}

	@Test
	public void testHealth() {
		AbstractTank mTank = new AbstractTankMock();
		mTank.setHealth(10);
		assertTrue(mTank.getHealth()==10);
	}

	@Test
	public void testTurnSpeed() {
		AbstractTank mTank = new AbstractTankMock();
		mTank.setTurnSpeed(0.5f);
		assertTrue(mTank.getTurnSpeed()==0.5f);
	}

	@Test
	public void testLastDirection() {
		AbstractTank mTank = new AbstractTankMock();
		mTank.setLastDir(90);
		assertTrue(mTank.getLastDir()==90);
	}

	@Test
	public void testTurretOffset() {
		AbstractTank mTank = new AbstractTankMock();
		mTank.setTurretOffset(15);
		assertTrue(mTank.getTurretOffset()==15);
	}

	@Test
	public void testColor() {
		AbstractTank mTank = new AbstractTankMock();
		assertTrue(mTank.getColor().equals("red"));
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
