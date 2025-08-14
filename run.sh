#!/bin/bash

# Run chonker7 with the correct library path
export DYLD_LIBRARY_PATH="./lib:$DYLD_LIBRARY_PATH"

if [ "$1" = "--build" ] || [ "$1" = "-b" ]; then
    echo "Building chonker7..."
    cargo build --release
    echo "Build complete!"
fi

echo "Running chonker7..."
./target/release/chonker7 "$@"