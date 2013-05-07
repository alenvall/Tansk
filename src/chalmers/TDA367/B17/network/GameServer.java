package chalmers.TDA367.B17.network;

import java.io.IOException;
import java.util.ArrayList;

import chalmers.TDA367.B17.model.Entity;

import com.esotericsoftware.kryonet.Server;
import com.esotericsoftware.minlog.Log;

public class GameServer {
	private Server server;
	private ArrayList<Entity> entityList;
	private boolean entityListSet = false;
	
	public GameServer() throws IOException{
		Log.set(Log.LEVEL_DEBUG);
		
		server = new Server();
		Common.register(server);
		server.addListener(new ServerListener(this));
		server.bind(Common.port, Common.port);
				
		server.start();
	}

	public ArrayList<Entity> getEntityList() {
		if(entityListSet){
			return entityList;		
		}
		return null;
    }
	
	public void setEntityList(ArrayList<Entity> entities){
		if(this.entityList != null){
			this.entityList = entities;
		} else {
			this.entityList = new ArrayList<Entity>();
			this.entityList = entities;
		}
		entityListSet = true;
	}
}
