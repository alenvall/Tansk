package chalmers.TDA367.B17.network;

import java.util.ArrayList;
import chalmers.TDA367.B17.network.Common.*;
import com.esotericsoftware.kryonet.Connection;
import com.esotericsoftware.kryonet.Listener;
import com.esotericsoftware.minlog.Log;

public class ServerListener extends Listener{

	ArrayList<Connection> cons = new ArrayList<Connection>();
	
	@Override
	public void received(Connection con, Object packet) {     
	    if(packet instanceof Pck0_LoginRequest){
	        handleLoginRequest(con, (Pck0_LoginRequest) packet);
	    }
	    if(packet instanceof Pck1_LoginAnswer){
	        //do stuff here
	    }
	    if(packet instanceof Pck2_Message){
			String message = ((Pck2_Message) packet).message;
			Log.info(message);
	    }
	}

	@Override
	public void connected(Connection connection) {
	    Log.info("[SERVER] Player connected.");
	    cons.add(connection);
	}
	
	@Override
	public void disconnected(Connection connection) {
		Log.info("[SERVER] Player disconnected.");
		cons.remove(connection);
	}
	
	public void handleLoginRequest(Connection con, Pck0_LoginRequest packet){
		Pck1_LoginAnswer loginAnswer = new Pck1_LoginAnswer();
		loginAnswer.accepted = true;
		con.sendTCP(loginAnswer);
	}
} 