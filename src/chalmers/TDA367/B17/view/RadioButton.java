package chalmers.TDA367.B17.view;

import org.newdawn.slick.Color;
import org.newdawn.slick.Graphics;
import org.newdawn.slick.Input;
import org.newdawn.slick.SpriteSheet;
import org.newdawn.slick.geom.Vector2f;

import chalmers.TDA367.B17.controller.GameController;

public class RadioButton {
	
	private boolean selected;
	private boolean isPressed = false;
	
	public static int WIDTH = 25;
	public static int HEIGHT = 25;
	
	private SpriteSheet selectedButton;
	private SpriteSheet unselectedButton;
	
	private Vector2f position;
	
	private Label label;
		
	public RadioButton(Vector2f position, boolean selected, String labelText){
		this.position = position;
		this.selected = selected;
			
		label = new Label(labelText, Color.black, (int)position.x+40, (int)position.y+5);
		
		selectedButton = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_radio_selected"), 25, 25);
		unselectedButton = new SpriteSheet(GameController.getInstance().getImageHandler().getSprite("button_radio_unselected"), 25, 25);
	}
	
	public boolean isPressed(Input input) {
		int mouseX = input.getMouseX();
		int mouseY = input.getMouseY();

		if(mouseX>=position.x && mouseX<=position.x+WIDTH && mouseY>=position.y && mouseY<=position.y+HEIGHT){
			if(input.isMouseButtonDown(Input.MOUSE_LEFT_BUTTON))
				isPressed = true;
			else{
				if(isPressed){
					isPressed = false;
					return true;
				}
			}
		}else{
			isPressed = false;
		}
		return false;
	}
	
	public void draw(Graphics g){
		label.render(g);
		if(selected)
			selectedButton.draw(position.x, position.y);
		else
			unselectedButton.draw(position.x, position.y);
	}
	
	public void setSelected(boolean selected){
		this.selected = selected;
	}
	
	public boolean isSelected(){
		return selected;
	}
}
