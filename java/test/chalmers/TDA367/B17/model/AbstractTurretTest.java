package chalmers.TDA367.B17.model;

import static org.junit.Assert.*;

import org.junit.After;
import org.junit.AfterClass;
import org.junit.Before;
import org.junit.BeforeClass;
import org.junit.Test;
import org.newdawn.slick.geom.Vector2f;

public class AbstractTurretTest {
	
	class AbstractTurretMock extends AbstractTurret{
		protected AbstractTurretMock(){
			super(0, new Vector2f(50,50), 90, null, "red");
		}

		@Override
		public void fireWeapon(int delta, AbstractTank tank) {
		}

		@Override
		public AbstractProjectile createProjectile() {
			return null;
		}
	}
	

	@Test
	public void testFireRate() {
		AbstractTurret mTurret = new AbstractTurretMock();
		mTurret.setFireRate(40);
		assertTrue(mTurret.getFireRate()==40);
	}

	@Test
	public void testRotation() {
		AbstractTurret mTurret = new AbstractTurretMock();
		mTurret.setRotation(180);
		assertTrue(mTurret.getRotation()==180);
		mTurret.setRotation(360);
		assertTrue(mTurret.getRotation()==0);
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
