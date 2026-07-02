# Spec 006 — The dock interlude and wave battles

**Status:** accepted (2026-07-02); shape chosen by Sondre (waves + hull
templates + dedicated dock scene).

## The loop

Dock → set sail → wave n → cleared (or sunk) → dock → wave n+1 (or retry).

- **`GamePhase`** (Bevy `States`): `Dock` | `Battle`. Combat, sailing, AI,
  and the chase camera are gated to `Battle`; the dock gets an orbit
  camera, forced build mode, and its own key handler. Note: the initial
  `OnEnter(Dock)` fires *before* `PreStartup`, so `enter_dock` owns
  spawning the first player ship (a `Startup` spawner would duplicate it).
- **Waves** (`enemy::wave_composition`): waves 1–7 are a hand-tuned ramp
  (2 sloops → frigate pairs); wave 8 is the Dreadnought with an escort
  (victory condition, unchanged); past 8 a growing strength budget
  composes fleets of up to 5, with another Dreadnought every 8th wave.
  The whole wave spawns at once on golden-angle bearings. The old
  `FleetDirector` drip-feed, kill-count unlocks, and `BOSS_AT_KILLS` are
  gone; `--boss` now jumps to wave 8.
- **Cleared**: no live hostiles → remaining flotsam is swept aboard as
  salvage (no post-battle trawling), wave counter advances, back to dock.
- **Sunk**: ship gone → towed home, lose a third of banked salvage, retry
  the *same* wave. Ship design is never lost (a fresh hull of the same
  class launches if yours is on the bottom); the sting is economic. The
  old R-to-respawn system is gone.

## The dock

A sheltered cove at the world origin: a lantern-lit pier built from
ordinary registry blocks as a static voxel grid (the ship mesher renders
scenery for free), flat-calm water, and a free orbit camera (WASD orbits/
tilts, scroll zooms — sailing keys are free at the dock). Build mode is
the default interaction; Tab is battle-only.

**Sea state** (`dock::SeaState`): a single factor over the swell table,
eased over ~2 s on phase changes. The GPU ocean reads it via the material
uniforms (`OceanExtension::set_amplitude_scale`), CPU buoyancy/flotsam/
splash sampling multiplies by the same factor — one source of truth, so
ships sit flat in the cove and the swell rises as you sail out.

**Dock keys**: `Enter` sets sail. `R` repairs the plan lowest-block-first
while salvage lasts. `U` buys the next hull class (`UPGRADE_COSTS`
unchanged); the value of blocks *beyond* the stock plan of your current
class is refunded, so custom fittings carry their worth into the new
hull. The mid-sail auto-upgrade is deleted — hull swaps are an explicit
dock decision now.

## Harnesses

- `--selftest` drives the full loop: build at the dock, set sail, fire
  wave 1's first broadside.
- `--demo` gets a dock brain that presses the real dock keys (repair,
  buy, sail) on a timer, so the autopilot plays the actual loop; verified
  clearing waves 1→6 in one 5-minute run, and losing/retrying wave 8
  repeatedly under `--boss`.
- `--selftest`/`--demo`/`--mute` zero the global volume so harness runs
  don't blare cannon fire at whoever is at the machine.

## Known gaps (deliberate, small)

- The pier has no collision; a battle that drifts into the cove can pass
  through it. Battles spawn 55 m+ out, so it's cosmetic for now.
- Derelicts only appear during battles, and leftovers despawn at the dock.
- Wave numbering is the only between-run difficulty knob; no meta-economy
  beyond banked salvage yet.
