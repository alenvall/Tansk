package chalmers.TDA367.B17.tanks;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Player;
import chalmers.TDA367.B17.states.ServerState;

public class TankFactory {

	private TankFactory() {}

	public static AbstractTank getTank(Player player){
		if(player.tankType.equals("default")){
			return new DefaultTank(ServerState.getInstance().generateID(), new Vector2f(0,-1));
		}else{
			return new DefaultTank(ServerState.getInstance().generateID(), new Vector2f(0,-1));
		}
	}

}
