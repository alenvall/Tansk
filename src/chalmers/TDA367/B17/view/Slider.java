package chalmers.TDA367.B17.view;

import org.newdawn.slick.Color;
import org.newdawn.slick.GameContainer;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Input;
import org.newdawn.slick.SpriteSheet;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;

public class Slider {

	private int value;
	private int maxValue;
	private int minValue;
	private Vector2f sliderPosition;
	private Vector2f markerPosition;
	
	private Label label;
	
	private GameContainer gc;
	
	private SpriteSheet slider;
	public static final int SLIDER_WIDTH = 200;
	public static final int SLIDER_HEIGHT = 20;
	
	private String labelText;
	
	private SpriteSheet sliderMarker;
	
	public Slider(int maxValue, int minValue, int defaultValue, Vector2f position, GameContainer gc, String labelText){
		value = defaultValue;
		this.maxValue = maxValue;
		this.minValue = minValue;
		this.sliderPosition = position;
		
		float n = maxValue-minValue;
		float i = defaultValue/n;
		markerPosition = new Vector2f(position.x + SLIDER_WIDTH*i, sliderPosition.y-5);
		
		sliderPosition.x -= 10;
		label = new Label(labelText, Color.black, (int)position.x, (int)position.y-25);
		this.gc = gc;
		
		this.labelText = labelText;
		

		
		slider = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("slider"), 200, 10);
		sliderMarker = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("slider_marker"), 10, 20);
	}
	
	public void update() {
		Input input = gc.getInput();
		int mouseX = input.getMouseX();
		int mouseY = input.getMouseY();
		
		new Label("Scorelimit:", Color.black, 100, 150);

		if(mouseX>=sliderPosition.x-10 && mouseX<=sliderPosition.x+10+SLIDER_WIDTH &&
				mouseY>=sliderPosition.y && mouseY<=sliderPosition.y+SLIDER_HEIGHT){
			if(input.isMouseButtonDown(Input.MOUSE_LEFT_BUTTON)){
				markerPosition = new Vector2f(mouseX, sliderPosition.y-5);
				if(markerPosition.x < sliderPosition.x-10){
					markerPosition.x -=10;
				}else if(markerPosition.x > sliderPosition.x+10){
					markerPosition.x +=10;
				}
				
				value = (int)((((markerPosition.x - sliderPosition.x)/SLIDER_WIDTH) * (maxValue-minValue)) + minValue);
				
				if(value > maxValue){
					value = maxValue;
				}else if(value < minValue){
					value = minValue;
				}
			}
		}
	}
	
	public void draw(Graphics g){
		label.setText(labelText + value);
		label.render(g);
		slider.draw(sliderPosition.x, sliderPosition.y);
		sliderMarker.draw(markerPosition.x, markerPosition.y);
	}
	
	public int getValue(){
		return value;
	}
}
