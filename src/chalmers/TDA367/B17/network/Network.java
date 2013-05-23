package chalmers.TDA367.B17.network;

import java.util.ArrayList;

import org.newdawn.slick.geom.Vector2f;

import com.esotericsoftware.kryo.*;
import com.esotericsoftware.kryonet.*;

public class Network {
	static public final int PORT = 54555;
	
	/**
	 * Register the network classes.
	 * @param endPoint
	 */
	static public void register(EndPoint endPoint) {
		Kryo kryo = endPoint.getKryo();
		kryo.register(Pck0_JoinRequest.class);
		kryo.register(Pck1_LoginAnswer.class);
		kryo.register(Pck2_ClientConfirmJoin.class);
		kryo.register(Pck3_Message.class);
		kryo.register(Pck31_ChatMessage.class);
		kryo.register(Pck4_ClientInput.class);
		kryo.register(Vector2f.class);
		kryo.register(Pck7_TankID.class);
		kryo.register(ArrayList.class);
		kryo.register(Pck8_EntityDestroyed.class);
		kryo.register(Pck100_WorldState.class);
		kryo.register(Pck11_PickupCreated.class);
		kryo.register(Pck102_TankUpdate.class);
		kryo.register(Pck103_ProjectileUpdate.class);
		kryo.register(Pck9_EntityCreated.class);
		kryo.register(Pck10_TankCreated.class);
		kryo.register(Pck1000_GameEvent.class);
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
	
	public static class Pck3_Message extends Packet{
		public String message;
	}
	public static class Pck31_ChatMessage extends Pck3_Message{
	}	
		
	public static class Pck4_ClientInput extends Packet{
		public boolean W_pressed;
		public boolean A_pressed;
		public boolean S_pressed;
		public boolean D_pressed;
		public boolean LMB_pressed;
        public float turretNewAngle;
	}
	
	public static class Pck7_TankID extends Packet{
		public int tankID;
	}	
	
	public static class Pck8_EntityDestroyed extends EntityPacket {
	}

	public static class Pck9_EntityCreated extends EntityPacket {
		public String identifier;
		public int possibleOwnerID;
		public String color;
	}

	public static class Pck10_TankCreated extends Pck9_EntityCreated {
		public Vector2f direction;
		public String color;
	}
	
	public static class Pck11_PickupCreated extends Pck9_EntityCreated {
		public Vector2f position;
	}
	
	public static class Pck100_WorldState extends Packet {
		public ArrayList<EntityPacket> updatePackets;
	}
	
//	public static class Pck101_StaticEntityUpdate extends EntityPacket{
//		public Vector2f position;
//	}	
	
	
	public static class Pck102_TankUpdate extends EntityPacket{
		public Vector2f tankPosition;
		public Vector2f tankDirection;
		public Vector2f turretPosition;
		public double tankHealth;
		public double tankShieldHealth;
		public Vector2f shieldPosition;
		public float turretAngle;
	}	
	
	public static class Pck103_ProjectileUpdate extends EntityPacket{
		public Vector2f projPosition;
		public Vector2f projDirection;
	}
		
	public static class Pck1000_GameEvent extends Packet{
		public int sourceID;
		public String eventDesc;
		public int eventType;
	}
}