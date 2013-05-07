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
	
	//Used for delay between rounds
	private int delayTimer;
	private boolean delaying;
	
	//keeps track of the amount of eliminated players
	private int eliminatedPlayerCount;
	
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
	}
	
	public void newRound(){
		roundWinner = null;
		roundCounter += 1;
		roundTimer = roundTime;
		
		eliminatedPlayerCount = 0;
		
		for(Player p : players){
			p.setRespawnTime(100);
			p.eliminated = false;
			p.setActive(true);
			if(p.getTank() != null){
				p.getTank().destroy();
				p.setTank(null);
			}
			p.spawnTank();
		}
		
		setPlayerSpawnTime();
		setPlayerLives();
	}
	
	public void update(int delta){
		if(!gameOver && !delaying){
			gameTimer -= delta;
			roundTimer -= delta;
			
			if(gameTimer <= 0){
				gameOver = true;
				gameOver();
			}
			
			//Check whether all players have been eliminated
			for(int i = 0; i < players.size(); i++){
				if(players.get(i).isActive() && players.get(i).eliminated == true){
					players.get(i).setActive(false);
					incrementPlayerScores();
					eliminatedPlayerCount++;
				}else if(players.get(i).isActive()){
					//Keep setting the roundWinner for later use
					roundWinner = players.get(i);
				}
			}
			
			//Winning by rounds
			if((roundWinner != null && eliminatedPlayerCount >= players.size()-1 && !(players.size() <= 1))
					|| roundTimer <= 0){
				System.out.println("Winner of round #" + roundCounter + ": "+ roundWinner.getName());
				
				//Check if it's the last round
				if(roundCounter >= rounds){
					//The game is over.
					gameOver = true;
					gameOver();
				}else{
					//When the round is over, 
					//start a timer for a delay between rounds
					newRoundDelayTimer(5000);
				}
			}
			
			//Winning by score
			//TODO How score should be awarded
			if(true){
				
			}
		}

		if(delaying){
			delayTimer-=delta;
			if(delayTimer <= 0){
				//Start a new round when the delay is over
				newRound();
				delaying = false;
			}
		}
	}
	
	public int getDelayTimer() {
		return delayTimer;
	}

	public boolean isDelaying() {
		return delaying;
	}

	public void newRoundDelayTimer(int time){
		delaying = true;
		delayTimer = time;
	}
	
	public void gameOver(){
		if(gameOver){
			//Checking who's got the highest score
			winningPlayer = getHigestScoringPlayer();
			
			System.out.println("Winner: " + winningPlayer.getName() + "\n------------------");
			for(Player p : players){
				System.out.println(p.getName() + "'s score: " + p.getScore());
			}
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
	
	public void incrementPlayerScores(){
		for(Player p : players){
			if(p.isActive())
				p.setScore(p.getScore() + 1);
		}
	}
	
	public void setPlayerSpawnTime(){
		for(Player p : players){
			p.setRespawnTime(this.spawnTime);
		}
	}
	
	public void setPlayerLives(){
		for(int i = 0; i < players.size(); i++){
			players.get(i).setLives(this.playerLives);
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

	public int getPlayerLives() {
		return playerLives;
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
