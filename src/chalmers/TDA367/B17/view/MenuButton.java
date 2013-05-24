package chalmers.TDA367.B17.view;

import org.newdawn.slick.*;

public class MenuButton {

	private int x;
	private int y;
	public static final int WIDTH = 150;
	public static final int HEIGHT = 50;

	private boolean isPressed;
	private boolean isHovered;
	private SpriteSheet sheet;
	private SpriteSheet pressedSheet;
	private SpriteSheet hoverSheet;


	public MenuButton(int x, int y, SpriteSheet sheet, SpriteSheet pressedSheet, SpriteSheet hoverSheet) {
		this.x = x;
		this.y = y;
		isPressed = false;
		isHovered = false;
		this.sheet = sheet;
		this.pressedSheet = pressedSheet;
		this.hoverSheet = hoverSheet;
	}



	public boolean isClicked(Input input) {
		int mouseX = input.getMouseX();
		int mouseY = input.getMouseY();

		if(mouseX>=x && mouseX<=x+WIDTH && mouseY>=y && mouseY<=y+HEIGHT){
			isHovered = true;
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
			isHovered = false;
		}
		return false;
	}

	public void draw() {
		if(isPressed)
			pressedSheet.draw(x, y);
		else if(isHovered)
			hoverSheet.draw(x, y);
		else
			sheet.draw(x, y);
	}
}
