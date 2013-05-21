package chalmers.TDA367.B17.gamemodes;

import org.newdawn.slick.geom.Shape;

import chalmers.TDA367.B17.model.Player;

public class KingOfTheHillMode extends ScoreBasedGame{
	
	private Shape pointZone;
	
	public KingOfTheHillMode(Shape shape){
		pointZone = shape;
	}
	
	
	@Override
	public void update(int delta){
		super.update(delta);
		for(Player p: players){
			if(pointZone.contains(p.getTank().getShape())){
				incrementPlayerScore(p);
			}
		}
		
	}
}
