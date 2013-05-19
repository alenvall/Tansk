package chalmers.TDA367.B17.model;

import static org.junit.Assert.*;

import org.junit.After;
import org.junit.AfterClass;
import org.junit.Before;
import org.junit.BeforeClass;
import org.junit.Test;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.tanks.DefaultTank;

public class AbstractProjectileTest {

	class AbstractProjectileMock extends AbstractProjectile{
		protected AbstractProjectileMock(){
			super(new DefaultTank(new Vector2f(1,2), null), new Vector2f(1,2), new Vector2f(2,1), 100, 0, 10, 15);
		}
	}
	
	@Test
	public void testSetDamage() {
		AbstractProjectile abstractProjectile = new AbstractProjectileMock();
		abstractProjectile.setDamage(10);
		assertTrue(abstractProjectile.getDamage() == 10);
	}
	
	@Test
	public void testSetDuration() {
		AbstractProjectile abstractProjectile = new AbstractProjectileMock();
		abstractProjectile.setDirection(new Vector2f(10,5));
		assertTrue(abstractProjectile.getDirection().x == 10 && abstractProjectile.getDirection().x == 5);
	}
	
	//TODO Add Update Test
	
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
