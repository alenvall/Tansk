package chalmers.TDA367.B17.gamemodes;

 

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
				incrementAllPlayerScores();
			}
		}
	}
}
