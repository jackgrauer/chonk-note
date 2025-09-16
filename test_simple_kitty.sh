#!/bin/bash
echo "=== Kitty Graphics Protocol Test ==="
echo "Environment:"
echo "TERM: $TERM"
echo "TERM_PROGRAM: $TERM_PROGRAM" 
echo "KITTY_WINDOW_ID: $KITTY_WINDOW_ID"
echo ""

if [ -n "$KITTY_WINDOW_ID" ]; then
    echo "✅ Kitty detected (KITTY_WINDOW_ID=$KITTY_WINDOW_ID)"
    
    # Test basic Kitty graphics support with a simple query
    echo "Testing graphics protocol support..."
    printf '\e_Ga=q\e\\'
    
    echo ""
    echo "If you see graphics protocol response above, Kitty graphics are working."
else
    echo "❌ Not running in Kitty terminal"
fi