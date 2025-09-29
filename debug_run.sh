#!/bin/bash
# Debug runner for Chonker7 with all debugging features enabled

echo "=== Chonker7 Debug Mode ==="
echo ""
echo "Debug features enabled:"
echo "  ✓ Title bar coordinate display"
echo "  ✓ File logging to /tmp/chonker7.log"
echo "  ✓ Coordinate transformation tracking"
echo ""
echo "Optional environment variables:"
echo "  CHONKER_DEBUG_VERBOSE=1  - Show status line at bottom"
echo "  CHONKER_LOG_FILE=/path   - Custom log file location"
echo ""
echo "Starting in 2 seconds..."
echo "Tail the log in another terminal: tail -f /tmp/chonker7.log"
echo ""
sleep 2

# Enable all debugging features
export CHONKER_DEBUG=1
export CHONKER_LOG=1
export DYLD_LIBRARY_PATH=./lib

# Run in notes mode by default for testing, or use provided argument
if [ $# -eq 0 ]; then
    echo "Starting in Notes mode (use './debug_run.sh file.pdf' for PDF mode)"
    ./target/release/chonker7 notes
else
    ./target/release/chonker7 "$@"
fi