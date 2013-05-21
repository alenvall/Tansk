package chalmers.TDA367.B17.gamemodes;

import chalmers.TDA367.B17.model.Player;



public abstract class scoreBasedGame extends GameConditions{
private static int DEFAULT_SCORE_LIMIT = 3;	
private int scoreLimit;

	public scoreBasedGame(){
		this.scoreLimit = DEFAULT_SCORE_LIMIT;
	}
	
	@Override
	public void update(int delta){
		//Check whether all players have been eliminated
		for(int i = 0; i < players.size(); i++){
			if(players.get(i).isActive() && players.get(i).isEliminated()){
				players.get(i).setActive(false);
				incrementPlayerScores();
				eliminatedPlayerCount++;
			}else if(players.get(i).isActive()){
				//Keep setting the roundWinner for later use
				roundWinner = players.get(i);
			}
		}
		//Winning by score
		for(Player p: players){
			if(p.getScore() >= scoreLimit){
				setGameOver(true);
				winningPlayer = p;
			}
		}
		
	}
	
	/**
	 * Increments every player's score.
	 */
	public void incrementPlayerScores(){
		for(Player p : getPlayerList()){
			if(p.isActive())
				p.setScore(p.getScore() + 1);
		}
	}
	
	/**
	 * Return the score of a player at an index.
	 * @param index The index of the player.
	 * @return The score of the player at index.
	 */
	public int getPlayerScoreAtIndex(int index){
		return getPlayerList().get(index).getScore();
	}

	/**
	 * Set the score-limit.
	 * @param scoreLimit The new score-limit
	 */
	public void setScoreLimit(int scoreLimit) {
		this.scoreLimit = scoreLimit;
	}
	
	/**
	 * Returns the player with the highest score.
	 * return Player with the highest score
	 */
	public Player getHighestScoringPlayer(){
		Player player = getPlayerList().get(0);
		for(int i = 0; i < getPlayerList().size(); i++){
			if(getPlayerScoreAtIndex(i) > player.getScore()){
				player = getPlayerList().get(i);
			}
		}

		return player;
	}

	
	/**
	 * Return the score-limit.
	 * @return The score-limit
	 */
	public int getScoreLimit() {
		return scoreLimit;
	}
}
