#!/bin/bash

echo "🐹 Testing Chonker7..."
echo ""

# Check if chonker7 is installed
if ! command -v chonker7 &> /dev/null; then
    echo "❌ chonker7 not found in PATH"
    echo "   Run ./install.sh first"
    exit 1
fi

echo "✅ chonker7 is installed at: $(which chonker7)"
echo ""

# Check for test PDF
TEST_PDF="test.pdf"
if [ ! -f "$TEST_PDF" ]; then
    echo "ℹ️  No test.pdf found."
    echo ""
    echo "Launching chonker7 with file dialog..."
    echo ""
    chonker7
else
    echo "📄 Found test.pdf - launching chonker7..."
    echo ""
    chonker7 "$TEST_PDF"
fi