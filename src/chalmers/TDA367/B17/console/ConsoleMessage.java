package chalmers.TDA367.B17.console;

import java.text.SimpleDateFormat;
import java.util.Calendar;
import chalmers.TDA367.B17.console.Console.MsgLevel;

/** A message for use with the console */
public class ConsoleMessage {
	/** Message */
	private String message;
	/** Timestamp */
	private String timestamp;
	/** Message level */
	private MsgLevel msgLevel;
	
	/**
	 * Create a new message with a message level
	 * @param message The text message
	 * @param msgLevel The message level of the message
	 */
	public ConsoleMessage(String message, MsgLevel msgLevel){
		this.message = message;
		this.msgLevel = msgLevel;
		
		// get current time and store it as a timestamp
    	SimpleDateFormat sdf = new SimpleDateFormat("HH:mm:ss");
    	this.timestamp = sdf.format(Calendar.getInstance().getTime());
	}
	
	/**
	 * Create a new message with a standard message level
	 * @param message The text message
	 */
	public ConsoleMessage(String message){
		this(message, Console.MsgLevel.STANDARD);
	}

	/** Returns the text message of the message
	 * @return The text message
	 */
	public String getMessage() {
	    return message;
    }

	/** Returns the timestamp
	 * @return The timestamp
	 */	
	public String getTimestamp() {
	    return timestamp;
    }

	/** Returns the message level  of the message
	 * @return The message level
	 */
	public MsgLevel getMessageLevel() {
	    return msgLevel;
    }
}