# Cuber

voxel based game

## Techniques
- brickmap of 8^3 volumes with empty bricks storing an SDF
- palette compression of these 8^3 volumes to take the next 2^n bits necessary for indexing

## Working priorities
hopefully getting most of this done in december.

- building & destruction
- brickmap of 64^3 octrees
- more interesting worldgen
- material variations or material groups
- single light source + shadows
- dynamic chunk and LOD loading/unloading 
- local lighting + shadows (non GI for now)
- simple bounding box collions
- loading + saving