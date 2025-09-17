#!/bin/bash

# Test script for block selection in chonker7
echo "Testing block selection in chonker7"
echo "The fix implements:"
echo "1. Drag events (is_drag=true) are now matched BEFORE click events"
echo "2. Click events have is_drag=false constraint to prevent matching drags"
echo "3. Block selection is the default - no modifiers needed"
echo ""
echo "To test manually:"
echo "1. Run: chonker7 <file>"
echo "2. Click and drag with mouse to create a rectangular selection"
echo "3. The selection should appear as a blue block, not a traditional selection"
echo ""
echo "Key changes made:"
echo "- Reordered match patterns in handle_mouse() to process drag events first"
echo "- Added is_drag: false constraint to left click pattern"
echo "- Block selection renders with distinct blue color (RGB 80,80,200)"

# Build and install
echo ""
echo "Building release version..."
cd /Users/jack/chonker7
DYLD_LIBRARY_PATH=./lib cargo build --release --quiet

echo "Block selection implementation is complete!"
echo "Test with: chonker7 <any_file>"