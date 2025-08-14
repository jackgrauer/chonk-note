#!/bin/bash

echo "🐹 Installing Chonker7..."

# Build the release version
echo "📦 Building chonker7..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "❌ Build failed!"
    exit 1
fi

# Create local bin directory if it doesn't exist
mkdir -p ~/.local/bin

# Get the absolute path to the chonker7 directory
CHONKER7_DIR="$(cd "$(dirname "$0")" && pwd)"

# Create a wrapper script instead of copying the binary
echo "📁 Installing to ~/.local/bin..."
cat > ~/.local/bin/chonker7 << EOF
#!/bin/bash
export DYLD_LIBRARY_PATH="$CHONKER7_DIR/lib:\$DYLD_LIBRARY_PATH"
exec "$CHONKER7_DIR/target/release/chonker7" "\$@"
EOF

# Make it executable
chmod +x ~/.local/bin/chonker7

# Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo ""
    echo "⚠️  ~/.local/bin is not in your PATH"
    echo ""
    echo "Add this to your shell config file (.bashrc, .zshrc, etc):"
    echo '  export PATH="$HOME/.local/bin:$PATH"'
    echo ""
    echo "Then reload your shell or run:"
    echo "  source ~/.bashrc  # or ~/.zshrc"
else
    echo "✅ Installation complete!"
fi

echo ""
echo "Usage:"
echo "  chonker7 <pdf-file>           # View PDF with AI text extraction"
echo "  chonker7 <pdf-file> -p 3      # Start at page 3"
echo "  chonker7 <pdf-file> -m text   # Text-only mode"
echo "  chonker7 <pdf-file> -m split  # Split view (default)"
echo ""
echo "Controls:"
echo "  n/→     Next page"
echo "  p/←     Previous page"
echo "  m       Toggle display mode"
echo "  Ctrl+E  Re-extract current page"
echo "  q       Quit"