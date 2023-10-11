#!/bin/bash
cargo build --release --package glyphers_ffi
if [ $? -ne 0 ]; then
    echo "Error: 'cargo build --release' failed."
    exit 1
fi
cp target/release/libglyphers.so sdfui/glue/libglyphers.so
if [ $? -ne 0 ]; then
    echo "Error: Failed to move libglyphers.so."
    exit 1
fi
echo "Successfully moved libglyphers.so to ../sdfui/glue/libglyphers.so"