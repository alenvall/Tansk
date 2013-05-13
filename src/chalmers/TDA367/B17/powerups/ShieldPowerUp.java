package chalmers.TDA367.B17.powerups;

import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.states.ServerState;

public class ShieldPowerUp extends AbstractPowerUp {
	
	private Shield shield;

	public ShieldPowerUp(int id, Vector2f position) {
		super(id, position);
		effectDuration = 10000;
		spriteID = "shield_powerup";
	}
	
	private void createShield(){
		shield = new Shield(ServerState.getInstance().generateID(), absTank.getPosition(), absTank);
	}
	
	private void moveShield(){
		shield.setPosition(absTank.getPosition());
	}

	@Override
	public void effect() {
		createShield();
	}

	@Override
	public void endEffect() {
		shield.destroy();
	}

	@Override
	public void updateEffect() {
		if(!shield.isActive() && effectActive){
			deactivate();
		}else if(shield.isActive()){
			moveShield();
		}
	}

}
