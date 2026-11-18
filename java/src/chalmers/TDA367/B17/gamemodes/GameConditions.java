package chalmers.TDA367.B17.gamemodes;

import java.util.ArrayList;
import java.util.List;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractPowerUpPickup;
import chalmers.TDA367.B17.model.AbstractProjectile;
import chalmers.TDA367.B17.model.AbstractWeaponPickup;
import chalmers.TDA367.B17.model.Entity;
import chalmers.TDA367.B17.model.Player;
import chalmers.TDA367.B17.powerups.Shield;

public abstract class GameConditions {

	/**The amount of rounds that will be played.*/
	private int rounds;
	/**The lives of each player.*/
	private int playerLives;
	/**The time it takes for tanks to spawn (in milliseconds)*/
	private int spawnTime;
	private int roundCounter;
	/**How long each round should last*/
	private int roundTime;
	private int roundTimer;
	/**How long each game should last*/
	private int gameTimer;

	/**Determines of the game is over.*/
	private boolean gameOver;

	/**Used for delay between rounds*/
	protected int delayTimer;
	protected boolean delaying;

	/**keeps track of the amount of eliminated players*/
	protected int eliminatedPlayerCount;

	/**The winner of the game.*/
	protected List<Player> winningPlayers;
	/**The winner of the latest round.*/
	protected Player roundWinner;

	/**A list of all the players.*/
	protected ArrayList<Player> players;

	/**The maximum amount of powerups that can be out at a time.*/
	private int powerupLimit = 6;

	/**The maximum amount of weapons that can be out at a time.*/
	private int weaponLimit = 6;
	
	/**has the round been ended (map cleared and such)*/
	protected boolean roundEnded;
	
	/**
	 * Create a new GameConditions object.
	 */
	public GameConditions() {
		players = new ArrayList<Player>();
		gameOver = false;
	}

	/**
	 * Initiates the object's variables.
	 * @param rounds The number of rounds that will be played.
	 * @param playerLives The number of lives each player has.
	 * @param spawnTime The spawn-time of the tanks.
	 * @param roundTime The time of each round.
	 * @param gameTime The maximum time the game will continue on for.
	 */
	public void init(int rounds, int playerLives, int spawnTime, int roundTime, int gameTime){
		this.rounds = rounds;
		this.playerLives = playerLives;
		this.spawnTime = spawnTime;
		this.gameTimer = gameTime;
		this.roundTime = roundTime;
		this.roundTimer = roundTime;
		winningPlayers = new ArrayList<Player>();
		roundCounter = 0;
	}

	/**
	 * Start a new round, reseting player lives and spawn new tanks.
	 * Removes all tanks and power-ups on the map.
	 * Also increases the roundCounter and resets the roundTimer.
	 */
	public void newRound(){
		roundEnded = false;
		roundWinner = null;
		roundCounter += 1;
		roundTimer = roundTime;
		eliminatedPlayerCount = 0;

		for(Player p : players){
			if(p.getTank() != null){
				p.getTank().destroy();
				p.setTank(null);
			}
			p.setRespawnTime(100);
			p.setEliminated(false);
			p.setActive(true);
			p.spawnTank();
		}

		setPlayerSpawnTime();
		setPlayerLives();
	}

	
	public void endRound(){
		if(!roundEnded){
			//Reset the powerup count
			GameController.getInstance().getWorld().getSpawner().setPowerupCount(0);
			//Reset the weapon count
			GameController.getInstance().getWorld().getSpawner().setWeaponCount(0);
	
			//Remove all entities
			for(Entity entity : GameController.getInstance().getWorld().getEntities().values()){
				if(entity instanceof AbstractProjectile || entity instanceof AbstractPowerUpPickup
						|| entity instanceof AbstractWeaponPickup || entity instanceof Shield){
					entity.destroy();
				}
			}
			roundEnded = true;
		}
	}
	
	/**
	 * Updates timers and checks if any player has won.
	 * @param delta The time since the last update in milliseconds
	 */
	public void update(int delta){
		if(!gameOver && !delaying){
			gameTimer -= delta;
			roundTimer -= delta;

			if(gameTimer <= 0)
				gameOver();

			if((eliminatedPlayerCount >= players.size()-1 && !(players.size() <= 1)) || roundTimer <= 0){
				newRoundDelayTimer(5000);
			}
		}	
		if(delaying){
			endRound();
			delayTimer-=delta;
			if(delayTimer <= 0){
				//Start a new round when the delay is over
				newRound();
				delaying = false;
			}
		}
	}

