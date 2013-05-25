package chalmers.TDA367.B17.model;

import static org.junit.Assert.*;

import org.junit.After;
import org.junit.AfterClass;
import org.junit.Before;
import org.junit.BeforeClass;
import org.junit.Test;
import org.newdawn.slick.geom.Vector2f;

public class AbstractProjectileTest {
	
	class AbstractProjectileMock extends AbstractProjectile{
		protected AbstractProjectileMock(){
			super(0, null, new Vector2f(100,100), new Vector2f(1,0), 10f, 3f, 5.0, 5000);
		}
	}
	
	
	@Test
	public void testDamage(){
		AbstractProjectileMock mProjectile = new AbstractProjectileMock();
		mProjectile.setDamage(6.0);
		assertTrue(mProjectile.getDamage()==6.0);
	}
	
	@Test
	public void testDuration(){
		AbstractProjectileMock mProjectile = new AbstractProjectileMock();
		mProjectile.setDuration(10000);
		assertTrue(mProjectile.getDuration()==10000);
	}
	
	@Test
	public void testDurationTimer(){
		AbstractProjectileMock mProjectile = new AbstractProjectileMock();
		mProjectile.setDurationTimer(7500);
		assertTrue(mProjectile.getDurationTimer()==7500);
	}
	
	@Test
	public void testUpdate(){
		AbstractProjectileMock mProjectile = new AbstractProjectileMock();
		mProjectile.setDurationTimer(7500);
		assertTrue(mProjectile.getDurationTimer()==7500);
		mProjectile.update(30);
		assertTrue(mProjectile.getDurationTimer()==7500-30);
		mProjectile.setDuration(0);
		mProjectile.update(50);
		assertTrue(mProjectile.getDurationTimer()==7500-30);
		
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
