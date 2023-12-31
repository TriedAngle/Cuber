# cuber
Voxel Game (engine)

# sdfui
2D graphics library based on Signed Distance Fields focused on UI usage.

# pressa
platform/windowing agnostic input library

# glfw
higher level glfw abstraction, might get merged into factor/extra/

# glyphers & glyphers_ffi
text-rasterization library with c-ffi based on fontdue

# liverking
roleplay library to make (unsafe) rust sane to use

# samples
testing ground, samples and demos for libraries or internal functionality

# Building
## requirements
1. newest `factor` installation
2. newest `rust` installation with `nightly` toolchain active
3. newest `glfw` and `opengl` (4.6) (Windows: having `glfw.dll` in `factor/` works)
4. add this directory to your `.factor-roots` (run: `scaffold-factor-rc` within factor and add this path) 

## automatically
TODO!
## manually
1. run `cargo build --release` in this directory
2. move `target/release/glyphers.dll` or `libglyphers.so` into your `factor` directory
3. type `"cuber" run` within factor, or any of the demos or samples ex: `"samples.glfw" run`

# Status
## Cuber
Done: initial setup
next goals are rendering 3d grids via only DDA and basic world-gen

## SDFUI
Done: primitive shapes and basic text rendering
- [ ] more primitives
- [ ] caching (only text rasterization is cached right now)
- [ ] tree-hirarchy
- [ ] input
- [ ] rasterized text to sdf conversion

## Pressa
Done: basically done, only minor changes and predefined interop missing
- [ ] glfw mouse support and other events
- [ ] factor ui interop
- [ ] cleaning up api and speeding up internals

## Glyphers
Done: supports very basic multi-font text rasterization
- [ ] text-alignment
- [ ] multiline text with max width and height
- [ ] font sizes (I just realized I don't actually do this yet lol)

## GLFW
Done: functionally done, only higher level improvements needed
- [ ] improve api (missing events for higher level)
- [ ] make multiple window support and "main-thread" loops nice to use