	/**
	 * Get the timer of the delay.
	 * @return The delay timer
	 */
	public int getDelayTimer() {
		return delayTimer;
	}

	/**
	 * Get the timer of the delay.
	 * @return The delay timer
	 */
	public boolean isDelaying() {
		return delaying;
	}

	/**
	 * Start a new delaytimer.
	 * @param Time of the new delay
	 */
	public void newRoundDelayTimer(int time){
		delaying = true;
		delayTimer = time;
	}

	/**
	 * Determines the winning player.
	 */
	public void gameOver(){
		gameOver = true;
		winningPlayers = getWinningPlayers();

		//TODO Remove prints
		if(winningPlayers.size() == 1){
			System.out.println("Winner: " + winningPlayers.get(0).getName() + "\n------------------");
		}else{
			for(Player p: winningPlayers){
				System.out.println("Winners: " + p.getName() + "\n------------------");
			}
		}
	}

	/**
	 * Set the spawn time of every player to the preset spawn-time.
	 */
	public void setPlayerSpawnTime(){
		for(Player p : players){
			p.setRespawnTime(this.spawnTime);
		}
	}

	/**
	 * Set the lives of every player to the preset lives.
	 */
	public void setPlayerLives(){
		for(int i = 0; i < players.size(); i++){
			players.get(i).setLives(this.playerLives);
		}
	}

	/**
	 * Add a player to the playerList.
	 * @param player The player you want to add
	 */
	public void addPlayer(Player player){
		players.add(player);
	}

	/**
	 * Return the number of the current round.
	 * @return The number of the current round
	 */
	public int getRoundCounter() {
		return roundCounter;
	}

	/**
	 * Set the round-counter to a number.
	 * @param roundCounter The new number of the round
	 */
	public void setRoundCounter(int roundCounter) {
		this.roundCounter = roundCounter;
	}

	/**
	 * Checks if the game is over.
	 * @return True if the game is over
	 */
	public boolean isGameOver() {
		return gameOver;
	}

	/**
	 * Set the state of gameOver.
	 * @param gameOver What gameOver should be set to (true/false)
	 */
	public void setGameOver(boolean gameOver) {
		this.gameOver = gameOver;
	}

	/**
	 * Return the winner of the latest round.
	 * @return The winner of the latest round
	 */
	public Player getRoundWinner() {
		return roundWinner;
	}

	/**
	 * Set the winner of the round.
	 * @param roundWinner The winner of the latest round
	 */
	public void setRoundWinner(Player roundWinner) {
		this.roundWinner = roundWinner;
	}

	/**
	 * The number of rounds that will be played.
	 * @return The number of round that will be played
	 */
	public int getRounds() {
		return rounds;
	}

	/**
	 * Return the default lives of each player at the start of each round.
	 * @return The default lives of each player
	 */
	public int getPlayerLives() {
		return playerLives;
	}

	/**
	 * Return the spawn-time of every player.
	 * @return The spawn of every player
	 */
	public int getSpawnTime() {
		return spawnTime;
	}

	/**
	 * Return the winning player.
	 * @return The winning player
	 */
	public abstract List<Player> getWinningPlayers();

	/**
	 * Return a list of all the players.
	 * @return A list of all the players
	 */
	public ArrayList<Player> getPlayerList() {
		return players;
	}

	/**
	 * Get the maximum amount of powerups that can be out at once.
	 * @return The powerup limit
	 */
	public int getPowerupLimit() {
		return powerupLimit;
	}

	/**
	 * Set the maximum amount of powerups that can be out at once.
	 * @param powerupLimit The new powerup limit
	 */
	public void setPowerupLimit(int powerupLimit) {
		this.powerupLimit = powerupLimit;
	}

	/**
	 * Get the maximum amount of weapons that can be out at once.
	 * @return The weapon limit
	 */
	public int getWeaponLimit() {
		return weaponLimit;
	}

	/**
	 * Set the maximum amount of weapons that can be out at once.
	 * @param powerupLimit The new weapon limit
	 */
	public void setWeaponLimit(int weaponLimit) {
		this.weaponLimit = weaponLimit;
	}
	
	/**
	 * Add a player to the winning players.
	 * @param p The player to add
	 */
	public void addWinningPlayer(Player p){
		winningPlayers.add(p);
	}
	
	/**
	 * Remove a player.
	 * @param p The player to remove
	 */
	public void removePlayer(Player p){
		if(p.getTank() != null){
			p.getTank().tankDeath();
		}
		getPlayerList().remove(p);
	}
}