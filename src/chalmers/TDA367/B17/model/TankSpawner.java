package chalmers.TDA367.B17.model;

import java.util.ArrayList;
import java.util.List;

import chalmers.TDA367.B17.spawnpoints.TankSpawnPoint;

public class TankSpawner{
	
	private List<TankSpawnPoint> tankSpawnPoints;
	private List<Player> playerList;
	
	public TankSpawner(){
		tankSpawnPoints = new ArrayList<TankSpawnPoint>();
		playerList = new ArrayList<Player>();
	}
	
	public void update(int delta){
		for(int i = 0; i < playerList.size(); i++){
			Player p = playerList.get(i);
			p.setRespawnTimer(p.getRespawnTimer() - delta);
			if(p.getRespawnTimer() <= 0){
				getTankSpawnPoint(p);
				playerList.remove(i);
			}
		}
	}
	
	public void addTankSpawnPoint(TankSpawnPoint tankSpawnPoint){
		tankSpawnPoints.add(tankSpawnPoint);
	}
	
	public void addPlayer(Player player){
		playerList.add(player);
	}
	
	public void getTankSpawnPoint(Player player){
		for(TankSpawnPoint tsp : tankSpawnPoints){
			if(tsp.isSpawnable()){
				tsp.spawnTank(player);
				break;
			}
		}
	}
}
