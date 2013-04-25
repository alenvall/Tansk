package chalmers.TDA367.B17.model;

import static org.junit.Assert.*;

import org.junit.Test;

public class PlayerTest {
	
	@Test
	public void testSetName(){
		Player player = new Player("Test Player");
		assertTrue(player.getName().equals("Test Player"));
		
		player.setName("New Name");
		assertTrue(player.getName().equals("New Name"));
	}
	
	@Test
	public void testSetPlayerID(){
		Player player = new Player("Test Player");
		player.setPlayerId(123);
		assertTrue(player.getPlayerId()==123);
	}
	
	@Test
	public void testSetScore(){
		Player player = new Player("Test Player");
		player.setScore(123);
		assertTrue(player.getScore()==123);
	}

}