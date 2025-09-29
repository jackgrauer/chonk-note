#!/bin/bash
# One-command diagnosis of what's wrong

echo "=== Chonker7 Quick Diagnosis ==="
echo ""
echo "Building and running diagnostics..."

# Build with validation
export DYLD_LIBRARY_PATH=./lib
cargo build --release 2>&1 | grep -E "error" && echo "❌ Build errors" && exit 1

echo "✅ Build OK"
echo ""
echo "Common issues:"
echo ""

# Check for coordinate issues in the code
echo -n "1. Double coordinate conversion: "
if grep -q "saturating_sub(1).*saturating_sub(1)" src/mouse.rs; then
    echo "❌ FOUND - coordinates subtracted twice"
else
    echo "✅ OK"
fi

echo -n "2. Viewport offset missing: "
if ! grep -q "viewport_[xy]" src/mouse.rs; then
    echo "❌ MISSING - clicks won't account for scrolling"
else
    echo "✅ OK"
fi

echo -n "3. Conflicting input handlers: "
if grep -q "Ok(false).*// Fall through" src/dual_pane_keyboard.rs; then
    echo "✅ OK"
else
    echo "⚠️  May have dual handler conflicts"
fi

echo -n "4. Grid/rope sync: "
if grep -q "notes_grid.*VirtualGrid::new" src/mouse.rs; then
    echo "✅ OK"
else
    echo "⚠️  Grid might not sync with rope"
fi

echo ""
echo "To see coordinates live, run: ./debug_run.sh"
echo "To watch the title bar update with coordinates."
echo ""
echo "The debug panel in top-right shows:"
echo "  🎯 Mouse position"
echo "  📍 Cursor position"
echo "  📜 Viewport offset"
echo "  🔲 Active pane"