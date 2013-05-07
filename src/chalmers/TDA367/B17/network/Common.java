package chalmers.TDA367.B17.network;

import com.esotericsoftware.kryo.*;
import com.esotericsoftware.kryonet.*;

public class Common {
	static public final int port = 54555;
	
	static public void register(EndPoint endPoint) {
		Kryo kryo = endPoint.getKryo();
		kryo.register(Pck0_LoginRequest.class);
		kryo.register(Pck1_LoginAnswer.class);
		kryo.register(Pck2_Message.class);
	}

	public static class Pck0_LoginRequest { }
	
	public static class Pck1_LoginAnswer {
		boolean accepted = false;
	}
	
	public static class Pck2_Message {
		String message;
	}
}