#!/bin/bash
# Build script for utf8proj playground
# Builds the WASM module and prepares the playground for deployment

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
WASM_CRATE="$PROJECT_ROOT/crates/utf8proj-wasm"
PLAYGROUND_DIR="$SCRIPT_DIR"

echo "=== Building utf8proj WASM Playground ==="

# Check for wasm-pack
if ! command -v wasm-pack &> /dev/null; then
    echo "Error: wasm-pack not found. Install it with:"
    echo "  cargo install wasm-pack"
    exit 1
fi

# Build WASM with wasm-pack
echo ""
echo "Building WASM module..."
cd "$WASM_CRATE"
wasm-pack build --target web --out-dir "$PLAYGROUND_DIR/pkg"

echo ""
echo "Build complete!"
echo ""
echo "To serve the playground locally:"
echo "  cd $PLAYGROUND_DIR"
echo "  python3 -m http.server 8080"
echo ""
echo "Then open: http://localhost:8080"
