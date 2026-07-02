use std::collections::HashMap;

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::blocks::BlockId;
use crate::combat::{Broadsides, GameStats, Sinking};

pub const BLOCK_SIZE: f32 = 1.0;

/// A sailable vessel. Yaw and speed live here (not in the Transform) because
/// the float system rewrites the transform's rotation every frame to combine
/// heading with wave roll/pitch.
#[derive(Component)]
pub struct Ship {
    pub yaw: f32,
    pub speed: f32,
    pub top_speed: f32,
}

/// Steering intent, written by player input or enemy AI and applied by
/// [`drive_ships`]. Thrust and turn are both in [-1, 1].
#[derive(Component, Default)]
pub struct Helm {
    pub thrust: f32,
    pub turn: f32,
}

#[derive(Component)]
pub struct PlayerShip;

/// The ship's voxel grid in local block coordinates. Block (0,0,0) is the
/// aft-port-bottom corner; y = 0 is the layer at the waterline.
///
/// The grid is pure data: rendering is two ship-wide meshes rebuilt by
/// [`remesh_ships`] whenever this component changes (Bevy change detection),
/// so systems just mutate `blocks` and the visuals follow.
#[derive(Component)]
pub struct ShipVoxels {
    pub blocks: HashMap<IVec3, BlockId>,
    /// The as-designed layout; cells present here but missing from `blocks`
    /// are battle damage that salvage can repair.
    pub plan: HashMap<IVec3, BlockId>,
    /// Offset from grid space to the ship's local origin: the center of the
    /// footprint at the waterline, so wave roll rotates about the midpoint.
    pub center: Vec3,
    /// Flat (xz) radius of the hull around its origin, for coarse
    /// ship-vs-ship separation.
    pub radius: f32,
}

impl ShipVoxels {
    pub fn world_to_grid(&self, ship_transform: &Transform, world: Vec3) -> IVec3 {
        let local = ship_transform
            .compute_affine()
            .inverse()
            .transform_point3(world);
        ((local + self.center) / BLOCK_SIZE).floor().as_ivec3()
    }

    pub fn grid_to_world(&self, ship_transform: &Transform, cell: IVec3) -> Vec3 {
        ship_transform.transform_point(self.local_offset(cell))
    }

    /// A cell's translation relative to the ship entity (child transform).
    pub fn local_offset(&self, cell: IVec3) -> Vec3 {
        (cell.as_vec3() + Vec3::splat(0.5)) * BLOCK_SIZE - self.center
    }

    /// Fraction of the design currently missing (battle damage).
    pub fn damage_fraction(&self) -> f32 {
        1.0 - self.blocks.len() as f32 / self.plan.len().max(1) as f32
    }
}

/// A hull design plus its sailing characteristics. Used for the player's
/// upgrade ladder and for enemy variety.
pub struct ShipClass {
    pub name: &'static str,
    pub layout: fn() -> HashMap<IVec3, BlockId>,
    pub reload: f32,
    pub top_speed: f32,
}

/// The player's upgrade ladder; salvage pays for each step up.
pub const PLAYER_CLASSES: [ShipClass; 4] = [
    ShipClass {
        name: "Barge",
        layout: barge_layout,
        reload: 2.2,
        top_speed: 6.0,
    },
    ShipClass {
        name: "Brig",
        layout: brig_layout,
        reload: 2.0,
        top_speed: 6.4,
    },
    ShipClass {
        name: "Frigate",
        layout: frigate_layout,
        reload: 1.9,
        top_speed: 6.8,
    },
    ShipClass {
        name: "Galleon",
        layout: galleon_layout,
        reload: 1.8,
        top_speed: 7.0,
    },
];

/// Salvage cost to step from tier i to tier i + 1. Tuned against flotsam
/// values: a fully scavenged kill yields roughly 15-25 salvage.
pub const UPGRADE_COSTS: [u32; 3] = [20, 60, 140];

