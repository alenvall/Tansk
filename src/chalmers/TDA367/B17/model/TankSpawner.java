package chalmers.TDA367.B17.model;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Map.Entry;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.spawnpoints.TankSpawnPoint;
public class TankSpawner{
	
	private List<TankSpawnPoint> tankSpawnPoints;
	
	public TankSpawner(){
		tankSpawnPoints = new ArrayList<TankSpawnPoint>();
	}
	
	public void update(int delta, ArrayList<Player> players){
		ArrayList<Player> playerList = players;
		for(int i = 0; i < playerList.size(); i++){
			Player p = playerList.get(i);
			p.setRespawnTimer(p.getRespawnTimer() - delta);
			if(p.getRespawnTimer() <= 0){
				//Bad place to run this
				Iterator<Entry<Integer, Entity>> iterator = GameController.getInstance().getWorld().getEntities().entrySet().iterator();
				while(iterator.hasNext()){
					Map.Entry<Integer, Entity> entry = (Entry<Integer, Entity>) iterator.next();
					Entity entity = entry.getValue();
					if(entity instanceof TankSpawnPoint)
						GameController.getInstance().getWorld().checkCollisionsFor(entity);
				}
				getTankSpawnPoint(p);
//				playerList.remove(i);
			}
		}
	}
	
	public void addTankSpawnPoint(TankSpawnPoint tankSpawnPoint){
		tankSpawnPoints.add(tankSpawnPoint);
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
