package chalmers.TDA367.B17.network;

import java.util.ArrayList;

import org.newdawn.slick.geom.Vector2f;

import com.esotericsoftware.kryo.*;
import com.esotericsoftware.kryonet.*;

/** Network utility class */
public class Network {
	static public final int PORT = 54555;
	
	/**
	 * Register the network classes.
	 * @param endPoint
	 */
	static public void register(EndPoint endPoint) {
		Kryo kryo = endPoint.getKryo();
		kryo.register(Pck0_JoinRequest.class);
		kryo.register(Pck1_JoinAnswer.class);
		kryo.register(Pck2_ClientConfirmJoin.class);
		kryo.register(Pck3_Message.class);
		kryo.register(Pck3_1_ChatMessage.class);
		kryo.register(Pck4_ClientInput.class);
		kryo.register(Pck5_PlayerKicked.class);
		kryo.register(Pck6_CreatePlayer.class);
		kryo.register(Pck8_EntityDestroyed.class);
		kryo.register(Pck9_EntityCreated.class);
		kryo.register(Pck10_TankCreated.class);
		kryo.register(Pck11_StaticObjectCreated.class);
		kryo.register(Pck12_RemovePlayer.class);
		kryo.register(Pck13_UpdatePlayer.class);
		kryo.register(Pck100_WorldState.class);
		kryo.register(Pck102_TankUpdate.class);
		kryo.register(Pck103_ProjectileUpdate.class);
		kryo.register(Pck1000_GameEvent.class);
		kryo.register(ArrayList.class);
		kryo.register(Vector2f.class);
	}

	/** Packet */
	public static class Packet{
		private Connection connection;
		
		public void setConnection(Connection con){
			this.connection = con;
		}
		
		public Connection getConnection(){
			return this.connection;
		}
	}
	
	/** Entity packet */
	public static class EntityPacket extends Packet{
		public int entityID;
	}	
	
	/** Sent by the client upon connection */		
	public static class Pck0_JoinRequest extends Packet{
		public String playerName;
		public int unique;
	}

	/** Answer sent by the server */
	public static class Pck1_JoinAnswer extends Packet{
		public boolean accepted = false;
		public String reason = "N/A";
		public ArrayList<Pck6_CreatePlayer> oldPlayers;
		public int localID;
	}

	/** Sent by the client upon accepted join request */
	public static class Pck2_ClientConfirmJoin extends Packet{
		public String message;
	}

	/** Message */
	public static class Pck3_Message extends Packet{
		public String message;
	}

	/** Chat message */
	public static class Pck3_1_ChatMessage extends Pck3_Message{
	}	

	/** Contains the clients key presses (and turret angle), sent by the client on every update */	
	public static class Pck4_ClientInput extends Packet{
		public boolean W_pressed;
		public boolean A_pressed;
		public boolean S_pressed;
		public boolean D_pressed;
		public boolean LMB_pressed;
        public float turretNewAngle;
	}

	/** Player kicked */
	public static class Pck5_PlayerKicked extends Packet{
		public String reason;
	}
	
	/** Player joined */
	public static class Pck6_CreatePlayer extends Packet{
		public int id;
		public String name;
		public int score;
		public int tankID;
		public int lives;
		public boolean active;
		public boolean eliminated;
		public String color;
		public int unique;
	}

	/** Entity destroyed */
	public static class Pck8_EntityDestroyed extends EntityPacket {
	}

	/** Entity created */
	public static class Pck9_EntityCreated extends EntityPacket {
		public String identifier;
		public int possibleOwnerID;
		public String color;
	}

	/** Tank created */
	public static class Pck10_TankCreated extends Pck9_EntityCreated {
		public Vector2f direction;
		public String color;
		public int owner;
	}

	/** Static object created */
	public static class Pck11_StaticObjectCreated extends Pck9_EntityCreated {
		public Vector2f position;
	}

	/** Player removed */
	public static class Pck12_RemovePlayer extends Packet {
		public int playerID;
	}
	
	/** Player update */
	public static class Pck13_UpdatePlayer extends Packet {
		public int id;
		public int score;
		public int tankID;
		public int lives;
		public boolean active;
		public boolean eliminated;
	}
	
	/** World state */
	public static class Pck100_WorldState extends Packet {
		public ArrayList<Packet> updatePackets;
	}

	/** Tank update */
	public static class Pck102_TankUpdate extends EntityPacket{
		public Vector2f tankPosition;
		public Vector2f tankDirection;
		public Vector2f turretPosition;
		public double tankHealth;
		public double tankShieldHealth;
		public Vector2f shieldPosition;
		public float turretAngle;
	}	

	/** Projectile update */
	public static class Pck103_ProjectileUpdate extends EntityPacket{
		public Vector2f projPosition;
		public Vector2f projDirection;
	}
	
	/** Game event */
	public static class Pck1000_GameEvent extends Packet{
		public int sourceID;
		public String eventDesc;
		public int eventType;
	}
}