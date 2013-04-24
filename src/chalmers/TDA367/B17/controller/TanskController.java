package chalmers.TDA367.B17.controller;

import java.awt.Point;

import chalmers.TDA367.B17.model.World;

public class TanskController {
	
	private static TanskController instance;
	private World world;
	
	private Point mouseCoordinates;

	private TanskController() {
		world = new World();
		mouseCoordinates = new Point();
		// TODO Auto-generated constructor stub
	}
	
	public static TanskController getInstance(){
		if(instance==null)
			instance = new TanskController();
		
		return instance;
	}
	
	public void newGame(){
		world = new World();
	}
	
	public World getWorld(){
		return world;
	}
	
	public Point getMouseCoordinates(){
		return mouseCoordinates;
	}
	
	public void setMouseCoordinates(int x, int y){
		mouseCoordinates.setLocation(x, y);
	}

}
