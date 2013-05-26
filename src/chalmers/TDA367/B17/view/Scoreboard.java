package chalmers.TDA367.B17.view;

import org.newdawn.slick.Color;
import org.newdawn.slick.Font;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.TrueTypeFont;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.Tansk;
import chalmers.TDA367.B17.controller.GameController;
import chalmers.TDA367.B17.model.Player;

public class Scoreboard {

	private Font scoreboardFont;
	
	public Scoreboard(){

		scoreboardFont = new TrueTypeFont(new java.awt.Font("Verdana", java.awt.Font.PLAIN, 22), true);
		
	}
	
	public void render(Graphics g){
		g.setLineWidth(15);
		g.setColor(new Color(100, 100, 100, 255));
		int tmpSideLength = 350;
		int tmpYOffset = 10;
		Vector2f tmpPosition = new Vector2f(Tansk.SCREEN_WIDTH/2-tmpSideLength/2, Tansk.SCREEN_HEIGHT/2-tmpSideLength/2);
		g.fillRect(tmpPosition.x, tmpPosition.y, tmpSideLength, tmpSideLength);
		g.setColor(Color.black);
		g.drawRect(tmpPosition.x, tmpPosition.y, tmpSideLength, tmpSideLength);
		g.setLineWidth(1);
		
		g.setFont(scoreboardFont);
		for(Player p: GameController.getInstance().getGameMode().getPlayerList()){
			g.drawString(p.getName() + ": " + p.getScore(), tmpPosition.x+70, tmpPosition.y+tmpYOffset);
			tmpYOffset += 30;
		}
	}
}