/// Spawn a ship from a block layout. The grid is centered on its footprint
/// so the hull rolls about its midpoint; grid y = 1 sits on the waterline.
pub fn spawn_ship(
    commands: &mut Commands,
    layout: HashMap<IVec3, BlockId>,
    position: Vec3,
    yaw: f32,
    reload_time: f32,
    top_speed: f32,
) -> Entity {
    let (mut min, mut max) = (IVec3::MAX, IVec3::MIN);
    for pos in layout.keys() {
        min = min.min(*pos);
        max = max.max(*pos);
    }
    let center = Vec3::new(
        (min.x + max.x + 1) as f32 * 0.5,
        1.0,
        (min.z + max.z + 1) as f32 * 0.5,
    ) * BLOCK_SIZE;

    let ship = commands
        .spawn((
            Ship {
                yaw,
                speed: 0.0,
                top_speed,
            },
            Helm::default(),
            Broadsides::new(reload_time),
            Transform::from_translation(position).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .id();

    let radius = layout
        .keys()
        .map(|pos| {
            ((pos.as_vec3() + Vec3::splat(0.5)) * BLOCK_SIZE - center)
                .xz()
                .length()
        })
        .fold(0.0_f32, f32::max)
        + 0.5;
    commands.entity(ship).insert(ShipVoxels {
        blocks: layout.clone(),
        plan: layout,
        center,
        radius,
    });
    ship
}

/// Handles to a ship's two render meshes: all opaque blocks in one
/// atlas-textured mesh, translucent blocks (sails) in a second blended,
/// non-shadow-casting one. Two draw calls per ship instead of an entity
/// per cube — the difference between a fleet and a slideshow.
#[derive(Component)]
pub struct ShipMeshes {
    opaque: Handle<Mesh>,
    translucent: Handle<Mesh>,
}

/// (Re)build the render meshes of any ship whose voxel grid changed this
/// frame — spawning, cannon damage, ramming, building, salvage repair. Only
/// faces exposed to air (or showing through a translucent neighbour) are
/// emitted, so solid hull interiors cost nothing.
pub fn remesh_ships(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    ships: Query<(Entity, &ShipVoxels, Option<&ShipMeshes>), Changed<ShipVoxels>>,
) {
    for (entity, voxels, handles) in &ships {
        let opaque = build_ship_mesh(voxels, false);
        let translucent = build_ship_mesh(voxels, true);
        match handles {
            Some(handles) => {
                let _ = meshes.insert(&handles.opaque, opaque);
                let _ = meshes.insert(&handles.translucent, translucent);
            }
            None => {
                let opaque = meshes.add(opaque);
                let translucent = meshes.add(translucent);
                commands.entity(entity).with_children(|parent| {
                    parent.spawn((
                        Mesh3d(opaque.clone()),
                        MeshMaterial3d(assets.atlas_opaque.clone()),
                        Transform::IDENTITY,
                    ));
                    parent.spawn((
                        Mesh3d(translucent.clone()),
                        MeshMaterial3d(assets.atlas_translucent.clone()),
                        Transform::IDENTITY,
                        // Translucent sails shouldn't cast opaque shadows.
                        bevy::light::NotShadowCaster,
                    ));
                });
                commands.entity(entity).insert(ShipMeshes {
                    opaque,
                    translucent,
                });
            }
        }
    }
}

/// Cube face table: outward normal and the four corners of that face
/// (relative to the cube center, CCW seen from outside).
const FACES: [(IVec3, [Vec3; 4]); 6] = [
    (
        IVec3::X,
        [
            Vec3::new(0.5, -0.5, 0.5),
            Vec3::new(0.5, -0.5, -0.5),
            Vec3::new(0.5, 0.5, -0.5),
            Vec3::new(0.5, 0.5, 0.5),
        ],
    ),
    (
        IVec3::NEG_X,
        [
            Vec3::new(-0.5, -0.5, -0.5),
            Vec3::new(-0.5, -0.5, 0.5),
            Vec3::new(-0.5, 0.5, 0.5),
            Vec3::new(-0.5, 0.5, -0.5),
        ],
    ),
    (
        IVec3::Y,
        [
            Vec3::new(-0.5, 0.5, 0.5),
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::new(0.5, 0.5, -0.5),
            Vec3::new(-0.5, 0.5, -0.5),
        ],
    ),
    (
        IVec3::NEG_Y,
        [
            Vec3::new(-0.5, -0.5, -0.5),
            Vec3::new(0.5, -0.5, -0.5),
            Vec3::new(0.5, -0.5, 0.5),
            Vec3::new(-0.5, -0.5, 0.5),
        ],
    ),
    (
        IVec3::Z,
        [
            Vec3::new(-0.5, -0.5, 0.5),
            Vec3::new(0.5, -0.5, 0.5),
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::new(-0.5, 0.5, 0.5),
        ],
    ),
    (
        IVec3::NEG_Z,
        [
            Vec3::new(0.5, -0.5, -0.5),
            Vec3::new(-0.5, -0.5, -0.5),
            Vec3::new(-0.5, 0.5, -0.5),
            Vec3::new(0.5, 0.5, -0.5),
        ],
    ),
];

/// Mesh one translucency class of a ship's blocks: every block face that is
/// exposed (no neighbour, or an opaque face showing through a translucent
/// neighbour), textured from the block's atlas tile.
fn build_ship_mesh(voxels: &ShipVoxels, translucent: bool) -> Mesh {
    let is_translucent = |id: BlockId| crate::blocks::def(id).color.alpha() < 1.0;
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for (cell, id) in &voxels.blocks {
        if is_translucent(*id) != translucent {
            continue;
        }
        let center = voxels.local_offset(*cell);
        let (uv_min, uv_max) = crate::assets::tile_uv(*id);
        for (normal, corners) in FACES {
            let visible = match voxels.blocks.get(&(*cell + normal)) {
                None => true,
                // A face against a translucent neighbour still shows
                // through it; anything behind an opaque block never does.
                Some(neighbor) => !translucent && is_translucent(*neighbor),
            };
            if !visible {
                continue;
            }
            let base = positions.len() as u32;
            for (k, corner) in corners.into_iter().enumerate() {
                positions.push((center + corner * BLOCK_SIZE).to_array());
                normals.push(normal.as_vec3().to_array());
                let (u, v) = match k {
                    0 => (uv_min.x, uv_max.y),
                    1 => (uv_max.x, uv_max.y),
                    2 => (uv_max.x, uv_min.y),
                    _ => (uv_min.x, uv_min.y),
                };
                uvs.push([u, v]);
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::mesh::Indices::U32(indices));
    mesh
}

/// A tall-ship recipe (Spec 002). `build` turns one of these into a tapered,
/// multi-level hull with bulwarks, gun ports, castles, masts, and rigging.
/// Bow is +x (the ship's forward), the centerline runs down z = beam / 2.
struct ShipSpec {
    /// Hull length along the keel (number of x stations).
    length: i32,
    /// Maximum beam (width in z) amidships; should be odd for a clean
    /// centerline. The hull tapers away from this fore and aft.
    beam: i32,
    /// Hull material — `OakHull` or `IronHull`.
    hull: BlockId,
    /// Solid hull layers from the keel up (waterline and below).
    hull_height: i32,
    /// Stations over which the bow narrows to its cutwater point.
    bow: i32,
    /// Stations over which the stern fills out to its transom.
    stern: i32,
    /// Masts as (x station, pole height above the deck), aft-most first —
    /// the first entry carries the spanker on castled ships.
    masts: &'static [(i32, i32)],
    /// The guns mounted along each rail, aft to fore; both sides carry the
    /// same battery. Mixing gun blocks mixes shot types in one broadside.
    battery: &'static [BlockId],
    /// Whether to raise a quarterdeck aft and a forecastle forward.
    castles: bool,
}

/// Half-beam (in z) of the hull at station `x`: full amidships, tapering to a
/// point at the bow and to a broad transom at the stern.
fn half_width(spec: &ShipSpec, x: i32) -> i32 {
    let max = (spec.beam - 1) / 2;
    if x >= spec.length - spec.bow {
        // Bow: shrink from full beam to 0 (a single cutwater column) at the tip.
        let into = x - (spec.length - spec.bow) + 1;
        let frac = into as f32 / spec.bow as f32;
        (max as f32 * (1.0 - frac)).round().max(0.0) as i32
    } else if x < spec.stern {
        // Stern: grow from a half-beam transom up to full beam.
        let stern_min = (max + 1) / 2;
        let frac = x as f32 / spec.stern as f32;
        (stern_min as f32 + (max - stern_min) as f32 * frac).round() as i32
    } else {
        max
    }
}

/// Render a `ShipSpec` into a voxel layout.
fn build(spec: &ShipSpec) -> HashMap<IVec3, BlockId> {
    let mut m = HashMap::new();
    let cz = spec.beam / 2;
    let deck_y = spec.hull_height;
    let rail_y = deck_y + 1;

    // Solid tapered hull, capped by a weather deck.
    for x in 0..spec.length {
        let half = half_width(spec, x);
        for z in (cz - half)..=(cz + half) {
            for y in 0..spec.hull_height {
                m.insert(IVec3::new(x, y, z), spec.hull);
            }
            m.insert(IVec3::new(x, deck_y, z), BlockId::OakDeck);
        }
    }

    // Wale: a tarred band along the topmost hull strake and around the
    // transom — the classic dark stripe that makes the sheer line read.
    let wale_y = spec.hull_height - 1;
    let stern_half = half_width(spec, 0);
    for x in 0..spec.length {
        let half = half_width(spec, x);
        m.insert(IVec3::new(x, wale_y, cz - half), BlockId::Trim);
        m.insert(IVec3::new(x, wale_y, cz + half), BlockId::Trim);
    }
    for z in (cz - stern_half)..=(cz + stern_half) {
        m.insert(IVec3::new(0, wale_y, z), BlockId::Trim);
    }

    // Bulwarks: a raised rail down both sides; the open bow tip stays low.
    for x in 0..spec.length {
        let half = half_width(spec, x);
        if half <= 0 {
            continue;
        }
        m.insert(IVec3::new(x, rail_y, cz - half), spec.hull);
        m.insert(IVec3::new(x, rail_y, cz + half), spec.hull);
    }
    // Closed stern transom across the back.
    for z in (cz - stern_half)..=(cz + stern_half) {
        m.insert(IVec3::new(0, rail_y, z), spec.hull);
    }

    // Castles raise and wall the ends; guns go in the clear waist between
    // them, so the waist span depends on whether this ship has castles.
    let (mut waist_lo, mut waist_hi) = (spec.stern, spec.length - spec.bow);
    if spec.castles {
        // Quarterdeck: raise and wall the stern third for a sterncastle.
        let q_len = (spec.stern + 1).min(spec.length);
        raise_castle(&mut m, spec, cz, rail_y, 0, q_len);
        // Forecastle: a short raised deck just aft of the bow taper.
        let f_end = (spec.length - spec.bow + 1).min(spec.length);
        let f_start = (f_end - 3).max(q_len);
        raise_castle(&mut m, spec, cz, rail_y, f_start, f_end);
        waist_lo = q_len;
        waist_hi = f_start;

        // Stern gallery: lantern-lit windows in the sterncastle bulkhead,
        // a great lantern above them, and a gilded figurehead at the stem.
        for z in (cz - stern_half)..=(cz + stern_half) {
            if (z - cz).rem_euclid(2) == 1 {
                m.insert(IVec3::new(0, rail_y + 1, z), BlockId::Lantern);
            }
        }
        m.insert(IVec3::new(0, rail_y + 2, cz), BlockId::Lantern);
        m.insert(IVec3::new(spec.length - 1, rail_y, cz), BlockId::Gold);
    }

    // Cargo hatch: a dark grating amidships, clear of the mast steps.
    let mid = (waist_lo + waist_hi) / 2;
    for x in [mid - 1, mid] {
        if x > waist_lo && x < waist_hi && !spec.masts.iter().any(|&(mx, _)| mx == x) {
            m.insert(IVec3::new(x, deck_y, cz), BlockId::Trim);
        }
    }

    // Gun ports: the battery spaced evenly along the rail through the waist,
    // the same mix on both sides.
    let gun_count = spec.battery.len() as i32;
    for (i, gun) in spec.battery.iter().enumerate() {
        let x = waist_lo + (waist_hi - waist_lo) * (i as i32 * 2 + 1) / (gun_count * 2);
        let half = half_width(spec, x);
        if half <= 0 {
            continue;
        }
        m.insert(IVec3::new(x, rail_y, cz - half), *gun);
        m.insert(IVec3::new(x, rail_y, cz + half), *gun);
    }

    // Masts: pole, stacked square sails on yards (two on short masts, three
    // on tall ones, narrowing aloft), and a masthead pennant.
    let sail_hw = (spec.beam - 1) / 2 + 1;
    for &(mx, mh) in spec.masts {
        for y in rail_y..=rail_y + mh {
            m.insert(IVec3::new(mx, y, cz), BlockId::Mast);
        }
        m.insert(IVec3::new(mx, rail_y + mh + 1, cz), BlockId::Flag);
        let sails = if mh >= 9 { 3 } else { 2 };
        let rig_lo = rail_y + mh / 4;
        let rig_hi = rail_y + mh - 1;
        let band = ((rig_hi - rig_lo + 1) / sails).max(1);
        for s in 0..sails {
            let y0 = rig_lo + s * (band + 1);
            if y0 > rig_hi {
                break;
            }
            add_sail(
                &mut m,
                mx,
                cz,
                (sail_hw - s).max(2),
                y0,
                (y0 + band - 1).min(rig_hi),
            );
        }
    }

    // Spanker: a fore-aft triangle of canvas trailing off the aft mast,
    // the age-of-sail counterweight to all that square rig.
    if spec.castles {
        let (mx, mh) = spec.masts[0];
        let gaff = rail_y + mh * 2 / 3;
        for k in 1..=3 {
            for y in (rail_y + 2)..=(gaff - k) {
                m.entry(IVec3::new(mx - k, y, cz)).or_insert(BlockId::Sail);
            }
        }
    }

    // Bowsprit angling up and forward off the bow, carrying jib sails.
    let tip = spec.length - 1;
    for k in 1..=3 {
        m.insert(IVec3::new(tip + k, rail_y + k.min(2), cz), BlockId::Mast);
    }
    for k in 1..=2 {
        m.insert(IVec3::new(tip + k, rail_y + k, cz - 1), BlockId::Sail);
        m.insert(IVec3::new(tip + k, rail_y + k, cz + 1), BlockId::Sail);
        m.entry(IVec3::new(tip + k, rail_y + k + 1, cz))
            .or_insert(BlockId::Sail);
    }

    m
}

/// Raise a walled deck (a fore- or quarter-castle) over stations `x0..x1`:
/// an `OakDeck` floor one level up with hull-block walls around it.
fn raise_castle(
    m: &mut HashMap<IVec3, BlockId>,
    spec: &ShipSpec,
    cz: i32,
    rail_y: i32,
    x0: i32,
    x1: i32,
) {
    for x in x0..x1 {
        let half = half_width(spec, x);
        if half <= 0 {
            continue;
        }
        for z in (cz - half)..=(cz + half) {
            m.insert(IVec3::new(x, rail_y, z), BlockId::OakDeck);
        }
        m.insert(IVec3::new(x, rail_y + 1, cz - half), spec.hull);
        m.insert(IVec3::new(x, rail_y + 1, cz + half), spec.hull);
    }
    // End bulkheads close the castle off fore and aft.
    for &x in &[x0, x1 - 1] {
        let half = half_width(spec, x);
        for z in (cz - half)..=(cz + half) {
            m.insert(IVec3::new(x, rail_y + 1, z), spec.hull);
        }
    }
}

/// A square sail: a flat panel in the y-z plane at station `x`, hung beneath a
/// horizontal yard. Leaves the mast column intact (sails don't overwrite it).
fn add_sail(m: &mut HashMap<IVec3, BlockId>, x: i32, cz: i32, hw: i32, y0: i32, y1: i32) {
    for z in (cz - hw)..=(cz + hw) {
        m.insert(IVec3::new(x, y1 + 1, z), BlockId::Mast); // the yard
    }
    for y in y0..=y1 {
        for z in (cz - hw)..=(cz + hw) {
            m.entry(IVec3::new(x, y, z)).or_insert(BlockId::Sail);
        }
    }
}

/// Tier 0: the starter barge — humble, single-masted, two guns a side.
pub fn barge_layout() -> HashMap<IVec3, BlockId> {
    build(&ShipSpec {
        length: 12,
        beam: 5,
        hull: BlockId::OakHull,
        hull_height: 2,
        bow: 3,
        stern: 2,
        masts: &[(6, 6)],
        battery: &[BlockId::Cannon, BlockId::Cannon],
        castles: false,
    })
}

/// Tier 1: brig — two masts, a raised stern, three guns a side.
pub fn brig_layout() -> HashMap<IVec3, BlockId> {
    build(&ShipSpec {
        length: 15,
        beam: 7,
        hull: BlockId::OakHull,
        hull_height: 2,
        bow: 4,
        stern: 3,
        masts: &[(5, 7), (10, 8)],
        battery: &[BlockId::Carronade, BlockId::Cannon, BlockId::Cannon],
        castles: true,
    })
}

/// Tier 2: frigate — iron-hulled, three masts, four guns a side.
pub fn frigate_layout() -> HashMap<IVec3, BlockId> {
    build(&ShipSpec {
        length: 19,
        beam: 7,
        hull: BlockId::IronHull,
        hull_height: 2,
        bow: 5,
        stern: 4,
        masts: &[(5, 8), (10, 9), (15, 7)],
        battery: &[
            BlockId::Culverin,
            BlockId::Cannon,
            BlockId::Cannon,
            BlockId::Culverin,
        ],
        castles: true,
    })
}

/// Tier 3: galleon — broad iron hull, towering rig, five guns a side.
pub fn galleon_layout() -> HashMap<IVec3, BlockId> {
    build(&ShipSpec {
        length: 23,
        beam: 9,
        hull: BlockId::IronHull,
        hull_height: 2,
        bow: 6,
        stern: 5,
        masts: &[(6, 9), (12, 11), (18, 8)],
        battery: &[
            BlockId::Carronade,
            BlockId::Cannon,
            BlockId::Culverin,
            BlockId::Cannon,
            BlockId::Carronade,
        ],
        castles: true,
    })
}

/// Smallest hostile: a nimble single-masted sloop.
pub fn sloop_layout() -> HashMap<IVec3, BlockId> {
    build(&ShipSpec {
        length: 11,
        beam: 5,
        hull: BlockId::OakHull,
        hull_height: 2,
        bow: 3,
        stern: 2,
        masts: &[(5, 6)],
        battery: &[BlockId::Cannon, BlockId::Cannon],
        castles: false,
    })
}

/// The boss: a four-masted iron leviathan, seven guns a side.
pub fn dreadnought_layout() -> HashMap<IVec3, BlockId> {
    build(&ShipSpec {
        length: 29,
        beam: 11,
        hull: BlockId::IronHull,
        hull_height: 3,
        bow: 7,
        stern: 6,
        masts: &[(7, 10), (13, 13), (19, 12), (24, 9)],
        battery: &[
            BlockId::Carronade,
            BlockId::Culverin,
            BlockId::Cannon,
            BlockId::Cannon,
            BlockId::Cannon,
            BlockId::Culverin,
            BlockId::Carronade,
        ],
        castles: true,
    })
}

/// Spawn the player's ship for the given upgrade tier.
pub fn spawn_player(commands: &mut Commands, tier: usize, position: Vec3, yaw: f32) -> Entity {
    let class = &PLAYER_CLASSES[tier];
    let ship = spawn_ship(
        commands,
        (class.layout)(),
        position,
        yaw,
        class.reload,
        class.top_speed,
    );
    commands.entity(ship).insert(PlayerShip);
    ship
}

/// WASD steers; Q/E fire broadsides for keyboard-only play (the primary
/// firing control is the mouse, see [`player_fire_mouse`]).
pub fn player_helm(
    keys: Res<ButtonInput<KeyCode>>,
    mut players: Query<(&mut Helm, &mut Broadsides), With<PlayerShip>>,
) {
    for (mut helm, mut guns) in &mut players {
        let mut thrust = 0.0;
        if keys.pressed(KeyCode::KeyW) {
            thrust += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) {
            thrust -= 0.4;
        }
        let mut turn = 0.0;
        if keys.pressed(KeyCode::KeyA) {
            turn += 1.0;
        }
        if keys.pressed(KeyCode::KeyD) {
            turn -= 1.0;
        }
        helm.thrust = thrust;
        helm.turn = turn;
        if keys.just_pressed(KeyCode::KeyQ) {
            guns.fire_port = true;
        }
        if keys.just_pressed(KeyCode::KeyE) {
            guns.fire_starboard = true;
        }
    }
}

/// Mouse gunnery (Spec 001): the click is projected onto the sea, and the
/// guns converge on that point. The button picks the side — left mouse fires
/// the port (left) broadside, right mouse the starboard (right) — and a gun
/// only fires if it can swing to bear on the point, so clicking the wrong
/// side is a harmless no-op. Keeps sailing on WASD and gunnery on the mouse.
pub fn player_fire_mouse(
    mode: Res<crate::build::PlayMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    aim: Res<crate::build::AimOverride>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut players: Query<&mut Broadsides, With<PlayerShip>>,
) {
    if *mode != crate::build::PlayMode::Sail {
        return;
    }
    let fire_port = mouse.just_pressed(MouseButton::Left);
    let fire_starboard = mouse.just_pressed(MouseButton::Right);
    if !fire_port && !fire_starboard {
        return;
    }
    let Some(ray) = crate::build::cursor_ray(&windows, &cameras, &aim) else {
        return;
    };
    if ray.direction.y.abs() < 1e-4 {
        return;
    }
    let t = -ray.origin.y / ray.direction.y;
    if t < 0.0 {
        return;
    }
    let aim_point = ray.origin + ray.direction * t;
    for mut guns in &mut players {
        if fire_port {
            guns.aim_port = Some(aim_point);
        }
        if fire_starboard {
            guns.aim_starboard = Some(aim_point);
        }
    }
}

/// Apply helm intent: thrust with water drag, turning authority that scales
/// with speed — a ship dead in the water barely answers the helm. Running
/// with the wind is faster than beating into it, for every ship alike.
pub fn drive_ships(
    time: Res<Time>,
    wind: Res<crate::ocean::Wind>,
    mut ships: Query<(&mut Ship, &Helm, &mut Transform), Without<Sinking>>,
) {
    let dt = time.delta_secs();
    let wind_dir = wind.dir();
    for (mut ship, helm, mut transform) in &mut ships {
        ship.yaw += helm.turn * dt * (0.2 + 0.15 * ship.speed.abs());
        let forward = Quat::from_rotation_y(ship.yaw) * Vec3::X;
        let wind_factor = 1.0 + 0.25 * forward.dot(wind_dir);
        let speed = (ship.speed + helm.thrust * 3.0 * wind_factor * dt) * (1.0 - 0.3 * dt);
        ship.speed = speed.clamp(-2.0, ship.top_speed * wind_factor);
        transform.translation += forward * ship.speed * dt;
    }
}

/// Coarse collision between hulls: overlapping ships shoulder each other
/// apart, and a hard closing contact is a ram — both hulls splinter at the
/// contact point and lose way.
pub fn separate_ships(
    mut commands: Commands,
    assets: Res<GameAssets>,
    sounds: Res<crate::audio::SoundBank>,
    mut stats: ResMut<GameStats>,
    mut ships: Query<
        (
            Entity,
            &mut Ship,
            &mut ShipVoxels,
            &mut Transform,
            Has<PlayerShip>,
            Has<crate::salvage::Derelict>,
            Has<crate::enemy::Dreadnought>,
        ),
        Without<Sinking>,
    >,
) {
    let mut pairs = ships.iter_combinations_mut();
    while let Some(
        [
            (entity_a, mut ship_a, mut voxels_a, mut a, player_a, derelict_a, boss_a),
            (entity_b, mut ship_b, mut voxels_b, mut b, player_b, derelict_b, boss_b),
        ],
    ) = pairs.fetch_next()
    {
        let mut delta = b.translation - a.translation;
        delta.y = 0.0;
        let distance = delta.length();
        // Circles overstate long narrow hulls; allow some overlap.
        let min_distance = (voxels_a.radius + voxels_b.radius) * 0.7;
        if distance >= min_distance || distance <= 0.001 {
            continue;
        }
        let direction = delta / distance;
        let push = direction * ((min_distance - distance) * 0.5);
        a.translation -= push;
        b.translation += push;

        let velocity_a = Quat::from_rotation_y(ship_a.yaw) * Vec3::X * ship_a.speed;
        let velocity_b = Quat::from_rotation_y(ship_b.yaw) * Vec3::X * ship_b.speed;
        let closing = (velocity_a - velocity_b).dot(direction);
        if closing < 2.5 {
            continue;
        }

        // Ram: splinter both hulls where they meet and kill most of the way.
        let contact = (a.translation + b.translation) * 0.5;
        crate::audio::play(&mut commands, &sounds.crunch, 0.9);
        ship_a.speed *= 0.4;
        ship_b.speed *= 0.4;
        let mut ram = |voxels: &mut ShipVoxels,
                       transform: &Transform,
                       entity: Entity,
                       kick: Vec3,
                       is_player: bool,
                       is_derelict: bool,
                       is_boss: bool,
                       credit: bool| {
            let near_cell = voxels.world_to_grid(transform, contact);
            let target = voxels
                .blocks
                .keys()
                .min_by(|x, y| {
                    let dx = (**x - near_cell).as_vec3().length_squared();
                    let dy = (**y - near_cell).as_vec3().length_squared();
                    dx.total_cmp(&dy)
                })
                .copied();
            let Some(cell) = target else {
                return;
            };
            if (cell - near_cell).as_vec3().length() > 3.0 {
                return;
            }
            let sank = crate::combat::apply_blast(
                &mut commands,
                &assets,
                voxels,
                transform,
                cell,
                1.3,
                kick,
            );
            if sank {
                crate::combat::start_sinking(&mut commands, &assets, entity, transform, voxels);
                crate::combat::record_sunk(&mut stats, is_player, is_derelict, is_boss, credit);
            }
        };
        ram(
            &mut voxels_a,
            &a,
            entity_a,
            -direction * 2.0,
            player_a,
            derelict_a,
            boss_a,
            player_b,
        );
        ram(
            &mut voxels_b,
            &b,
            entity_b,
            direction * 2.0,
            player_b,
            derelict_b,
            boss_b,
            player_a,
        );
    }
}

/// Simplified buoyancy until real per-block physics: ships ride the shared
/// ocean swell ([`crate::ocean::wave_height`]), taking their height from the
/// surface under the hull and their pitch/roll from the wave slope sampled
/// bow-to-stern and beam-to-beam — softened so a hull reads as massive
/// rather than cork-like. Battle damage makes a ship ride lower.
pub fn float_ships(
    time: Res<Time>,
    sea: Res<crate::dock::SeaState>,
    mut ships: Query<(&Ship, &ShipVoxels, &mut Transform), Without<Sinking>>,
) {
    // Wrapped, to stay in phase with the GPU ocean's `globals.time`; scaled
    // by the sea state so hulls sit flat in the sheltered cove.
    let t = time.elapsed_secs_wrapped();
    let scale = sea.current;
    for (ship, voxels, mut transform) in &mut ships {
        let draft = voxels.damage_fraction() * 0.7;
        let p = transform.translation.xz();
        let forward = Vec2::new(ship.yaw.cos(), -ship.yaw.sin());
        let beam = Vec2::new(-forward.y, forward.x);
        let half_len = (voxels.radius * 0.7).max(2.0);
        let half_beam = (voxels.radius * 0.35).max(1.5);
        transform.translation.y = crate::ocean::wave_height(p, t) * scale - draft;
        let bow = crate::ocean::wave_height(p + forward * half_len, t) * scale;
        let stern = crate::ocean::wave_height(p - forward * half_len, t) * scale;
        let starboard = crate::ocean::wave_height(p + beam * half_beam, t) * scale;
        let port = crate::ocean::wave_height(p - beam * half_beam, t) * scale;
        let pitch = ((bow - stern) / (2.0 * half_len)).atan() * 0.8;
        let roll = ((port - starboard) / (2.0 * half_beam)).atan() * 0.6;
        transform.rotation = Quat::from_rotation_y(ship.yaw)
            * Quat::from_rotation_z(pitch)
            * Quat::from_rotation_x(roll);
    }
}
