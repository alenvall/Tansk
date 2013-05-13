package chalmers.TDA367.B17.network;

import java.util.ArrayList;

import org.newdawn.slick.geom.Vector2f;

import com.esotericsoftware.kryo.*;
import com.esotericsoftware.kryonet.*;

public class Network {
	static public final int PORT = 54555;
	
	static public void register(EndPoint endPoint) {
		Kryo kryo = endPoint.getKryo();
		kryo.register(Pck0_JoinRequest.class);
		kryo.register(Pck1_LoginAnswer.class);
		kryo.register(Pck2_ClientConfirmJoin.class);
		kryo.register(Pck3_ServerMessage.class);
		kryo.register(Pck4_ClientInput.class);
		kryo.register(Pck5_ClientTurretAngle.class);
		kryo.register(Pck6_CreateTank.class);
		kryo.register(Vector2f.class);
		kryo.register(Pck7_TankID.class);
		kryo.register(Pck8_TurretAngleUpdate.class);
		kryo.register(ArrayList.class);
		kryo.register(Pck101_TankUpdate.class);
		kryo.register(Pck102_TurretUpdate.class);
	}
	
	public static class Packet{
		private Connection connection;
		
		public void setConnection(Connection con){
			this.connection = con;
		}
		
		public Connection getConnection(){
			return this.connection;
		}
	}
	
	public static class EntityPacket extends Packet{
		public int entityID;
	}
	
	public static class Pck0_JoinRequest extends Packet{
		public String playerName;
	}
	
	public static class Pck1_LoginAnswer extends Packet{
		public boolean accepted = false;
		public String reason = "N/A";
	}

	public static class Pck2_ClientConfirmJoin extends Packet{
		public String message;
	}
	
	public static class Pck3_ServerMessage extends Packet{
		public String message;
	}	
	
	public static class Pck4_ClientInput extends Packet{
        public boolean pressed;
        public int keyCode;
	}
	
	public static class Pck5_ClientTurretAngle extends Packet{
        public float angle;
	}	
	
	public static class Pck6_CreateTank extends Packet{
		public int id;
		public Vector2f velocity;
	}	
	
	public static class Pck7_TankID extends Packet{
		public int tankID;
	}
	
	public static class Pck8_TurretAngleUpdate extends Packet{
		public ArrayList<Float> turretAngles;
		public ArrayList<Integer> turretIDs;
	}	
	
	public static class Pck101_TankUpdate extends EntityPacket{
		public Vector2f position;
		public Vector2f direction;
	}	
	
	public static class Pck102_TurretUpdate extends EntityPacket{
		public Vector2f position;
	}	
}