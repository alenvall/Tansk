package chalmers.TDA367.B17.powerups;

import java.util.ArrayList;
import java.util.List;

import org.newdawn.slick.geom.Transform;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.model.AbstractPowerUp;
import chalmers.TDA367.B17.weapons.ShockwaveSecondaryProjectile;

public class ShieldPowerUp extends AbstractPowerUp {
	
	private List<Shield> shieldList;

	public ShieldPowerUp(Vector2f position) {
		super(position);
		shieldList = new ArrayList<Shield>();
		effectDuration = 10000;
		spriteID = "shield_powerup";
	}
	
	private void createShield(){
		Vector2f pos = absTank.getPosition(); System.out.println(pos);
		double dir = absTank.getDirection().getTheta(); System.out.println(dir);
		for(int i = 0; i < 40; i++){
			Vector2f tmp = new Vector2f(dir + i*9); System.out.println(tmp);
			Vector2f velocity = tmp.scale(10); System.out.println(velocity);
			
			velocity.x = velocity.x * 1.5f;
			velocity.y = velocity.y * 1.5f;
			Shield s = new Shield(pos.add(velocity));
			shieldList.add(s);
			System.out.println("shield created" + shieldList.size() + " - pos: " + s.getPosition());
		}
	}
	
	private void moveShield(){
		
	}

	@Override
	public void effect() {
		createShield();
	}

	@Override
	public void endEffect() {
		for(Shield s : shieldList){
			s.destroy();
		}
	}

	@Override
	public void updateEffect() {
		// TODO Auto-generated method stub

	}

}
