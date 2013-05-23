package chalmers.TDA367.B17.model;

import java.util.ArrayList;

import chalmers.TDA367.B17.controller.GameController;

import com.esotericsoftware.kryonet.Connection;

public class Player {
	private int id;
	private String name;
	private int playerId;
	private int score;
	private AbstractTank tank;
	private int lives;
	private int respawnTimer;
	private int respawnTime;
	private boolean tankDead;
	private String tankType;
	private boolean active;
	private boolean eliminated;
	private Connection connection;
	private ArrayList<Boolean> inputStatuses;
	
	private String color;

	/**
	 * Create a new Player with a connection.
	 * @param connection The connection
	 * @param name The player's name
	 */
	public Player(Connection connection, String name){
		this.connection = connection;
		this.id = connection.getID();
		this.name = name;
		tankType = "default";
		active = true;
		eliminated = false;
		inputStatuses = new ArrayList<Boolean>();
		for(int i = 0; i < 6; i++){
			inputStatuses.add(false);
		}
		
		color = "white";
	}
	
	// only for single(Play)er state..
	/**
	 * Create a new player.
	 * @param name The name
	 */
	public Player(String name){
		this.connection = null;
		this.id = 0;
		this.name = name;
		tankType = "default";
		active = true;
		eliminated = false;
		inputStatuses = new ArrayList<Boolean>();
		for(int i = 0; i < 6; i++){
			inputStatuses.add(false);
		}
		
		color = "red";
	}
	
	public static final int INPT_W = 0;
	public static final int INPT_A = 1;
	public static final int INPT_S = 2;
	public static final int INPT_D = 3;
	public static final int INPT_LMB = 4;

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
		
		//TODO clear up code afer having added line below
		setActive(false);
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
//		ServerState.getInstance().addPlayer(this);
		setActive(true);
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
	 * Get the id of this player.
	 * @return The id
	 */
	public int getId() {
	    return id;
    }

	/**
	 * Get the connection associated with the player.
	 * @return connection
	 */
	public Connection getConnection() {
	    return connection;
    }
	
	/**
	 * Set the connection associated with the player.
	 * @param connection
	 */
	public void setConnection(Connection connection) {
	    this.connection = connection;
    }
	
	/**
	 * Set the players input status of a key.
	 * @param key
	 * @param pressed
	 */
	public void setInputStatus(int key, boolean pressed) {
	    inputStatuses.set(key, pressed);
    }

	/**
	 * Return the input statuses of the player.
	 * @return inputStatuses
	 */
	public ArrayList<Boolean> getInput(){
		return inputStatuses;
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
	
	/**
	 * Set the color of this player.
	 * @param color The new color
	 */
	public void setColor(String color){
		this.color = color;
	}
	
	/**
	 * Get the color of this player.
	 * @return The color
	 */
	public String getColor(){
		return color;
	}
}
