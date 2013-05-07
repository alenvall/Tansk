package chalmers.TDA367.B17.network;

import java.io.IOException;
import java.util.ArrayList;

import chalmers.TDA367.B17.model.Entity;
import chalmers.TDA367.B17.network.Common.*;
import com.esotericsoftware.kryonet.Client;
import com.esotericsoftware.minlog.Log;

public class GameClient {
	private Client client;
	private ClientListener listener;
	private ArrayList<Entity> entityList;
	private boolean isEntityListUpdated = false;
	
	public GameClient(){
//		Log.set(Log.LEVEL_DEBUG);
		
		client = new Client();
		Common.register(client);
		listener = new ClientListener(this);
		listener.init(client);
		client.addListener(listener);
		
		client.start();
	}
	
	public void Connect(int timeout, String ip, int tcpPort, int udpPort){
		try {
//	        client.connect(600000, "127.0.0.1", Common.port, Common.port);
	        client.connect(timeout, ip, tcpPort, udpPort);
        } catch (IOException e) {
        	Log.info("[CLIENT] Failed to connect!");
	        e.printStackTrace();
	        client.stop();
        }
	}
	
	public void MessageToServer(String msg){
		Pck2_Message msgPacket = new Pck2_Message();
		msgPacket.message = msg;
		client.sendTCP(msgPacket);
	}

	public boolean isConnected() {
		if(listener != null){
			return listener.isConnected();
		}
		return false;
    }

	public void requestEntities() {
	    Pck3_EntityListRequest packet = new Pck3_EntityListRequest();
	    client.sendTCP(packet);	    
    }

	public void setEntityList(ArrayList<Entity> entityList) {
	    this.entityList = entityList;
		setEntityListUpdated(true);
    }

	public ArrayList<Entity> getEntityList() {
	    setEntityListUpdated(false);
		return this.entityList;
    }

	public boolean isEntityListUpdated() {
	    return isEntityListUpdated;
    }

	public void setEntityListUpdated(boolean isEntityListUpdated) {
	    this.isEntityListUpdated = isEntityListUpdated;
    }
	
}
