# VoxelPirates

(Formerly CraftPirate — the repository directory keeps the old name.)

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

## Decision protocol: ask only at the Pareto front

When developing autonomously, use Pareto optimality as the criterion for when
to ask the user for direction:

- **One option dominates** (better on at least one axis — correctness, perf,
  simplicity, extensibility — and worse on none): take it. Do not ask.
- **Options are Pareto-equivalent** (neither is better or worse; the pick is
  purely a matter of preference — game feel, art direction, which mechanic to
  prioritize, naming): ask the user before proceeding down either path.
- **If the user doesn't respond**: park the decision in `DECISIONS.md` with
  the options, their trade-offs, and a recommendation. Then switch to an
  independent branch of work that doesn't depend on the parked choice. When
  the user returns, surface the queue.

Don't manufacture questions for dominated choices, and don't silently pick a
preference the user would have wanted to make.

## Commands

- `cargo check` — fast compile check (use this in the edit loop)
- `cargo clippy` / `cargo fmt` — lint and format before committing
- `cargo run` — opens the game window (needs a display; in the devcontainer
  this requires the X11/GPU passthrough configured in `.devcontainer/`)

## Roadmap (rough)

1. ✅ Vertical slice: voxel barge, ocean plane, WASD sailing, fake wave bob
2. Real buoyancy from per-block mass/volume; ship sinks when overloaded or holed
3. ✅ Block placement/removal at sea (Tab build mode, salvage economy);
   greedy meshing still pending if per-cube entities get slow
4. ✅ Cannons that fire and damage other ships' voxel grids (Q/E broadsides,
   blast damage with debris, sinking, AI sloops in an endless skirmish loop)
5. ✅ Enemy/derelict vessels to scavenge (flotsam economy, decision A);
   material tiers (oak/iron) and a Dreadnought boss with victory condition

Next ideas: structural connectivity (cut-off blocks break away), real
buoyancy (roadmap 2), boarding/stripping disabled ships (decision B),
greedy meshing, gamepad support.
