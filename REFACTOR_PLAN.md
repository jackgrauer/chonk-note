# Chonker7 Refactoring Plan

## Problem
Coordinate conversions and state management are scattered across multiple files, making debugging nearly impossible.

## Solution: Unified Architecture

### 1. Single State Store
```rust
struct UnifiedState {
    cursor: CursorState,
    viewport: ViewportState,
    selection: SelectionState,
    document: DocumentState,
}
```
All state changes go through this, making it easy to log/debug.

### 2. Event System
```rust
enum Event {
    MouseClick { screen_x: u16, screen_y: u16 },
    KeyPress(KeyEvent),
    Scroll { dx: i32, dy: i32 },
}

fn handle_event(state: &mut UnifiedState, event: Event) -> Action {
    // ALL event handling in one place
}
```

### 3. Render Pipeline
```rust
fn render(state: &UnifiedState) -> TerminalBuffer {
    // State → Display, never the other way around
}
```

## Migration Strategy

### Phase 1: Add Without Breaking (1 day)
- Add CoordinateSystem alongside existing code
- Add debug overlay to see both systems
- Add tests to verify they match

### Phase 2: Gradual Migration (3-5 days)
- Replace one coordinate conversion at a time
- Run tests after each change
- Keep old code commented until stable

### Phase 3: Cleanup (1 day)
- Remove old coordinate code
- Remove debug overlays
- Document the new system

## Quick Wins Right Now

1. **Add logging that doesn't break the UI**:
```bash
# In one terminal:
tail -f /tmp/chonker7.log

# In another:
./target/release/chonker7 file.txt
```

2. **Create a coordinate debug mode**:
```rust
if env::var("CHONKER_DEBUG_COORDS").is_ok() {
    eprintln!("Click: ({},{}) → Pane: {:?} → Doc: ({},{})",
             x, y, pane, doc_x, doc_y);
}
```

3. **Make a test file that exercises all edge cases**:
```
Line with 80 characters to test horizontal scrolling....................end
Short line
Empty line above

Virtual space test - click way out here →                                    X
```

## Why This Will Work

1. **Single source of truth**: No more scattered coordinate math
2. **Testable**: Can unit test conversions without running the full app
3. **Debuggable**: Can log every transformation in one place
4. **Maintainable**: New contributors can understand the flow

## Escape Route

If refactoring feels too big, at minimum:
1. Add the debug overlay (5 minutes)
2. Add coordinate logging (10 minutes)
3. Write tests for current behavior (30 minutes)

This alone will make debugging 10x easier.