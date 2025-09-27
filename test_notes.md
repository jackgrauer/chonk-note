## Testing Notes Mode in Chonker7

### Test Steps:
1. Run `chonker7` (or `./target/release/chonker7`)
2. Press `Ctrl+E` to enter Notes mode
3. Press `Ctrl+N` to create a new note
4. Check the debug log: `cat /Users/jack/chonker7_debug.log | tail -20`

### Expected Behavior:
- After Ctrl+N, the left pane (notes) should show:
  ```
  # New Note
  
  Start typing here...
  
  Tags:
  ```
- The cursor should be positioned after "# New Note\n"
- The status message should say "Created new note" or "New note created"

### Debug Info:
- The debug log will show if Ctrl+N is detected
- It will show the notes_rope length before and after
- If the rope length changes from 0 to ~40 characters, the note was created

### Other Notes Commands:
- Ctrl+S: Save the current note
- Ctrl+L: List all saved notes
- Ctrl+F: Search notes (shows instructions)
- Ctrl+E: Toggle back to PDF mode
