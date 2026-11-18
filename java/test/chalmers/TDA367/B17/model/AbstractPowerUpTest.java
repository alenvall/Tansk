package chalmers.TDA367.B17.model;

import static org.junit.Assert.*;

import org.junit.After;
import org.junit.AfterClass;
import org.junit.Before;
import org.junit.BeforeClass;
import org.junit.Test;
import org.newdawn.slick.geom.Vector2f;

public class AbstractPowerUpTest {
	
	class AbstractPowerUpMock extends AbstractPowerUp{
		protected AbstractPowerUpMock(){
			super(0, new Vector2f(1,1));
		}

		@Override
		public void effect() {
		}

		@Override
		public void endEffect() {
		}

		@Override
		public void updateEffect() {
		}
	}

	
	@Test
	public void testDuration() {
		AbstractPowerUp mPowerUp = new AbstractPowerUpMock();
		mPowerUp.setDuration(15000);
		assertTrue(mPowerUp.getDuration()==15000);
	}

	@Test
	public void testEffectDuration() {
		AbstractPowerUp mPowerUp = new AbstractPowerUpMock();
		mPowerUp.setEffectDuration(10000);
		assertTrue(mPowerUp.getEffectDuration()==10000);
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
