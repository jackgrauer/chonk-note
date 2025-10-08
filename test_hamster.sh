#!/bin/bash
echo "Testing hamster emoji:"
echo "🐹"
echo ""
echo "If you see a blob hamster (round, cute), it worked!"
echo "If you see a realistic hamster, the font isn't loading."
echo ""
echo "Run this to debug:"
echo "kitty --debug-font-fallback 2>&1 | grep -i hamster"
