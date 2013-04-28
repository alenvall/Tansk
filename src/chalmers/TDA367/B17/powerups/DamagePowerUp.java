package chalmers.TDA367.B17.powerups;

import java.util.ArrayList;
import java.util.List;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.model.AbstractProjectile;

public class DamagePowerUp extends AbstractPowerUp{

	private final int MULTIPLIER = 2;
	private List<AbstractProjectile> projectiles;
	
	public DamagePowerUp(Vector2f position) {
		super(position);
		effectDuration = 7000;
		projectiles = new ArrayList<AbstractProjectile>();
		spriteID = "damage_powerup";
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
