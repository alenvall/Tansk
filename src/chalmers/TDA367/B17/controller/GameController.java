package chalmers.TDA367.B17.controller;

import chalmers.TDA367.B17.console.Console;
import chalmers.TDA367.B17.model.World;

public class GameController {
	private static GameController instance;
	private World world;
	private Console console;
	
	private GameController(){
		
	}
	
	public static GameController getInstance(){
		if(instance == null)
			instance = new GameController();
		
		return instance;
	}
	
	public World getWorld(){
		return world;
	}
	
	public void setWorld(World world){
		this.world = world;
	}
	
	public Console getConsole(){
		return console;
	}
	
	public void setConsole(Console console){
		this.console = console;
	}
}
