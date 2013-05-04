package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractPowerUp;

public class PowerUpFactory {

	private PowerUpFactory() {}

	public static AbstractPowerUp getPowerUp(String powup, Vector2f position){
		if(powup.equals("damage")){
			return new DamagePowerUp(position);
		}else if(powup.equals("firerate")){
			return new FireRatePowerUp(position);
		}else if(powup.equals("shield")){
			return new ShieldPowerUp(position);
		}else if(powup.equals("speed")){
			return new SpeedPowerUp(position);
		}else{
			return getRandomPowerUp(position);
		}
	}
	
	public static AbstractPowerUp getRandomPowerUp(Vector2f position){
		int rand = (int)(Math.random()*4);
		if(rand == 1){
			return new DamagePowerUp(position);
		}else if(rand == 2){
			return new FireRatePowerUp(position);
		}else if(rand == 3){
			return new ShieldPowerUp(position);
		}else{
			return new SpeedPowerUp(position);
		}
	}
}
