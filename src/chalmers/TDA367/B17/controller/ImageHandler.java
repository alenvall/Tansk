package chalmers.TDA367.B17.controller;

import org.newdawn.slick.Image;
import org.newdawn.slick.SlickException;
import org.newdawn.slick.SpriteSheet;
import java.io.File;
import java.util.HashMap;

public class ImageHandler {

	private HashMap<String, SpriteSheet> sprites = new HashMap<String, SpriteSheet>();
	
	public ImageHandler(){
		
	}
	
	/**
	 * Return a sprite given its sprite id.
	 * @param id
	 * @return sprite
	 */
	public SpriteSheet getSprite(String id){
		return sprites.get(id);
	}
	
	/**
	 * Loads all available image files from a directory.
	 * @param directory The target directory
	 */
	public void loadAllImages(String directory){
		File folder = new File(directory);
		File[] listOfFiles = folder.listFiles();

		for(File file : listOfFiles) {
			if(file.isFile()) {
				if(file.getName().contains(".png")){
					try {						
	                    Image img = new Image(directory + "/" + file.getName());
	                    SpriteSheet sprite = new SpriteSheet(img, img.getHeight(), img.getWidth());
	                    sprites.put(file.getName().substring(0, file.getName().lastIndexOf('.')), sprite);
                    } catch (SlickException e) {
                    	System.out.println("ImageHandler: Failed to load image!");
	                    e.printStackTrace();
                    }
				}
			}
		}
        System.out.println("ImageHandler: Loaded images.");
	}
}
