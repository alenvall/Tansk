package chalmers.TDA367.B17.tanks;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.Player;

public class TankFactory {

	private TankFactory() {}

	/**
	 * Get a tank based on a player's tank type.
	 * @param player The new owner of the tank
	 * @return A tank based on a player's type
	 */
	public static AbstractTank getTank(Player player){
		if(player.tankType.equals("default")){
			return new DefaultTank(GameController.getInstance().generateID(), new Vector2f(0,-1), player);
		}else{
			return new DefaultTank(GameController.getInstance().generateID(), new Vector2f(0,-1), player);
		}
	}
}
