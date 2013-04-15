package chalmers.TDA367.B17;

import org.newdawn.slick.*;

public class Tansk extends BasicGame
{
 
  public Tansk() {
     super("Tansk!");
  }
 
  @Override
  public void init(GameContainer gc) throws SlickException {
 
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