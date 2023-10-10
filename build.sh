#!/bin/bash
cd glyphers_ffi
cargo build --release
if [ $? -ne 0 ]; then
    echo "Error: 'cargo build --release' failed."
    exit 1
fi
mv target/release/libglyphers.so ../sdfui/glue/libglyphers.so
if [ $? -ne 0 ]; then
    echo "Error: Failed to move libglyphers.so."
    exit 1
fi
echo "Successfully moved libglyphers.so to ../sdfui/glue/libglyphers.so"