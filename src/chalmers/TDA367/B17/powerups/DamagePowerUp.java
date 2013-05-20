package chalmers.TDA367.B17.powerups;

import java.util.ArrayList;
import java.util.List;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractProjectile;

public class DamagePowerUp extends AbstractPowerUp{

	//The damage multiplier
	private final int MULTIPLIER = 2;
	
	//The tank's projectiles
	private List<AbstractProjectile> projectiles;
	
	/**
	 * Create a new damage powerup at a position.
	 * @param position The position of this powerup
	 */
	public DamagePowerUp(int id, Vector2f position) {
		super(id, position);
		effectDuration = 7000;
		projectiles = new ArrayList<AbstractProjectile>();
		spriteID = "damage_powerup";
		GameController.getInstance().getWorld().addEntity(this);
	}

	@Override
	public void effect(){}
	
	@Override
	public void endEffect(){}

	@Override
	public void updateEffect() {
		for(AbstractProjectile ap : absTank.getProjectiles()){
			if(!projectiles.contains(ap)){
				ap.setDamage(ap.getDamage()*MULTIPLIER);
				projectiles.add(ap);
			}
		}
	}
}
