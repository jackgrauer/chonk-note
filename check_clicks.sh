#!/bin/bash
# Check if clicks are landing in the right place

echo "=== Click Position Checker ==="
echo ""

# Clear old log
rm -f /tmp/clicks.log

echo "1. Run the normal app (no debug mode):"
echo "   ./target/release/chonker7 notes"
echo ""
echo "2. Click around 5-10 times in different places"
echo ""
echo "3. Press Ctrl+C to exit"
echo ""
echo "4. Run this script again to see results"
echo ""

if [ -f /tmp/clicks.log ]; then
    echo "Recent clicks:"
    tail -10 /tmp/clicks.log
    echo ""

    # Count mismatches
    TOTAL=$(grep -c "Click" /tmp/clicks.log)

    # Check for obvious mismatches (this is simplified)
    echo "Quick check:"
    grep "Click" /tmp/clicks.log | while read line; do
        # Extract coordinates
        click_coords=$(echo $line | grep -o 'Click([0-9]*,[0-9]*)' | grep -o '[0-9]*,[0-9]*')
        cursor_coords=$(echo $line | grep -o 'Cursor([0-9]*,[0-9]*)' | grep -o '[0-9]*,[0-9]*')

        if [ "$click_coords" != "$cursor_coords" ]; then
            echo "  ❌ Mismatch: $line"
        fi
    done

    echo ""
    echo "Total clicks logged: $TOTAL"
else
    echo "No clicks logged yet."
fi