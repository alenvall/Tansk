package chalmers.TDA367.B17.tanks;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Player;

public class TankFactory {

	private TankFactory() {}

	public static AbstractTank getPowerUp(Player player){
		if(player.tankType.equals("default")){
			return new DefaultTank(new Vector2f(0,-1), 0.1f, -0.06f, player);
		}else{
			return new DefaultTank(new Vector2f(0,-1), 0.1f, -0.06f, player);
		}
	}

}
