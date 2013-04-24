package chalmers.TDA367.B17.controller;

import chalmers.TDA367.B17.model.World;

public class TanskController {
	
	private static TanskController instance;
	private World world;

	private TanskController() {
		world = new World();
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

}
