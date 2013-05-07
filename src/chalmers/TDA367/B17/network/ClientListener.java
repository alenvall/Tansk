package chalmers.TDA367.B17.network;

import com.esotericsoftware.kryonet.Client;
import com.esotericsoftware.kryonet.Connection;
import com.esotericsoftware.kryonet.Listener;
import com.esotericsoftware.minlog.Log;
import chalmers.TDA367.B17.network.Common.*;

public class ClientListener extends Listener {
	public Client client;
	
	public void init(Client client){
		this.client = client;
	}
	
	@Override
	public void connected(Connection connection) {
	    Log.info("[CLIENT] You have connected.");
	    client.sendTCP(new Pck0_LoginRequest());
	}
	
	@Override
	public void disconnected(Connection connection) {
		Log.info("[CLIENT] You have disconnected.");
	}
	
	@Override
	public void received(Connection c, Object packet) {
		if(packet instanceof Pck1_LoginAnswer) {
			boolean answer = ((Pck1_LoginAnswer) packet).accepted;
			
			if(answer){
				Pck2_Message mpacket = new Pck2_Message();
				mpacket.message = "Player connected!";
				client.sendTCP(mpacket);
			} else {
				c.close();
			}
		}
		
		if(packet instanceof Pck2_Message){
			String message = ((Pck2_Message) packet).message;
			Log.info(message);
		}
	}
}
