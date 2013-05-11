package chalmers.TDA367.B17.tanks;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Player;

public class TankFactory {

	private TankFactory() {}

	public static AbstractTank getTank(Player player){
		if(player.getTankType().equals("default")){
			return new DefaultTank(new Vector2f(0,-1), player);
		}else{
			return new DefaultTank(new Vector2f(0,-1), player);
		}
	}

}
