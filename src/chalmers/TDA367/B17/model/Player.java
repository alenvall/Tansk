package chalmers.TDA367.B17.model;

import org.newdawn.slick.geom.Vector2f;

public class Player {
	private String name;
	private int playerId;
	private int score;
	private AbstractTank tank;
	
	public Player(String name){
		this.name = name;
		
		tank = new DefaultTank(1, new Vector2f(0,1), 5, 0, 5);
	}
	
	public AbstractTank getTank(){
		return tank;
	}
	
	public String getName() {
		return name;
	}

	public int getPlayerId() {
		return playerId;
	}

	public int getScore() {
		return score;
	}
	
	public void setName(String name){
		this.name = name;
	}
	
	public void setPlayerId(int id){
		this.playerId = id;
	}
	
	public void setScore(int score){
		this.score = score;
	}
}
