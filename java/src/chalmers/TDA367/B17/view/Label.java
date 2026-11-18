package chalmers.TDA367.B17.view;

import org.newdawn.slick.Color;
import org.newdawn.slick.Graphics;

public class Label {

	private String text;
	private Color color;
	private int x;
	private int y;

	public Label(String text, Color color, int x, int y){
		this.text = text;
		this.color = color;
		this.x = x;
		this.y = y;
	}

	public void setText(String text) {
		this.text = text;
	}

	public void setColor(Color color) {
		this.color = color;
	}

	public void setX(int x) {
		this.x = x;
	}

	public void setY(int y) {
		this.y = y;
	}

	public void render(Graphics graphics) {
		graphics.setColor(color);
		graphics.drawString(text, x, y);
	}
}
