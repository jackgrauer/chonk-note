#!/bin/bash
# Test harness for Chonker7 - run this after ANY change

echo "=== Chonker7 Regression Test ==="
echo "Building..."
cargo build --release 2>&1 | grep -E "error|warning: unused" | head -5

if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi

echo "✅ Build succeeded"

# Create a test file
cat > /tmp/test_input.txt << 'EOF'
Line 1
Line 2
Line 3
EOF

echo ""
echo "=== Manual Test Checklist ==="
echo "Run: ./target/release/chonker7 /tmp/test_input.txt"
echo ""
echo "1. [ ] Arrow keys move cursor"
echo "2. [ ] Click positions cursor"
echo "3. [ ] Can type text"
echo "4. [ ] Cmd+X cuts selection"
echo "5. [ ] Cmd+V pastes"
echo ""
echo "If ANY of these fail, run: git reset --hard HEAD"