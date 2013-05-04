package chalmers.TDA367.B17.controller;

import java.awt.*;

import chalmers.TDA367.B17.model.World;

public class GameController {

	public static final int SCREEN_WIDTH = 1024;
	public static final int SCREEN_HEIGHT = 768;
	public static final String DATA_FOLDER = "data";
	
	private static GameController instance;
	private World world;
	private ImageHandler imgHandler;
	
	private Point mouseCoordinates;

	private GameController() {
		mouseCoordinates = new Point();
		imgHandler = new ImageHandler();
		imgHandler.loadAllImages(DATA_FOLDER);
	}
	
	public static GameController getInstance(){
		if(instance==null)
			instance = new GameController();
		
		return instance;
	}
	
	public void newGame(int width, int height){
		world = new World(new Dimension(width, height));
		world.init();
	}
	
	public World getWorld(){
		return world;
	}
	
	public ImageHandler getImageHandler(){
		return imgHandler;
	}
	
	public Point getMouseCoordinates(){
		return mouseCoordinates;
	}
	
	public void setMouseCoordinates(int x, int y){
		mouseCoordinates.setLocation(x, y);
	}

}