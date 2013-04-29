package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class Player {
	private String name;
	private int playerId;
	private int score;
	private AbstractTank tank;
	
	/**
	 * Create a new Player.
	 * @param name The player's name
	 */
	public Player(String name){
		this.name = name;
		
		tank = new DefaultTank(new Vector2f(0,-1), 0.05f, -0.03f);
	}

	/**
	 * Get the players tank.
	 * @return The tank
	 */
	public AbstractTank getTank(){
		return tank;
	}
	
	/**
	 * Get the players name.
	 * @return The name
	 */
	public String getName() {
		return name;
	}
	
	/**
	 * Get the players ID.
	 * @return The ID
	 */
	public int getPlayerId() {
		return playerId;
	}
	
	/**
	 * Get the players score.
	 * @return The score
	 */
	public int getScore() {
		return score;
	}
	
	/**
	 * Set the players name.
	 * @param name The new name
	 */
	public void setName(String name){
		this.name = name;
	}
	
	/**
	 * Set the players ID.
	 * @param id The new ID
	 */
	public void setPlayerId(int id){
		this.playerId = id;
	}
	
	/**
	 * Set the players score.
	 * @param score The new score
	 */
	public void setScore(int score){
		this.score = score;
	}
}
