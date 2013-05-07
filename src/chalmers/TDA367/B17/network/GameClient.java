package chalmers.TDA367.B17.network;

import java.io.IOException;
import com.esotericsoftware.kryonet.Client;
import com.esotericsoftware.minlog.Log;

public class GameClient {
	private Client client;

	public GameClient(){
		Log.set(Log.LEVEL_DEBUG);
		
		client = new Client();
		Common.register(client);
		ClientListener listener = new ClientListener();
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
}
