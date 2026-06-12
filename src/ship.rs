use std::collections::HashMap;

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::blocks::BlockId;
use crate::combat::{Broadsides, GameStats, Sinking};

pub const BLOCK_SIZE: f32 = 1.0;

const PLAYER_RELOAD: f32 = 2.2;
const PLAYER_TOP_SPEED: f32 = 6.0;

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
    /// Offset from grid space to the ship's local origin: the center of the
    /// footprint at the waterline, so wave roll rotates about the midpoint.
    pub center: Vec3,
    pub initial_count: usize,
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
        ship_transform
            .transform_point((cell.as_vec3() + Vec3::splat(0.5)) * BLOCK_SIZE - self.center)
    }
}

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

    let initial_count = blocks.len();
    commands.entity(ship).insert(ShipVoxels {
        blocks,
        center,
        initial_count,
    });
    ship
}

/// The starter vessel: a flat 8x4 barge with a mast, a sail, and two cannons
/// per side.
pub fn barge_layout() -> HashMap<IVec3, BlockId> {
    let mut layout = HashMap::new();
    for x in 0..8 {
        for z in 0..4 {
            layout.insert(IVec3::new(x, 0, z), BlockId::OakHull);
            layout.insert(IVec3::new(x, 1, z), BlockId::OakDeck);
        }
    }
    for y in 2..7 {
        layout.insert(IVec3::new(4, y, 2), BlockId::Mast);
    }
    for y in 3..6 {
        for z in [0, 1, 3] {
            layout.insert(IVec3::new(4, y, z), BlockId::Sail);
        }
    }
    for x in [2, 5] {
        for z in [0, 3] {
            layout.insert(IVec3::new(x, 2, z), BlockId::Cannon);
        }
    }
    layout
}

/// Enemy vessel: a narrower 7x3 sloop with two cannons per side.
pub fn sloop_layout() -> HashMap<IVec3, BlockId> {
    let mut layout = HashMap::new();
    for x in 0..7 {
        for z in 0..3 {
            layout.insert(IVec3::new(x, 0, z), BlockId::OakHull);
            layout.insert(IVec3::new(x, 1, z), BlockId::OakDeck);
        }
    }
    for y in 2..6 {
        layout.insert(IVec3::new(3, y, 1), BlockId::Mast);
    }
    for y in 3..5 {
        for z in [0, 2] {
            layout.insert(IVec3::new(3, y, z), BlockId::Sail);
        }
    }
    for x in [1, 5] {
        for z in [0, 2] {
            layout.insert(IVec3::new(x, 2, z), BlockId::Cannon);
        }
    }
    layout
}

pub fn spawn_player_barge(mut commands: Commands, assets: Res<GameAssets>) {
    spawn_player_barge_inner(&mut commands, &assets);
}

/// WASD steers, Q/E fire the port/starboard broadside.
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

/// Apply helm intent: thrust with water drag, turning authority that scales
/// with speed — a ship dead in the water barely answers the helm.
pub fn drive_ships(
    time: Res<Time>,
    mut ships: Query<(&mut Ship, &Helm, &mut Transform), Without<Sinking>>,
) {
    let dt = time.delta_secs();
    for (mut ship, helm, mut transform) in &mut ships {
        ship.yaw += helm.turn * dt * (0.2 + 0.15 * ship.speed.abs());
        let speed = (ship.speed + helm.thrust * 3.0 * dt) * (1.0 - 0.3 * dt);
        ship.speed = speed.clamp(-2.0, ship.top_speed);
        let forward = Quat::from_rotation_y(ship.yaw) * Vec3::X;
        transform.translation += forward * ship.speed * dt;
    }
}

/// Fake buoyancy until real per-block physics: bob on a sine swell and
/// combine heading with a gentle roll/pitch. Phase varies with position so
/// ships don't bob in lockstep.
pub fn float_ships(time: Res<Time>, mut ships: Query<(&Ship, &mut Transform), Without<Sinking>>) {
    let t = time.elapsed_secs();
    for (ship, mut transform) in &mut ships {
        let phase = transform.translation.x * 0.13 + transform.translation.z * 0.17;
        transform.translation.y = (t * 0.9 + phase).sin() * 0.12;
        let roll = (t * 0.7 + phase).sin() * 0.025;
        let pitch = (t * 0.5 + phase).cos() * 0.015;
        transform.rotation = Quat::from_rotation_y(ship.yaw)
            * Quat::from_rotation_x(roll)
            * Quat::from_rotation_z(pitch);
    }
}

/// After the player's ship has gone down, R launches a fresh barge.
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
    spawn_player_barge_inner(&mut commands, &assets);
    stats.player_sunk = false;
}

fn spawn_player_barge_inner(commands: &mut Commands, assets: &GameAssets) {
    let ship = spawn_ship(
        commands,
        assets,
        barge_layout(),
        Vec3::ZERO,
        0.0,
        PLAYER_RELOAD,
        PLAYER_TOP_SPEED,
    );
    commands.entity(ship).insert(PlayerShip);
}
