#!/bin/bash

echo "Testing PDF loading in chonker7"
echo "================================"
echo ""
echo "This will show debug output to help diagnose any issues."
echo "When the file picker opens, select a PDF with Enter."
echo ""
echo "Starting chonker7..."
echo ""

# Run with debug output visible
DYLD_LIBRARY_PATH=/Users/jack/chonker7/lib /Users/jack/chonker7/target/release/chonker7 2>&1

echo ""
echo "Exit code: $?"
echo ""
echo "If the app exited unexpectedly, check the debug messages above."