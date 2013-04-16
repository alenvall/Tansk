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
	  tank = new DefaultTank(1, new Vector2f(10,10), 100, 0, 5);
	  tank.update();
  }
 
  @Override
  public void update(GameContainer gc, int delta) throws SlickException {
 
  }
 
  @Override
  public void render(GameContainer gc, Graphics g) throws SlickException {
	  int x = (int) (1000*Math.random());
	  int y = (int) (1000*Math.random());
	  g.drawString("Hello World", x, y);
  }
 
  public static void main(String[] args) throws SlickException {
     AppGameContainer app = new AppGameContainer(new Tansk());


     app.setDisplayMode(800, 600, false);
     app.start();
  }
}