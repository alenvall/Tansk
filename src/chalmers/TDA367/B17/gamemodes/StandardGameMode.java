package chalmers.TDA367.B17.gamemodes;

import chalmers.TDA367.B17.model.Player;

 

public class StandardGameMode extends ScoreBasedGame {


	public StandardGameMode(){
		super();
	}
	
	@Override
	public void update(int delta){
		super.update(delta);
		//Check whether all players have been eliminated
		for(int i = 0; i < players.size(); i++){
			if(players.get(i).isActive() && players.get(i).isEliminated()){
				players.get(i).setActive(false);
				eliminatedPlayerCount++;
				incrementPlayerScores();
			}
		}
		
		
	}
	
	@Override
	public void gameOver(){
		super.gameOver();
		for(Player p : players){
			System.out.println(p.getName() + "'s score: " + p.getScore());
		}
	}
	
	@Override
	public void newRound(){
		super.newRound();
	}
}
