package chalmers.TDA367.B17.network;

import java.io.IOException;
import com.esotericsoftware.kryonet.Server;
import com.esotericsoftware.minlog.Log;

public class GameServer {
	private Server server;	
	
	public GameServer() throws IOException{
		Log.set(Log.LEVEL_DEBUG);
		
		server = new Server();
		Common.register(server);
		server.addListener(new ServerListener());
		server.bind(Common.port, Common.port);
		
		server.start();
	}
}
