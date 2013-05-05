package chalmers.TDA367.B17.model;

import java.util.ArrayList;
import java.util.List;

public class GameConditions {
	
	private int scoreLimit;
	private int rounds;
	private int playerLives;
	//The time it takes for tanks to spawn (in milliseconds)
	private int spawnTime;
	private int roundCounter;
	//How long each round should last
	private int roundTime;
	private int roundTimer;
	//How long each game should last
	private int gameTimer;
	
	private boolean gameOver;
	private Player winningPlayer;
	private Player roundWinner;
	
	private List<Player> players;

	public GameConditions() {
		players = new ArrayList<Player>();
		gameOver = false;
	}
	
	public void init(int scoreLimit, int rounds, int playerLives, int spawnTime, int roundTime, int gameTime){
		this.scoreLimit = scoreLimit;
		this.rounds = rounds;
		this.playerLives = playerLives;
		this.spawnTime = spawnTime;
		this.gameTimer = gameTime;
		this.roundTime = roundTime;
		this.roundTimer = roundTime;
		roundCounter = 0;
		
		setPlayerSpawnTime(this.spawnTime);
		newRound();
	}
	
	public void newRound(){
		roundWinner = null;
		roundCounter++;
		roundTimer = roundTime;
		
		setPlayerLives(this.playerLives);
		
		System.out.println("New round!");
		//TODO Implements funcionality for starting a new round
	}
	
	public void update(int delta){
		if(!gameOver){
			gameTimer -= delta;
			roundTimer -= delta;
			
			if(gameTimer <= 0){
				gameOver = true;
				gameOver();
			}
			
			//Check whether all players have been eliminated
			int counter = 0;
			for(int i = 0; i < players.size(); i++){
				if(!players.get(i).isActive()){
					counter++; //Increased if the player is inactive
				}else{
					//Keep setting the roundWinner for later use
					roundWinner = players.get(i);
				}
			}
			
			//Winning by rounds
			if((counter >= players.size()-1 && roundWinner != null && !(players.size() <= 1))
					|| roundTimer <= 0){
				System.out.println("Winner of round #" + roundCounter + ": "+ roundWinner.getName());
				roundWinner.setScore(roundWinner.getScore() + 1);
				
				//Check if it's the last round
				if(roundCounter >= rounds){
					//The game is over.
					gameOver = true;
					gameOver();
				}else{
					//If the game isn't over, a new round begins
					try {
						Thread.sleep(5000);
					} catch (InterruptedException e) {
						e.printStackTrace();
					}
					newRound();
				}
			}
			
			//Winning by score
			//TODO How score should be awarded
			if(true){
				
			}
		}
		
	}
	
	public void gameOver(){
		if(gameOver){
			//Checking who's got the highest score
			winningPlayer = getHigestScoringPlayer();
			
			System.out.println("The winner of the game is: " + 
			winningPlayer.getName() + "\nWith a score of: " + 
					winningPlayer.getScore());
		}
	}
	
	public Player getHigestScoringPlayer(){
		Player player = players.get(0);
		for(int i = 0; i < players.size(); i++){
			if(getPlayerScoreAtIndex(i) > player.getScore()){
				player = players.get(i);
			}
		}
		
		return player;
	}
	
	public void setPlayerSpawnTime(int time){
		for(Player p : players){
			p.setRespawnTimer(time);
		}
	}
	
	public void setPlayerLives(int lives){
		for(Player p : players){
			p.setLives(lives);
		}
	}

	public int getPlayerScoreAtIndex(int index){
		return players.get(index).getScore();
	}
	
	public void addPlayer(Player player){
		players.add(player);
	}

	public int getScoreLimit() {
		return scoreLimit;
	}

	public void setScoreLimit(int scoreLimit) {
		this.scoreLimit = scoreLimit;
	}

	public int getRoundCounter() {
		return roundCounter;
	}

	public void setRoundCounter(int roundCounter) {
		this.roundCounter = roundCounter;
	}

	public boolean isGameOver() {
		return gameOver;
	}

	public void setGameOver(boolean gameOver) {
		this.gameOver = gameOver;
	}

	public Player getRoundWinner() {
		return roundWinner;
	}

	public void setRoundWinner(Player roundWinner) {
		this.roundWinner = roundWinner;
	}

	public int getRounds() {
		return rounds;
	}

	public int getSpawnTime() {
		return spawnTime;
	}

	public Player getWinningPlayer() {
		return winningPlayer;
	}

	public List<Player> getPlayers() {
		return players;
	}
}
