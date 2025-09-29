# Chonker7 Test Plan

## Core Functionality Checklist

### 1. Navigation
- [ ] Up arrow moves cursor up
- [ ] Down arrow moves cursor down
- [ ] Left arrow moves cursor left
- [ ] Right arrow moves cursor right
- [ ] Cursor can move into virtual space (beyond text)
- [ ] Page redraws after cursor movement

### 2. Mouse Interaction
- [ ] Click positions cursor correctly
- [ ] Click in virtual space works
- [ ] Drag creates block selection
- [ ] No stray Alt key requirement

### 3. Text Operations
- [ ] Type text at cursor position
- [ ] Type in virtual space (auto-padding)
- [ ] Backspace deletes character
- [ ] Cut block selection (Cmd+X)
- [ ] Paste block selection (Cmd+V)
- [ ] Cut leaves spaces (non-collapsing)

### 4. Visual Feedback
- [ ] Cursor visible at all positions
- [ ] Cursor visible in virtual space
- [ ] Block selection renders correctly
- [ ] No random blue selection boxes after cut

### 5. Pane Management
- [ ] Switch between notes and extraction panes
- [ ] Both panes handle all above operations

## Known Issues
- Arrow keys not triggering redraw? (Added debug output)

## Test Results
Date: [Fill in]
Version: [git hash]
Results: [Pass/Fail each item]