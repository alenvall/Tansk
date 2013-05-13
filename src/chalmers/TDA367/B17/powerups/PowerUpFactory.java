package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;
import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.states.*;

public class PowerUpFactory {

	private PowerUpFactory() {}

	public static AbstractPowerUp getPowerUp(String powup, Vector2f position){
		if(powup.equals("damage")){
			return new DamagePowerUp(ServerState.getInstance().generateID(), position);
		}else if(powup.equals("firerate")){
			return new FireRatePowerUp(ServerState.getInstance().generateID(), position);
		}else if(powup.equals("shield")){
			return new ShieldPowerUp(ServerState.getInstance().generateID(), position);
		}else if(powup.equals("speed")){
			return new SpeedPowerUp(ServerState.getInstance().generateID(), position);
		}else{
			return getRandomPowerUp(position);
		}
	}
	
	public static AbstractPowerUp getRandomPowerUp(Vector2f position){
		int rand = (int)(Math.random()*4);
		if(rand == 1){
			return new DamagePowerUp(ServerState.getInstance().generateID(), position);
		}else if(rand == 2){
			return new FireRatePowerUp(ServerState.getInstance().generateID(), position);
		}else if(rand == 3){
			return new ShieldPowerUp(ServerState.getInstance().generateID(), position);
		}else{
			return new SpeedPowerUp(ServerState.getInstance().generateID(), position);
		}
	}
}
