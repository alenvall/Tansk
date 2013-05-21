package chalmers.TDA367.B17.model;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Map.Entry;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.spawnpoints.TankSpawnPoint;

public class TankSpawner{
	
	//A list of all the TankSpawnPoints
	private List<TankSpawnPoint> tankSpawnPoints;
	//A list of all the players that wish to spawn.
	private List<Player> playersToSpawn;
	
	public TankSpawner(){
		tankSpawnPoints = new ArrayList<TankSpawnPoint>();
		playersToSpawn = new ArrayList<Player>();
	}

	/**
	 * Used for updating the TankSpawner object.
	 * @param delta The time that has passed since the last update
	 */
	public void update(int delta){
		//Loops through the playersToSpawn list and reduces their respawn time.
		for(int i = 0; i < playersToSpawn.size(); i++){
			Player p = playersToSpawn.get(i);
			p.setRespawnTimer(p.getRespawnTimer() - delta);
			if(p.getRespawnTimer() <= 0){
				GameController controller = GameController.getInstance();
				Iterator<Entry<Integer, Entity>> iterator = controller.getWorld().getEntities().entrySet().iterator();
				while(iterator.hasNext()){
					Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) iterator.next();
					Entity entity = entry.getValue();
					if(entity instanceof TankSpawnPoint)
						controller.getWorld().checkCollisionsFor(entity);
				}
				newTankRandSpawnPoint(p);
				playersToSpawn.remove(i);
			}
		}
	}
	
	/**
	 * Add a TankSpawnPoint to the tankSpawnPoints list.
	 * @param tankSpawnPoint The TankSpawnPoint to add
	 */
	public void addTankSpawnPoint(TankSpawnPoint tankSpawnPoint){
		tankSpawnPoints.add(tankSpawnPoint);
	}

	/**
	 * Add a Player to the player list.
	 * @param player The player to add
	 */
	public void addPlayer(Player player){
		playersToSpawn.add(player);
	}
	
	/**
	 * Spawn a new tank at a random available spawnpoint.
	 * @param player The player to spawn for
	 */
	public void newTankRandSpawnPoint(Player player){
		GameController controller = GameController.getInstance();
		if(!(controller.getGameMode().getPlayerList().size() > tankSpawnPoints.size())){
			while(true){
				int index = (int)(Math.random()*(controller.getGameMode().getPlayerList().size()));
				TankSpawnPoint tmp = tankSpawnPoints.get(index);
				if(tmp.isSpawnable()){
					tmp.spawnTank(player);
					break;
				}
			}
		}
	}
}
