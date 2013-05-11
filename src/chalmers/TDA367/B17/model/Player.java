package chalmers.TDA367.B17.model;

import chalmers.TDA367.B17.controller.GameController;

public class Player {
	private String name;
	private int playerId;
	private int score;
	private AbstractTank tank;
	private int lives;
	private int respawnTimer;
	private int respawnTime;
	private String tankType;
	private boolean active;
	private boolean eliminated;

	/**
	 * Create a new Player.
	 * @param name The player's name
	 */
	public Player(String name){
		this.name = name;
		tankType = "default";
		active = true;
		eliminated = false;
		GameController.getInstance().getGameConditions().addPlayer(this);
		setLives(GameController.getInstance().getGameConditions().getPlayerLives());
		setRespawnTime(GameController.getInstance().getGameConditions().getSpawnTime());
	}

	/**
	 * Get the players tank.
	 * @return The tank
	 */
	public AbstractTank getTank(){
		return tank;
	}
	
	/**
	 * Get the players name.
	 * @return The name
	 */
	public String getName() {
		return name;
	}
	
	/**
	 * Get the players ID.
	 * @return The ID
	 */
	public int getPlayerId() {
		return playerId;
	}
	
	/**
	 * Get the players score.
	 * @return The score
	 */
	public int getScore() {
		return score;
	}
	
	/**
	 * Return the lives of this player.
	 * @return The lives of this player
	 */
	public int getLives() {
		return lives;
	}

	/**
	 * Set the lives of this player.
	 * @param lives Next value of lives
	 */
	public void setLives(int lives) {
		this.lives = lives;
	}

	/**
	 * Return the respawn-timer.
	 * @return The respawn-timer.
	 */
	public int getRespawnTimer() {
		return respawnTimer;
	}

	/**
	 * Set the respawn-timer.
	 * @param respawnTimer The new respawn-timer.
	 */
	public void setRespawnTimer(int respawnTimer) {
		this.respawnTimer = respawnTimer;
	}

	/**
	 * Set the new tank.
	 * @param tank The new tank
	 */
	public void setTank(AbstractTank tank) {
		this.tank = tank;
	}

	/**
	 * Set the players name.
	 * @param name The new name
	 */
	public void setName(String name){
		this.name = name;
	}
	
	/**
	 * Set the players ID.
	 * @param id The new ID
	 */
	public void setPlayerId(int id){
		this.playerId = id;
	}
	
	/**
	 * Set the players score.
	 * @param score The new score
	 */
	public void setScore(int score){
		this.score = score;
	}
	
	/**
	 * Runs when the tank dies. Decreases lives and call for spawntank.
	 */
	public void tankDeath(){
		setLives(getLives() - 1);
		setTank(null);
		if(getLives() > 0){
			spawnTank();
		}else{
			eliminated = true;
		}
	}
	
	/**
	 * Starts a new respawn-timer and adds it to the TankSpawner to respawn.
	 */
	public void spawnTank(){
		if(tank != null){
			tank.destroy();
			tank = null;
		}
		GameController.getInstance().getWorld().getTankSpawner().addPlayer(this);
		this.respawnTimer = respawnTime;
	}

	/**
	 * Checks if the player is active.
	 * @return True if active
	 */
	public boolean isActive() {
		return active;
	}

	/**
	 * Set active to true or false.
	 * @param active The new state
	 */
	public void setActive(boolean active) {
		this.active = active;
	}

	/**
	 * Return the spawn-time.
	 * @return The spawn-time
	 */
	public int getRespawnTime() {
		return respawnTime;
	}

	/**
	 * Set the respawn-time of this player.
	 * @param respawnTime The new respawn-time.
	 */
	public void setRespawnTime(int respawnTime) {
		this.respawnTime = respawnTime;
	}

	/**
	 * Check if this player has been eliminated.
	 * @return True if the tank have zero lives
	 */
	public boolean isEliminated() {
		return eliminated;
	}

	/**
	 * Set this player to be eliminated.
	 * @param eliminated The new state of eliminated
	 */
	public void setEliminated(boolean eliminated) {
		this.eliminated = eliminated;
	}

	/**
	 * Get the tank-type of this player.
	 * @return The tank-type
	 */
	public String getTankType() {
		return tankType;
	}
}
