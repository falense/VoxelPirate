# Spec 002 — Elaborate ship designs

**Status:** accepted (2026-06-14)
**Roadmap:** polish for items 1/3/5 (ship variety & feel); flags greedy
meshing (roadmap follow-up) as the perf mitigation.

## Summary

Replace the flat slab hulls (a `length×width` deck-on-hull rectangle with
vertical masts and flat sails) with proper tall-ship silhouettes built by a
single parametric builder. Every existing class keeps its public
`fn() -> HashMap<IVec3, BlockId>` signature, so `enemy.rs`, `salvage.rs`, and
the player upgrade ladder are unaffected — only the geometry changes, and the
voxel count per ship rises several-fold.

## Anatomy the builder produces

From a `ShipSpec` (length, beam, hull material, hull height, bow/stern taper
lengths, mast list, guns-per-side, castles flag):

1. **Tapered solid hull** — width narrows to a point at the bow (a cutwater)
   and fills out to a broad transom at the stern, instead of a rectangle.
   Hull is `hull_height` solid layers (waterline and below).
2. **Weather deck** — `OakDeck` caps the hull at deck level.
3. **Bulwarks** — a raised rail (hull block) one level above the deck down both
   sides and across the stern transom; the bow tip stays open.
4. **Gun ports** — cannons spaced evenly along the rail amidships on both
   sides (replacing rail blocks), `guns_per_side` each side.
5. **Castles** — a raised, walled **quarterdeck** aft and a short
   **forecastle** forward, giving the stepped fore/aft profile of an age-of-
   sail warship.
6. **Masts + rig** — each mast is a pole from the deck up, carrying two
   stacked square sails on horizontal yards, topped with a crimson **pennant**.
7. **Bowsprit + jib** — a spar angling up and forward off the bow with a small
   headsail.

## New block

- `BlockId::Flag` ("Pennant") — crimson, near-weightless, decorative (`gun:
  false`, low cost). Registry-only addition (`blocks.rs`), per the architecture
  rule; materials and salvage handle it automatically. Adds a colour accent
  and pirate identity at the mastheads.

## Per-class specs (relative scale; tune by eye)

| Class       | length | beam | hull | hull_h | masts | guns/side | castles |
|-------------|--------|------|------|--------|-------|-----------|---------|
| Sloop       | 11     | 5    | oak  | 2      | 1     | 2         | no      |
| Barge (T0)  | 12     | 5    | oak  | 2      | 1     | 2         | no      |
| Brig (T1)   | 15     | 7    | oak  | 2      | 2     | 3         | yes     |
| Frigate(T2) | 19     | 7    | iron | 2      | 3     | 4         | yes     |
| Galleon(T3) | 23     | 9    | iron | 2      | 3     | 5         | yes     |
| Dreadnought | 29     | 11   | iron | 3      | 4     | 7         | yes     |

## Trade-offs / impacts (acknowledged)

- **Performance.** Each cube is still its own entity (no greedy meshing yet).
  Hulls are *solid*, so a galleon is ~700–900 blocks and the dreadnought
  ~1.5k; with a fleet of four plus the player and boss this is several
  thousand cube entities. On a dGPU this should hold, but it is the main risk.
  **Mitigation / follow-up: greedy meshing** (already on the roadmap) — merge
  each ship's static blocks into one mesh, regenerated on damage. Out of scope
  here. If it stutters before then, drop `hull_height` to 1 or thin the fleet.
- **Toughness.** Sinking is a fixed *fraction* (`SINK_LOSS_FRACTION = 0.35`) of
  designed blocks, so bigger ships now soak more hits before going down. This
  reads as "bigger ship = beefier," which is desirable; combat pacing was not
  retuned. Revisit blast radius / sink fraction if fights drag.
- **Solid vs. hollow hull.** Solid was chosen for bulk (HP, debris) and
  simplicity, matching the request to raise voxel count; the buried interior
  cubes are never visible. Hollowing would help perf for an identical look —
  noted as a lever if perf bites.

## Code changes

- `src/blocks.rs` — add `Flag` to the enum, `ALL`, and `def`.
- `src/build.rs` — add `Digit8` so the new block is selectable in build mode.
- `src/ship.rs` — remove `hull_layout`; add `ShipSpec`, `half_width`, the
  `build` function and its helpers; rewrite the six `*_layout` functions to
  return `build(&SPEC)`.

## Verification

- `cargo check` / `clippy` / `fmt` clean.
- `--selftest` still runs (it scraps/places a block and fires; geometry change
  must not break the deck-point or sea-point picks).
- Manual: ships read as ships (pointed bow, raised stern, layered sails);
  cannons still bear and fire from the rails; watch the frame rate with a full
  fleet up.
