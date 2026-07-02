# Spec 004 — Living sea and light

**Status:** accepted (2026-07-02)
**Roadmap:** graphics pass; steps toward item 2 (the swell is now a real
surface ships read their pose from).

## Summary

Three graphics upgrades chosen from the "too basic" menu (ocean, sky/light,
materials — wakes/combat feedback parked, see DECISIONS.md):

1. **A living ocean.** The static sea plane becomes a player-following,
   120×120-cell mesh displaced every frame by three world-anchored swell
   trains (`ocean::SWELLS`: a long primary, a cross-swell, and chop).
   `wave_sample` returns height and analytic gradient in one sin/cos pair
   per swell; the mesh takes vertex heights and normals from it, so sun
   glints roll with the sea. Everything floats on the *same* surface:
   - ships take height at the hull and pitch/roll from the slope sampled
     bow-to-stern and beam-to-beam (softened, so hulls read massive);
   - flotsam bobs on it; cannonball splashes land on it.
   A 20 km flat skirt slightly below the trough line carries the water to
   the true horizon — no more distance-fog trick (removed entirely; it
   fought the HDR sky and either muddied or milked the frame).
2. **Physical sky and light.** `Atmosphere::earthlike` scattering sky (HDR)
   with `Exposure::SUNLIGHT`, a 100 k-lux sun, 15 k-lux sky-fill ambient,
   and `Bloom::NATURAL` so emissives and muzzle flashes glow.
3. **Material identity per block.** The registry gains `metallic`,
   `roughness`, `emissive`; `setup_assets` feeds them to the PBR material.
   Iron/cannon/culverin are metallic, gold mirrors, tar is matte, and the
   stern lantern is emissive — castled ships carry glowing sterns at no
   extra cost, and new blocks stay a registry-only change.

## Performance

Heaviest scene (`--demo --boss --diag`) runs ~30–38 fps, down from ~50.
Attribution tests: ocean grid at 48 vs 120 subdivisions — no change; bloom
off — no change. The cost is the pre-existing per-cube entity rendering
under the brighter shadow-casting sun plus run-to-run battle variance;
greedy meshing (roadmap) remains the real mitigation.

## Verification

- `--selftest` passes exactly (scrap 1 → place 0); screenshots show swell
  shading, a lit hull riding the waves, and glowing lanterns.
- No panics over 40 s of `--demo --boss`.
