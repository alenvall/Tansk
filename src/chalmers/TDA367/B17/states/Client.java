package chalmers.TDA367.B17.states;

import java.util.ArrayList;
import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Image;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.SpriteSheet;
import org.newdawn.slick.geom.Shape;
import org.newdawn.slick.geom.Transform;
import org.newdawn.slick.state.BasicGameState;
import org.newdawn.slick.state.StateBasedGame;

import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.AbstractTank;
import chalmers.TDA367.B17.model.AbstractTurret;
import chalmers.TDA367.B17.model.Entity;
import chalmers.TDA367.B17.network.GameClient;

public class Client extends BasicGameState {
	private int state;
	private GameClient client;
	private Image map = null;
	private ArrayList<Entity> entities;
	private SpriteSheet entSprite;
	
	
	public Client(int state){
		this.state = state;
	}
	
	public void setClient(GameClient client){
		this.client = client;
	}
	
	@Override
	public void enter(GameContainer container, StateBasedGame game) throws SlickException {
		super.enter(container, game);
		client.requestEntities();
	}
	
	@Override
    public void init(GameContainer container, StateBasedGame game) throws SlickException {
		map = new Image("data/map.png");
		entities = new ArrayList<Entity>();
    }

	@Override
    public void update(GameContainer container, StateBasedGame game, int delta) throws SlickException {
		if(client.isEntityListUpdated())
			entities = client.getEntityList();
    }

	@Override
    public void render(GameContainer container, StateBasedGame game, Graphics g) throws SlickException {
		g.drawImage(map, 0, 0);
		if(entities != null){
			ArrayList<Entity> firstLayerEnts = new ArrayList<Entity>();
			ArrayList<Entity> secondLayerEnts = new ArrayList<Entity>();
			ArrayList<Entity> thirdLayerEnts = new ArrayList<Entity>();
			ArrayList<Entity> fourthLayerEnts = new ArrayList<Entity>();
	
			for(Entity entity : entities){			
				if(!entity.getSpriteID().equals("")){
					if(entity.getRenderLayer() == GameController.RenderLayer.FIRST)
						firstLayerEnts.add(entity);
					else if(entity.getRenderLayer() == GameController.RenderLayer.SECOND)
						secondLayerEnts.add(entity);
					else if(entity.getRenderLayer() == GameController.RenderLayer.THIRD)
						thirdLayerEnts.add(entity);
					else if(entity.getRenderLayer() == GameController.RenderLayer.FOURTH)
						fourthLayerEnts.add(entity);
				}
			}
			
			renderEntities(firstLayerEnts);
			renderEntities(secondLayerEnts);
			renderEntities(thirdLayerEnts);
			renderEntities(fourthLayerEnts);
		}
    }
	
	private void renderEntities(ArrayList<Entity> entities){
		for(Entity entity : entities){
			entSprite = GameController.getInstance().getImageHandler().getSprite(entity.getSpriteID());
			
			if(entSprite != null){
				if(entity instanceof AbstractTank){
					entSprite = GameController.getInstance().getImageHandler().getSprite(entity.getSpriteID());
					if(entity.getRotation()!=0){
							entSprite.setRotation((float) entity.getRotation());
							// draw sprite at the coordinates of the top left corner of tank when it is not rotated
							Shape nonRotatedShape = entity.getShape().transform(Transform.createRotateTransform((float)Math.toRadians(-entity.getRotation()), entity.getPosition().x, entity.getPosition().y));
							entSprite.draw(nonRotatedShape.getMinX(), nonRotatedShape.getMinY());
					} else {
						entSprite.draw(entity.getShape().getMinX(), entity.getShape().getMinY());
					}
				} else {
					if(entity instanceof AbstractTurret){
						entSprite.setCenterOfRotation(((AbstractTurret) entity).getTurretCenter().x, ((AbstractTurret) entity).getTurretCenter().y);
					}
					entSprite.setRotation((float) entity.getRotation());
					entSprite.draw(entity.getSpritePosition().x, entity.getSpritePosition().y);						
				}
			}
		}
	}

	@Override
    public int getID() {
	    return this.state;
    }
}
