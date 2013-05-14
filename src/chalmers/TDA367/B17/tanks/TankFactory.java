package chalmers.TDA367.B17.tanks;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Player;
import chalmers.TDA367.B17.states.ServerState;

public class TankFactory {

	private TankFactory() {}

	public static AbstractTank getTank(Player player){
		if(player.tankType.equals("default")){
			return new DefaultTank(ServerState.getInstance().generateID());
		}else{
			return new DefaultTank(ServerState.getInstance().generateID());
		}
	}

}
