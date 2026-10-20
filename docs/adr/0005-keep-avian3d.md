# Keep avian3d as the physics engine

The cube world stays on avian3d and turns on its realism (gravity, friction, restitution, density, damping) instead of moving to another engine. avian is ECS-native: bodies, colliders and material settings are ordinary Bevy components, so a Material preset just inserts `ColliderDensity`, `Friction` and `Restitution`. It already has the primitive colliders Shapes need (cuboid, cylinder, convex hull), the spatial queries the tests use, and the Bevy version this app is on.

## Considered options

- bevy_rapier3d: mature and fast, but its state lives in a separate Rapier world synced to Bevy, which is more to keep consistent when shapes are rebuilt from parameters every change.
- Hand-written physics: only the stilled, floating world needs nothing more. Gravity, bounce, friction and stacking do.

## Consequences

- Without gravity, shapes nobody holds are stilled every step (the floating world). With gravity on, they fall, bounce and slide freely.
- Electricity's contact graph (later) can read avian's collision events directly.
