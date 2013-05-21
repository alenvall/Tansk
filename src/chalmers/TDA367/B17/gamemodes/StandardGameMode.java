package chalmers.TDA367.B17.gamemodes;

import chalmers.TDA367.B17.model.Player;

 

public class StandardGameMode extends scoreBasedGame {


	public StandardGameMode(){
		super();
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
	
	@Override
	public void newRound(){
		super.newRound();
		incrementPlayerScores();
	}
}
