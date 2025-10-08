#!/bin/bash
# Setup 2017 Noto Color Emoji (blob-style) in kitty terminal

# Step 1: Download the 2017 version of Noto Color Emoji
echo "Downloading 2017 Noto Color Emoji font..."
wget https://github.com/googlefonts/noto-emoji/raw/914c9ecb/fonts/NotoColorEmoji.ttf -O /tmp/NotoColorEmoji.ttf

# Step 2: Install the font (detecting OS)
echo "Installing font..."
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    cp /tmp/NotoColorEmoji.ttf ~/Library/Fonts/
    echo "Font installed to ~/Library/Fonts/"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Linux
    mkdir -p ~/.local/share/fonts
    cp /tmp/NotoColorEmoji.ttf ~/.local/share/fonts/
    fc-cache -fv
    echo "Font installed to ~/.local/share/fonts/"
fi

# Step 3: Configure kitty
echo "Configuring kitty..."
KITTY_CONFIG_DIR="$HOME/.config/kitty"
mkdir -p "$KITTY_CONFIG_DIR"

# Backup existing config if it exists
if [ -f "$KITTY_CONFIG_DIR/kitty.conf" ]; then
    cp "$KITTY_CONFIG_DIR/kitty.conf" "$KITTY_CONFIG_DIR/kitty.conf.backup"
    echo "Backed up existing config to kitty.conf.backup"
fi

# Add emoji font configuration to kitty.conf
cat >> "$KITTY_CONFIG_DIR/kitty.conf" << 'EOF'

# Noto Color Emoji 2017 (blob-style) configuration
# This maps emoji Unicode ranges to specifically use Noto Color Emoji font
symbol_map U+1F300-U+1F6FF Noto Color Emoji
symbol_map U+1F900-U+1F9FF Noto Color Emoji
symbol_map U+2600-U+27BF Noto Color Emoji
symbol_map U+1F680-U+1F6FF Noto Color Emoji
symbol_map U+1F1E0-U+1F1FF Noto Color Emoji
symbol_map U+1F000-U+1FAFF Noto Color Emoji
EOF

# Step 4: Test the installation
echo ""
echo "Installation complete! Restart kitty and test with:"
echo "echo '🐹 🎉 🌟 🚀 🍕'"
echo ""
echo "If emojis don't appear correct:"
echo "1. Run: kitty --debug-font-fallback"
echo "2. Check font list: kitty +list-fonts --psnames | grep -i noto"
echo "3. You may need to remove newer emoji fonts that take precedence"
