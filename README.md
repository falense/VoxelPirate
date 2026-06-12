# VoxelPirates

A voxel pirate game built with Rust + Bevy. The world is open sea: every ship
is its own little Minecraft — a voxel grid that sails, floats, takes damage
block by block, and can be rebuilt block by block. Start on a barge, scavenge
the wrecks of your enemies, and grow into a galleon before the Dreadnought
comes for you.

## How to play

```sh
cargo run --release
```

| Input | Action |
|---|---|
| `W A S D` | Sail: thrust, turn (turning needs way on the ship) |
| Left click | Fire the broadside facing the cursor |
| Scroll | Zoom the chase camera |
| `Tab` | Toggle build mode |
| `1`-`7` (build) | Select block; click places, right-click removes |
| `Q` / `E` | Keyboard broadsides (fallback) |
| `P` | Pause |
| `R` | Set sail again after going down |

## The loop

- Sink ships; their blocks bob up as **flotsam**. Sail over it: it repairs
  your hull first, then banks as **salvage** (a scavenged cannon is worth 8,
  gold plunder 5).
- Salvage auto-buys your next hull: Barge → Brig → Frigate → Galleon.
- In build mode you spend salvage to reshape your ship — more hull is more
  durability, more cannons is more broadside.
- Mind the **wind** (intel line): running with it is a quarter faster.
- Derelict wrecks are risk-free salvage. Ramming works, and hurts you both.
- At 15 kills the **Dreadnought** is summoned. Sink it and the seas are
  yours; the hunt continues for as long as you can stay afloat.

## Dev flags

- `--selftest` — scripted smoke test: drives input resources, asserts the
  salvage economy in logs, saves screenshots to `/tmp/selftest_*.png`
- `--demo` — the player ship fights on autopilot, for pacing observation
- `--boss` — start at 15 kills in a frigate, next to the boss fight

## Architecture

See `AGENTS.md` (a.k.a. `CLAUDE.md`) for the architecture rules. The short
version: all block properties live in the registry (`src/blocks.rs`), ships
are entities with a ship-local voxel grid (`src/ship.rs`), and gameplay
systems never match on block ids.
