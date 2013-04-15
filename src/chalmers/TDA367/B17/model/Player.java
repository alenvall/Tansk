package chalmers.TDA367.B17.model;

public class Player {
	private String name;
	private int playerId;
	private int score;
	private AbstractTank tank;
	
	public Player(String name){
		this.name = name;
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
