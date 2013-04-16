package chalmers.TDA367.B17;

import chalmers.TDA367.B17.model.DefaultTank;
import org.newdawn.slick.*;

import chalmers.TDA367.B17.model.World;
import org.newdawn.slick.geom.Vector2f;

public class Tansk extends BasicGame
{
	World world;
	DefaultTank tank;

  public Tansk() {
     super("Tansk!");
  }
 
  @Override
  public void init(GameContainer gc) throws SlickException {
	  world = new World();
	  tank = new DefaultTank(1, new Vector2f(0,1), 20, 0, 5);
	  tank.update();
  }
 
  @Override
  public void update(GameContainer gc, int delta) throws SlickException {
	  Input input = gc.getInput();

	  if(input.isKeyDown(Input.KEY_W)){
		  tank.accelerate();
	  }else{
		  tank.deaccelerate();
	  }
	  
	  if(input.isKeyDown(Input.KEY_A) && !input.isKeyDown(Input.KEY_D)){
		  tank.turnLeft();
	  }
	if(input.isKeyDown(Input.KEY_D) && !input.isKeyDown(Input.KEY_A)){
		  tank.turnRight();
	}
	  // update all game objects
	  tank.update();
	  
	  //System.out.println(tank.getPosition().x + "  " + tank.getAcceleration() + "  " + tank.getSpeed());
	  //System.out.println(tank.getPosition().y);
  }
 
  @Override
  public void render(GameContainer gc, Graphics g) throws SlickException {

	  g.setColor(Color.pink);
	  g.drawRect((int)tank.getPosition().x, (int)tank.getPosition().y, 100, 100);
	  g.drawString(tank.getDebugInfo(), 20, 30);
  }
 
  public static void main(String[] args) throws SlickException {
     AppGameContainer app = new AppGameContainer(new Tansk());

     app.setTargetFrameRate(60);
     app.setMaximumLogicUpdateInterval(60);
     app.setMinimumLogicUpdateInterval(10);
     app.setDisplayMode(800, 600, false);
     app.start();
  }
}