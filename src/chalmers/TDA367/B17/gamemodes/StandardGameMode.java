package chalmers.TDA367.B17.gamemodes;

 

public class StandardGameMode extends ScoreBasedGame {
	
	public StandardGameMode(int scoreLimit){
		super(scoreLimit);
		playerLives = 1;
	}
	
	@Override
	public void update(int delta){
		super.update(delta);
		//Check whether all players have been eliminated
		for(int i = 0; i < players.size(); i++){
			int j = 0;
			if(players.get(i).isEliminated()){
				j++;
			}
			eliminatedPlayerCount = j;
		}

		if((eliminatedPlayerCount >= players.size()-1 && !(players.size() <= 1))){
			endRound();
			newRoundDelayTimer(5000);
		}
	}
}
