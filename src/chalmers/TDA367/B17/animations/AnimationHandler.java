package chalmers.TDA367.B17.animations;

import java.util.Iterator;
import java.util.Map;
import java.util.Map.Entry;
import java.util.concurrent.ConcurrentHashMap;

import org.newdawn.slick.Animation;
import org.newdawn.slick.Image;
import org.newdawn.slick.SpriteSheet;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;

public class AnimationHandler {

	//A list containing all active animations.
	private Map<Vector2f, Animation> animations;
	
	/**
	 * Create a new AnimationHandler.
	 */
	public AnimationHandler() {
		animations = new ConcurrentHashMap<Vector2f, Animation>();
	}
	
	/**
	 * Create a new explosion animation.
	 * @param pos The position
	 */
	public void newExplosion(Vector2f pos){
		//Animation #1
		Image[] frames = new Image[90];
		for(int i = 0; i < 90; i++){
			if(i < 9)
				frames[i] = GameController.getInstance().getImageHandler().getSprite("explosion1_000" + (i+1));
			else
				frames[i] = GameController.getInstance().getImageHandler().getSprite("explosion1_00" + (i+1));
		}
		Animation ab = new Animation(frames, 25);
		ab.setLooping(false);
		animations.put(new Vector2f(450, 450), ab);
		
		//Animation #2
		SpriteSheet ss = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("explosion_animation"), 133, 133);
		Animation a = new Animation(ss, 50);
		a.setLooping(false);
		animations.put(pos, a);
	}
	
	/**
	 * Render all active animations.
	 */
	public void renderAnimations(){
		Iterator<Entry<Vector2f, Animation>> iterator = animations.entrySet().iterator();
		while(iterator.hasNext()){
			Map.Entry<Vector2f, Animation> entry = (Entry<Vector2f, Animation>) iterator.next();
			if(!entry.getValue().isStopped()){
				Vector2f pos = entry.getKey();
				entry.getValue().draw(pos.x, pos.y);
			}else{
				animations.remove(entry.getKey());
			}
		}
	}

}
