package chalmers.TDA367.B17.network;

import java.util.ArrayList;
import java.util.Arrays;

import org.newdawn.slick.Sound;
import org.newdawn.slick.geom.Rectangle;
import org.newdawn.slick.openal.AudioImpl;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.Entity;
import chalmers.TDA367.B17.spawnpoints.PowerUpSpawnPoint;
import chalmers.TDA367.B17.spawnpoints.TankSpawnPoint;
import chalmers.TDA367.B17.terrain.BrownWall;

import com.esotericsoftware.kryo.*;
import com.esotericsoftware.kryonet.*;

public class Common {
	static public final int port = 54555;
	
	static public void register(EndPoint endPoint) {
		Kryo kryo = endPoint.getKryo();
		kryo.register(Pck0_LoginRequest.class);
		kryo.register(Pck1_LoginAnswer.class);
		kryo.register(Pck2_Message.class);
		kryo.register(Pck3_EntityListRequest.class);
		kryo.register(Pck4_EntityList.class);
		kryo.register(Arrays.asList("dummy").getClass());
//		kryo.register(TankSpawnPoint.class);
//		kryo.register(GameController.RenderLayer.class);
//		kryo.register(Rectangle.class);
//		kryo.register(float[].class);
//		kryo.register(PowerUpSpawnPoint.class);
//		kryo.register(BrownWall.class);
//		kryo.register(Sound.class);
//		kryo.register(AudioImpl.class);
//		kryo.register(TankSpawnPoint.class);
	}

	public static class Pck0_LoginRequest { }
	
	public static class Pck1_LoginAnswer {
		boolean accepted = false;
	}
	
	public static class Pck2_Message {
		String message;
	}
	
	public static class Pck3_EntityListRequest {
	}
	
	public static class Pck4_EntityList {
		ArrayList<Entity> entityList;
	}
}