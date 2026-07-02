# Spec 003 — Weapon variety and ship detail

**Status:** accepted (2026-07-02)
**Roadmap:** deepens item 4 (combat) and the ship-feel polish of spec 002.

## Summary

Two threads, both registry-driven:

1. **Gun types.** `BlockDef.gun` grows from a bool into `Option<GunDef>`
   (muzzle speed, blast radius, pierce depth, ball scale). Every gun block
   fires with the broadside as before; the shot's character comes entirely
   from its `GunDef`, so new weapons stay a registry-only change.
2. **Hull detail.** The spec-002 builder gains a wale stripe, stern gallery,
   figurehead, deck hatches, taller layered rigs, a spanker, and a fuller
   jib — a modest voxel-count rise, tuned to hold the current frame rate.

## The armory

| Block     | speed | blast | pierce | character                              |
|-----------|-------|-------|--------|----------------------------------------|
| Cannon    | 22    | 1.6   | 0      | the all-rounder (unchanged)            |
| Culverin  | 30    | 0.6   | 4      | flat and fast; drills a line of blocks clean through the hull |
| Carronade | 15    | 2.8   | 0      | short-ranged smasher; blows a crater   |

- Piercing shots (`apply_pierce`) walk the flight line from the entry point
  and destroy up to `pierce` further blocks, coasting a few cells across
  gaps — a culverin ball can punch in one side and out the other.
- Slow guns arc shorter with the shared muzzle loft, so range differences
  fall out of the ballistics with no extra range stat.
- Ships mount mixed batteries per class (`ShipSpec.battery`, aft to fore,
  mirrored on both rails): brigs add a carronade, frigates culverin chasers,
  the galleon and dreadnought carry all three. AI inherits the mix for free.
- Build mode: digits 1-0 select the first ten registry blocks, putting the
  culverin (9) and carronade (0) in players' hands.

## New blocks

- `Culverin` — bronze, pierce-4 gun, cost 12.
- `Carronade` — rust-red, blast-2.8 gun, cost 14.
- `Trim` ("Tarred Trim") — near-black wood for wales and hatch gratings.
- `Lantern` ("Stern Lantern") — warm amber for stern galleries.

## Hull detail (builder additions)

- **Wale**: a tarred `Trim` band along the top hull strake and transom.
- **Stern gallery** (castled ships): alternating `Lantern` windows across the
  sterncastle bulkhead, a great lantern above, a `Gold` figurehead at the
  stem — which also makes fancy wrecks drop slightly richer flotsam.
- **Cargo hatch**: two `Trim` grating cells amidships, skipping mast steps.
- **Rig**: masts of height ≥ 9 carry three square sails (narrowing aloft)
  instead of two; castled ships fly a fore-aft spanker off the aft mast;
  the bowsprit carries a fuller jib.

## Fixes riding along

- Ball/ship collision quick-reject used a fixed 15 m radius, smaller than
  the dreadnought's actual footprint (~17 m) — shots at its bow and stern
  tips passed straight through. Now uses each ship's own `voxels.radius`.

## Verification

- Barge `--selftest` passes exactly (scrap 1 → place 0). The `--boss`
  variant scraps a sail (cost 2) because the frigate's aft rig crosses the
  fixed aim point; known, cosmetic, dev-flag-only.
- `--demo --boss --diag`: no panics, ~46-50 fps — unchanged from before the
  detail pass.
