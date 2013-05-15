package chalmers.TDA367.B17.tanks;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Player;

public class TankFactory {

	private TankFactory() {}

	public static AbstractTank getTank(Player player){
		if(player.tankType.equals("default")){
			return new DefaultTank(GameController.getInstance().generateID());
		}else{
			return new DefaultTank(GameController.getInstance().generateID());
		}
	}
}
