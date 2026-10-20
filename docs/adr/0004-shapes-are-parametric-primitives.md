# Shapes are parametric primitives, not editable meshes

A Shape in the cube world is a primitive described by a few parameters (per-axis size, and later roundness and taper), never a mesh you edit. Its render mesh and its collider are rebuilt from those parameters whenever they change, and its `Transform` scale stays 1. Every tool is a parameter write: stretch sets one side's size, Round sets roundness, Taper sets taper. That keeps each edit clean, repeatable and saveable as a few numbers (templates), and it keeps colliders as avian's own primitives, or a convex hull, so a cylinder rolls and a cone tips like the real thing.

## Considered options

- Mesh editing (move vertices, extrude, booleans): far more freedom, but colliders become concave meshes that simulate badly. Every edit is a lossy change to a vertex list, hand input is too coarse to aim at vertices, and templates would store meshes instead of a handful of numbers.
- Scaling the transform for stretch, as before: free, but scale distorts rounded edges and cylinders out of their true form once Round lands.

## Consequences

- Only shapes the parameters can express exist: a box family that rounds to a cylinder and tapers to a frustum, pyramid or cone. Chamfer is the next parameter planned.
- Changing a parameter rebuilds the collider. Today that happens on every change, every frame of a stretch.
