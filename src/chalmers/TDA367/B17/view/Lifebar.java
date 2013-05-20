package chalmers.TDA367.B17.view;

import org.newdawn.slick.Color;
import org.newdawn.slick.Graphics;

public class Lifebar{
	
private static int DEFAULT_WIDTH = 100;
private static int DEFAULT_HEIGHT = 13;
	
private int width;
private int height;	
private int x;
private int y;

	public Lifebar(int x, int y, int width, int height){
		this.width = width;
		this.height = height;
		this.x = x;
		this.y = y;
	}
	
	public Lifebar(int x, int y){
		this.width = DEFAULT_WIDTH;
		this.height = DEFAULT_HEIGHT;
		this.x = x;
		this.y = y;
	}
	public void render(double healthPercent, double shieldPercent, Graphics g){
		g.setColor(Color.black);
		g.fillRoundRect(x, y, width, height, height/2);
		
		g.setColor(Color.red);
		g.fillRoundRect(x, y, (float)(width*healthPercent), height, height/2);
		
		g.setColor(Color.black);
		g.setLineWidth(2);
		g.drawRoundRect(x, y, width, height, height/2);
		
		if(shieldPercent != 0 && shieldPercent<=1){
			g.setColor(Color.black);
			g.fillRoundRect(x, y+height+10, width, height, height/2);
			
			g.setColor(Color.green);
			g.fillRoundRect(x, y+height+10, (float)(width*shieldPercent), height, height/2);
			
			g.setColor(Color.black);
			g.setLineWidth(2);
			g.drawRoundRect(x, y+height+10, width, height, height/2);
		}
	}
	
	
}
