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

pub struct Voxel {
    pub id: BlockId,
    /// The child entity rendering this block's cube.
    pub entity: Entity,
}

/// The ship's voxel grid in local block coordinates. Block (0,0,0) is the
/// aft-port-bottom corner; y = 0 is the layer at the waterline.
#[derive(Component)]
pub struct ShipVoxels {
    pub blocks: HashMap<IVec3, Voxel>,
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
    assets: &GameAssets,
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

    let mut blocks = HashMap::new();
    commands.entity(ship).with_children(|parent| {
        for (pos, id) in &layout {
            let entity = parent
                .spawn((
                    Mesh3d(assets.cube.clone()),
                    MeshMaterial3d(assets.block_materials[id].clone()),
                    Transform::from_translation(
                        (pos.as_vec3() + Vec3::splat(0.5)) * BLOCK_SIZE - center,
                    ),
                ))
                .id();
            blocks.insert(*pos, Voxel { id: *id, entity });
        }
    });

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
        blocks,
        plan: layout,
        center,
        radius,
    });
    ship
}

/// Shared hull builder: a `length` x `width` deck-on-hull slab with masts
/// (plus square sails) at the given x positions and cannons along both rails.
fn hull_layout(
    length: i32,
    width: i32,
    hull: BlockId,
    mast_xs: &[i32],
    mast_height: i32,
    cannon_xs: &[i32],
) -> HashMap<IVec3, BlockId> {
    let mut layout = HashMap::new();
    for x in 0..length {
        for z in 0..width {
            layout.insert(IVec3::new(x, 0, z), hull);
            layout.insert(IVec3::new(x, 1, z), BlockId::OakDeck);
        }
    }
    let mast_z = width / 2;
    for &x in mast_xs {
        for y in 2..2 + mast_height {
            layout.insert(IVec3::new(x, y, mast_z), BlockId::Mast);
        }
        for y in 3..1 + mast_height {
            for z in 0..width {
                if z != mast_z {
                    layout.insert(IVec3::new(x, y, z), BlockId::Sail);
                }
            }
        }
    }
    for &x in cannon_xs {
        for z in [0, width - 1] {
            layout.insert(IVec3::new(x, 2, z), BlockId::Cannon);
        }
    }
    layout
}

/// Tier 0: the starter barge — 8x4, one mast, two cannons per side.
pub fn barge_layout() -> HashMap<IVec3, BlockId> {
    hull_layout(8, 4, BlockId::OakHull, &[4], 5, &[2, 6])
}

/// Tier 1: brig — 10x5, two masts, three cannons per side.
pub fn brig_layout() -> HashMap<IVec3, BlockId> {
    hull_layout(10, 5, BlockId::OakHull, &[2, 6], 5, &[1, 4, 8])
}

/// Tier 2: frigate — 12x5, iron hull, four cannons per side.
pub fn frigate_layout() -> HashMap<IVec3, BlockId> {
    hull_layout(12, 5, BlockId::IronHull, &[3, 8], 6, &[1, 4, 7, 10])
}

/// Tier 3: galleon — 14x6, iron hull, three masts, five cannons per side.
pub fn galleon_layout() -> HashMap<IVec3, BlockId> {
    hull_layout(14, 6, BlockId::IronHull, &[3, 7, 11], 6, &[1, 4, 7, 10, 13])
}

/// Smallest hostile: 7x3 sloop, two cannons per side.
pub fn sloop_layout() -> HashMap<IVec3, BlockId> {
    hull_layout(7, 3, BlockId::OakHull, &[3], 4, &[1, 5])
}

/// The boss: a 20x8 iron monster with seven cannons per side.
pub fn dreadnought_layout() -> HashMap<IVec3, BlockId> {
    hull_layout(
        20,
        8,
        BlockId::IronHull,
        &[3, 8, 13, 17],
        7,
        &[1, 4, 7, 10, 13, 16, 18],
    )
}

/// Spawn the player's ship for the given upgrade tier.
pub fn spawn_player(
    commands: &mut Commands,
    assets: &GameAssets,
    tier: usize,
    position: Vec3,
    yaw: f32,
) -> Entity {
    let class = &PLAYER_CLASSES[tier];
    let ship = spawn_ship(
        commands,
        assets,
        (class.layout)(),
        position,
        yaw,
        class.reload,
        class.top_speed,
    );
    commands.entity(ship).insert(PlayerShip);
    ship
}

pub fn spawn_player_start(mut commands: Commands, assets: Res<GameAssets>, stats: Res<GameStats>) {
    spawn_player(&mut commands, &assets, stats.tier, Vec3::ZERO, 0.0);
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

/// Fake buoyancy until real per-block physics: bob on a sine swell and
/// combine heading with a gentle roll/pitch. Phase varies with position so
/// ships don't bob in lockstep, and battle damage makes a ship ride lower.
pub fn float_ships(
    time: Res<Time>,
    mut ships: Query<(&Ship, &ShipVoxels, &mut Transform), Without<Sinking>>,
) {
    let t = time.elapsed_secs();
    for (ship, voxels, mut transform) in &mut ships {
        let draft = voxels.damage_fraction() * 0.7;
        let phase = transform.translation.x * 0.13 + transform.translation.z * 0.17;
        transform.translation.y = (t * 0.9 + phase).sin() * 0.12 - draft;
        let roll = (t * 0.7 + phase).sin() * 0.025;
        let pitch = (t * 0.5 + phase).cos() * 0.015;
        transform.rotation = Quat::from_rotation_y(ship.yaw)
            * Quat::from_rotation_x(roll)
            * Quat::from_rotation_z(pitch);
    }
}

/// After the player's ship has gone down, R launches a fresh ship of the
/// same tier — death costs banked salvage progress only in time.
pub fn respawn_player(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut stats: ResMut<GameStats>,
    players: Query<(Entity, Has<Sinking>), With<PlayerShip>>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    if players.iter().any(|(_, sinking)| !sinking) {
        return;
    }
    for (entity, _) in &players {
        commands.entity(entity).despawn();
    }
    spawn_player(&mut commands, &assets, stats.tier, Vec3::ZERO, 0.0);
    stats.player_sunk = false;
}
