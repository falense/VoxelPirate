# CraftPirate

A voxel-based pirate game: Minecraft-style block building, but the world is the
open sea. You start with a small barge and grow it into a warship by scavenging
blocks (hull tiers, cannons, sails) from vessels you encounter. Ships are the
core entity: each ship is its own voxel grid that moves, rotates, and floats as
a rigid body — unlike Minecraft's static world grid.

## Stack

- **Rust + Bevy** (version pinned in `Cargo.toml` — do not bump without asking)
- ECS architecture: ships are entities, blocks contribute components/properties

## Architecture rules

- **Block registry** (`src/blocks.rs`): every block type is defined once with
  its properties (mass, color, later: buoyancy, hardness, tier, behavior).
  Gameplay systems read properties via `blocks::def()` — never match on
  `BlockId` in gameplay code. Adding a block must stay a registry-only change.
- **Ship-local coordinates** (`src/ship.rs`): a ship's voxels live in its own
  integer grid (`ShipVoxels`); the ship entity's `Transform` places the whole
  grid in the world. Block (0,0,0) is aft-port-bottom; grid y = 0 is the
  waterline layer.
- The waterline is world y = 0 (`src/ocean.rs`).

## Commands

- `cargo check` — fast compile check (use this in the edit loop)
- `cargo clippy` / `cargo fmt` — lint and format before committing
- `cargo run` — opens the game window (needs a display; in the devcontainer
  this requires the X11/GPU passthrough configured in `.devcontainer/`)

## Roadmap (rough)

1. ✅ Vertical slice: voxel barge, ocean plane, WASD sailing, fake wave bob
2. Real buoyancy from per-block mass/volume; ship sinks when overloaded or holed
3. Block placement/removal at sea; greedy meshing once per-cube entities get slow
4. Cannons that fire and damage other ships' voxel grids
5. Enemy/derelict vessels to scavenge; material tiers
