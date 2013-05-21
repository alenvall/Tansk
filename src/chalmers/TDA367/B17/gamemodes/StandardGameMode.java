package chalmers.TDA367.B17.gamemodes;

import chalmers.TDA367.B17.model.Player;

 

public class StandardGameMode extends GameConditions {
	
private static int DEFAULT_SCORE_LIMIT = 3;	

private int scoreLimit;

	public StandardGameMode(){
		super();
		this.scoreLimit = DEFAULT_SCORE_LIMIT;
	}

	@Override
	public Player getWinningPlayer() {
		return getHighestScoringPlayer();
	}
	
	@Override
	public void gameOver(){
		super.gameOver();
		for(Player p : getPlayerList()){
			System.out.println(p.getName() + "'s score: " + p.getScore());
		}
	}
	
	/**
	 * Return the score-limit.
	 * @return The score-limit
	 */
	public int getScoreLimit() {
		return scoreLimit;
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
}
