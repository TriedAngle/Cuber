# SDFUI
## Warning: this library is still very early into development
A ui library built on top of signed distance fields.

The SDFs are implemented using multithreading and AVX2 instructions

Supports cross-platform as long as AVX2 instructions are supported by the platform.
for Aarch64 support is planned.

Performance TODO: currently the render sizes of all SDFs are fixed, 
if this can be individualized and taken to the minimum size, the performance can be increased exponentially.

TODO: bindings for C