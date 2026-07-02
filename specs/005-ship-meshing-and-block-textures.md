# Spec 005 — Ship meshing, block textures, GPU ocean

**Status:** accepted (2026-07-02)
**Roadmap:** delivers "greedy meshing" (the standing perf mitigation from
specs 002/004) and the parked "procedural block textures" option.

## Ship meshing (perf)

Per-cube child entities are gone. `ShipVoxels.blocks` is now pure data
(`HashMap<IVec3, BlockId>`); rendering is **two meshes per ship** — one
opaque atlas-textured mesh (casts shadows) and one alpha-blended mesh for
sails (`NotShadowCaster`). `ship::remesh_ships` runs on Bevy change
detection (`Changed<ShipVoxels>`) after all mutating systems, so cannon
damage, ramming, building, and salvage repair just edit the map and the
mesh rebuilds once that frame. Only faces exposed to air (or visible
through a translucent neighbour) are emitted — solid hull interiors cost
nothing. A dreadnought went from ~1.9k draw entities to 2.

Consequences ripple pleasantly: `spawn_ship` no longer needs `GameAssets`,
placement/removal/repair code shrank (no child bookkeeping), and debris,
flotsam, and the build ghost stay as loose cubes with per-block materials.

## Procedural block textures

Every block gets a 16×16 tile generated at startup from a deterministic
hash (same spirit as the procedural audio — no asset files, no rand crate):
oak strakes with staggered butt joints, scrubbed deck planking, riveted
iron plates, vertical mast grain, canvas weave with seams, ringed gun
barrels, glinting gold, ragged bunting, tarred trim, and a lantern with
frame/muntins. Tiles compose into three 64×64 atlases — color,
metallic/roughness (glTF channel convention), and emissive (lantern glass
only) — sampled `nearest` for the crisp voxel look. Patterns multiply the
block's registry color, so recoloring a block in `blocks.rs` re-skins it.
Loose-cube materials reuse the same tiles, so debris matches the hull it
was shot off.

## GPU ocean

The CPU vertex rewrite (spec 004) cost ~8 ms/frame. The sea is now an
`ExtendedMaterial<StandardMaterial, OceanExtension>` whose WGSL vertex
stage (embedded string, `uuid_handle` shader asset — no assets dir)
displaces by the same three-swell sum with analytic normals. Swell
constants reach the shader packed as uniforms from the single Rust
`SWELLS` table, so CPU buoyancy and GPU water can't drift apart. The
shader reads `globals.time`, which wraps hourly — CPU wave sampling
switched to `elapsed_secs_wrapped()` to stay in phase.

Also: directional shadows trimmed to 2 cascades / 150 m (was 4; action is
close-in and shadow passes are dear on integrated GPUs), and the ocean
grid settled at 80 subdivisions — iGPU vertex load ceiling.

## Perf notes (Iris Xe, 720p, noisy box)

Battle scene no longer costs more than an empty sea (~40 vs ~42 fps) —
the per-cube entity cost is confirmed gone. Remaining ceiling is the HDR
pipeline (atmosphere, bloom) plus the displaced ocean on an integrated
GPU; on discrete hardware this should sit at cap. Thermal noise on the
dev box makes finer A/B attribution unreliable (same build swung 26–50).

## Verification

- `--selftest` passes exactly (scrap 1 → place 0) — build/scrap works
  against the meshed hull, including remesh-on-edit.
- Boss-tier screenshots: riveted iron frigate, plank decks, glowing stern
  gallery, canvas sails; damage still knocks matching debris loose.
- 40 s `--demo --boss` autopilot battle, no panics.
