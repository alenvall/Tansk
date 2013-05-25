package chalmers.TDA367.B17.gamemodes;

 

public class StandardGameMode extends ScoreBasedGame {


	public StandardGameMode(int scoreLimit){
		super(scoreLimit);
	}
	
	@Override
	public void update(int delta){
		super.update(delta);
		//Check whether all players have been eliminated
		for(int i = 0; i < players.size(); i++){
			if(players.get(i).isActive() && players.get(i).isEliminated()){
				players.get(i).setActive(false);
				eliminatedPlayerCount++;
				incrementAllPlayerScores();
			}
		}

		if(delaying){
			endRound();
			delayTimer -= delta;
			if(delayTimer <= 0){
				
				//Start a new round when the delay is over
				newRound();
				delaying = false;
			}
		}
	}
}
