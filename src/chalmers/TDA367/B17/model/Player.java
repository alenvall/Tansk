package chalmers.TDA367.B17.model;

import java.util.ArrayList;
import com.esotericsoftware.kryonet.Connection;
import chalmers.TDA367.B17.states.ServerState;

public class Player {
	private int id;
	private String name;
	private int playerId;
	private int score;
	private AbstractTank tank;
	private int lives;
	private int respawnTimer;
	private int respawnTime;
	public boolean tankDead;
	public String tankType;
	public boolean active;
	public boolean eliminated;
	private Connection connection;
	private ArrayList<Boolean> inputStatuses;

	/**
	 * Create a new Player.
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
//		GameController.getInstance().gameConditions.addPlayer(this);
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
	
	public int getLives() {
		return lives;
	}

	public void setLives(int lives) {
		this.lives = lives;
	}

	public int getRespawnTimer() {
		return respawnTimer;
	}

	public void setRespawnTimer(int respawnTimer) {
		this.respawnTimer = respawnTimer;
	}

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
	
	
	public void tankDeath(){
		tankDead = true;
		setLives(getLives() - 1);
		setTank(null);
		if(getLives() > 0){
			spawnTank();
		}else{
			eliminated = true;
		}
	}
	
	public void spawnTank(){
		if(tank != null){
			tank.destroy();
			tank = null;
		}
		ServerState.getInstance().addPlayer(this);
		this.respawnTimer = respawnTime;
	}

	public boolean isActive() {
		return active;
	}

	public void setActive(boolean active) {
		this.active = active;
	}

	public int getRespawnTime() {
		return respawnTime;
	}

	public void setRespawnTime(int respawnTime) {
		this.respawnTime = respawnTime;
	}

	public int getId() {
	    return id;
    }

	public Connection getConnection() {
	    return connection;
    }

	public void setConnection(Connection connection) {
	    this.connection = connection;
    }

	public void setInputStatus(int key, boolean pressed) {
	    inputStatuses.set(key, pressed);
    }

	public ArrayList<Boolean> getInput(){
		return inputStatuses;
	}
}
