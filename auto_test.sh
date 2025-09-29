#!/bin/bash
# Automatic testing that tells you what's broken

echo "=== Chonker7 Automatic Test Suite ==="
echo ""
echo "This will test coordinate handling and report issues..."
echo ""

# Create test script that simulates user actions
cat > /tmp/test_chonker.expect << 'EOF'
#!/usr/bin/expect -f

set timeout 5
spawn ./target/release/chonker7 notes

# Wait for app to start
sleep 1

# Test 1: Click at position (10, 5)
send "\x1b[<0;10;5M"
sleep 0.5

# Test 2: Type some text
send "Test"
sleep 0.5

# Test 3: Arrow key movement
send "\x1b[D"  ;# Left arrow
send "\x1b[C"  ;# Right arrow
send "\x1b[A"  ;# Up arrow
send "\x1b[B"  ;# Down arrow
sleep 0.5

# Test 4: Scroll then click
send "\x1b[<64;20;10m"  ;# Scroll down
sleep 0.5
send "\x1b[<0;15;8M"    ;# Click after scroll
sleep 0.5

# Test 5: Block selection
send "\x1b[<0;5;5M"      ;# Click start
send "\x1b[<32;15;10m"   ;# Drag to end
sleep 0.5

# Exit
send "\x03"  ;# Ctrl+C

expect eof
EOF

chmod +x /tmp/test_chonker.expect

echo "Running automated tests..."
echo ""

# Run with debugging enabled and capture log
CHONKER_DEBUG=1 CHONKER_LOG=1 DYLD_LIBRARY_PATH=./lib /tmp/test_chonker.expect 2>/dev/null

echo ""
echo "=== Test Results ==="
echo ""

# Analyze the log for issues
if [ -f /tmp/chonker7.log ]; then
    echo "Checking for coordinate mismatches..."

    # Look for patterns that indicate problems
    grep -E "Click.*Doc\(" /tmp/chonker7.log | tail -5

    echo ""
    echo "Summary:"
    echo "- Total clicks logged: $(grep -c "Click:" /tmp/chonker7.log)"
    echo "- Viewport events: $(grep -c "Viewport" /tmp/chonker7.log)"

    # Check for specific issues
    if grep -q "Doc(0,0)" /tmp/chonker7.log; then
        echo "⚠️  WARNING: Clicks registering at (0,0) - coordinate calculation issue"
    fi

    if ! grep -q "viewport_[xy]" /tmp/chonker7.log; then
        echo "⚠️  WARNING: No viewport tracking - scrolling won't work properly"
    fi
else
    echo "❌ No debug log found - debugging not working"
fi

echo ""
echo "To see full log: cat /tmp/chonker7.log"
echo "To run manual test: ./debug_run.sh"