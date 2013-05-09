package chalmers.TDA367.B17.console;

import java.util.ArrayList;
import org.newdawn.slick.Color;
import org.newdawn.slick.Graphics;
import chalmers.TDA367.B17.controller.GameController;

/** A simple graphical text output console */
public class Console {
	private final static int NUMBER_OF_MESSAGES = 10;
	private final static int SCREEN_OFFSETX = 10;
	private final static int CONSOLE_WIDTH = 450;
	private final static int TIMESTAMP_OFFSET = 8;
	private final static int MESSAGE_OFFSETY = 5;
	private final static int ROW_SPACE = 18;
	private final static int MESSAGE_OFFSETX = 93;
	
	private static int consolePosY = 0;
	private static int consoleSizeY = 0;
	private static int totalY = 0;
		
	private static Console instance;
	private ArrayList<ConsoleMessage> messages;
	
	/**
	 * Create a new console
	 */
	private Console(){
		messages = new ArrayList<ConsoleMessage>();
		
		// calculate the offset from the bottom of the screen depending on the max number of messages (more messages = bigger console)
		consoleSizeY = 2*MESSAGE_OFFSETY+NUMBER_OF_MESSAGES*ROW_SPACE;
		consolePosY = GameController.SCREEN_HEIGHT - consoleSizeY - 10;
		totalY = consolePosY + consoleSizeY;
	}
	
	/**
	 * Message level - different levels give different text colors
	 */
	public static enum MsgLevel{
		/**	Red text color	 */
		ERROR, 
		/**	Yellow text color	 */
		INFO, 
		/**	Green text color	 */
		SUCCESS, 
		/**	White text color	 */
		STANDARD
	}
	
	/**
	 * Return the an instance of the singleton Console
	 * @return instance An instance of the Console
	 */
	public static Console getInstance(){
		if(instance == null)
			instance = new Console();
		
		return instance;
	}
	
	/**
	 * Add a console message to the console and remove the oldest message if the max number of messages is reached 
	 * @param conMessage The console message to add
	 */
	public static void addMsg(ConsoleMessage conMessage){
		ArrayList<ConsoleMessage> messages = Console.getInstance().getMessages();
		
		// remove of the oldest message if the number of messages exceeds max
		if(messages.size() >= NUMBER_OF_MESSAGES){
			messages.remove(0);
		}
		messages.add(conMessage);
	}
	
	/**
	 * Add a standard message to the console and remove the oldest message if the max number of messages is reached 
	 * @param message The mesage to add
	 */
	public static void addMsg(String message){
		addMsg(new ConsoleMessage(message, MsgLevel.STANDARD));
	}
		
	/**
	 * Add a message with a message level to the console and remove the oldest message if the max number of messages is reached
	 * @param message The message
	 * @param msgLevel The message level
	 */
	public static void addMsg(String message, MsgLevel msgLevel){
		addMsg(new ConsoleMessage(message, msgLevel));
	}

	/**
	 * Render the messages on the given Graphics object
	 * @param graphics The graphics object to render the messages on
	 */
	public static void renderMessages(Graphics graphics) {	
	ArrayList<ConsoleMessage> messages = Console.getInstance().getMessages();
		// draw the console "window"
		graphics.setColor(Color.white);
		graphics.drawRect(SCREEN_OFFSETX, consolePosY, CONSOLE_WIDTH, consoleSizeY);
		
		if(!messages.isEmpty()){
			// draw the oldest message above the newer messages
			for(int i = 0; i < messages.size(); i++){
				ConsoleMessage currentMsg = messages.get(i);
				
				// draw the timestamp of the message
				graphics.setColor(Color.white);
				graphics.drawString("[" + currentMsg.getTimestamp() + "] ", SCREEN_OFFSETX + TIMESTAMP_OFFSET, totalY -  ROW_SPACE*(messages.size()-i) - 10 + MESSAGE_OFFSETY);
				
				// set the color of the text depending on the message level
				if(currentMsg.getMessageLevel() == MsgLevel.ERROR)
					graphics.setColor(Color.red);
				if(currentMsg.getMessageLevel() == MsgLevel.INFO)
					graphics.setColor(Color.yellow);
				if(currentMsg.getMessageLevel() == MsgLevel.SUCCESS)
					graphics.setColor(Color.green);
				if(currentMsg.getMessageLevel() == MsgLevel.STANDARD)
					graphics.setColor(Color.white);
				
				// draw the message
				graphics.drawString(currentMsg.getMessage(), SCREEN_OFFSETX + TIMESTAMP_OFFSET + MESSAGE_OFFSETX, totalY -  ROW_SPACE*(messages.size()-i) - 10 + MESSAGE_OFFSETY);
			}
			graphics.setColor(Color.white);
		}
    }
	
	/**
	 * Clear the consoles messages
	 */
	public void clearMessages(){
		messages = new ArrayList<ConsoleMessage>();
	}
	
	/**
	 * Return a list of the messages
	 * @return The messages
	 */
	private ArrayList<ConsoleMessage> getMessages(){
		return messages;
	}
}