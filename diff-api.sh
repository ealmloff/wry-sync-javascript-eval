#!/bin/bash

# Generate public API for wasm-bindgen
echo "Generating wasm-bindgen public API..."
(cd wasm-bindgen && cargo public-api --omit blanket-impls,auto-trait-impls,auto-derived-impls > ../wasm-bindgen.api.txt 2>/dev/null)

# Generate public API for wry-bindgen
echo "Generating wry-bindgen public API..."
cargo public-api --omit blanket-impls,auto-trait-impls,auto-derived-impls > wry-bindgen.api.txt 2>/dev/null

# Find APIs in wasm-bindgen but not in wry-bindgen
echo ""
echo "APIs in wasm-bindgen but NOT in wry-bindgen:"
echo "============================================="
comm -23 <(sort wasm-bindgen.api.txt) <(sort wry-bindgen.api.txt)

rm wasm-bindgen.api.txt wry-bindgen.api.txt