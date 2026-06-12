use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::blocks::BlockId;
use crate::combat::{GameStats, Sinking};
use crate::ship::{self, PLAYER_CLASSES, PlayerShip, Ship, ShipVoxels, UPGRADE_COSTS, Voxel};

/// How close (flat distance) a piece must drift before it's collected.
const COLLECT_RANGE: f32 = 3.0;
/// Inside this range flotsam is pulled toward the player's ship.
const MAGNET_RANGE: f32 = 9.0;
const MAGNET_PULL: f32 = 7.0;

/// A block bobbing on the sea after a ship went down. Sail over it to
/// collect: it repairs battle damage first, then banks as salvage.
#[derive(Component)]
pub struct Flotsam {
    age: f32,
    phase: f32,
}

/// An abandoned, drifting wreck. It doesn't fight back — blast it apart for
/// risk-free salvage.
#[derive(Component)]
pub struct Derelict;

#[derive(Resource, Default)]
pub struct DerelictDirector {
    spawned: u32,
}

/// How many derelicts the director keeps drifting within reach of the player.
const DERELICT_COUNT: usize = 2;
const DERELICT_SPAWN_DISTANCE: f32 = 70.0;
/// Derelicts farther than this are despawned and replaced nearer by.
const DERELICT_LEASH: f32 = 220.0;

pub fn spawn_flotsam(commands: &mut Commands, assets: &GameAssets, id: BlockId, position: Vec3) {
    commands.spawn((
        Flotsam {
            age: 0.0,
            phase: position.x * 3.1 + position.z * 1.7,
        },
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.block_materials[&id].clone()),
        Transform::from_translation(position).with_scale(Vec3::splat(0.6)),
    ));
}

/// Bob flotsam on the swell, drift it toward a nearby player, and collect
/// pieces that come alongside.
pub fn update_flotsam(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    mut stats: ResMut<GameStats>,
    mut flotsam: Query<(Entity, &mut Flotsam, &mut Transform), Without<Ship>>,
    mut players: Query<
        (Entity, &Transform, &mut ShipVoxels),
        (With<PlayerShip>, Without<Sinking>, Without<Flotsam>),
    >,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let mut player = players.single_mut().ok();

    for (entity, mut piece, mut transform) in &mut flotsam {
        piece.age += dt;
        if piece.age > 120.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation.y = 0.15 + (t * 1.2 + piece.phase).sin() * 0.08;
        transform.rotate_y(0.4 * dt);

        let Some((ship_entity, ship_transform, voxels)) = player.as_mut() else {
            continue;
        };
        let mut to_ship = ship_transform.translation - transform.translation;
        to_ship.y = 0.0;
        let distance = to_ship.length();
        if distance < MAGNET_RANGE && distance > 0.1 {
            transform.translation += to_ship / distance * (MAGNET_PULL * dt);
        }
        if distance < COLLECT_RANGE {
            collect(&mut commands, &assets, &mut stats, *ship_entity, voxels);
            commands.entity(entity).despawn();
        }
    }
}

/// One collected piece repairs the lowest missing cell of the ship's plan
/// (hull before superstructure); with nothing to repair it banks as salvage.
fn collect(
    commands: &mut Commands,
    assets: &GameAssets,
    stats: &mut GameStats,
    ship_entity: Entity,
    voxels: &mut ShipVoxels,
) {
    let missing = voxels
        .plan
        .iter()
        .filter(|(cell, _)| !voxels.blocks.contains_key(*cell))
        .min_by_key(|(cell, _)| (cell.y, cell.x, cell.z))
        .map(|(cell, id)| (*cell, *id));

    if let Some((cell, id)) = missing {
        let child = commands
            .spawn((
                Mesh3d(assets.cube.clone()),
                MeshMaterial3d(assets.block_materials[&id].clone()),
                Transform::from_translation(voxels.local_offset(cell)),
                ChildOf(ship_entity),
            ))
            .id();
        voxels.blocks.insert(cell, Voxel { id, entity: child });
    } else {
        stats.salvage += 1;
    }
}

/// Keep DERELICT_COUNT wrecks drifting near the player as a peaceful
/// salvage source; despawn ones left far behind.
pub fn maintain_derelicts(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut director: ResMut<DerelictDirector>,
    derelicts: Query<(Entity, &Transform), (With<Derelict>, Without<Sinking>)>,
    players: Query<&Transform, With<PlayerShip>>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    let mut nearby = 0;
    for (entity, transform) in &derelicts {
        if transform.translation.distance(player.translation) > DERELICT_LEASH {
            commands.entity(entity).despawn();
        } else {
            nearby += 1;
        }
    }
    for _ in nearby..DERELICT_COUNT {
        director.spawned += 1;
        let bearing = director.spawned as f32 * 2.399963 + 1.2;
        let offset = Vec3::new(bearing.cos(), 0.0, bearing.sin()) * DERELICT_SPAWN_DISTANCE;
        let layouts = [ship::sloop_layout, ship::brig_layout, ship::frigate_layout];
        let layout = layouts[director.spawned as usize % layouts.len()]();
        let wreck = ship::spawn_ship(
            &mut commands,
            &assets,
            weathered(layout, director.spawned as f32 * 17.3),
            (player.translation + offset).with_y(0.0),
            bearing * 1.9,
            f32::INFINITY,
            0.0,
        );
        commands.entity(wreck).insert(Derelict);
    }
}

/// Knock a deterministic ~third of the superstructure off a layout so wrecks
/// look battle-worn; the hull layer stays intact so they sit right.
fn weathered(
    mut layout: std::collections::HashMap<IVec3, crate::blocks::BlockId>,
    seed: f32,
) -> std::collections::HashMap<IVec3, crate::blocks::BlockId> {
    layout.retain(|cell, _| {
        if cell.y == 0 {
            return true;
        }
        let h = ((cell.as_vec3() + Vec3::splat(seed))
            .dot(Vec3::new(127.1, 311.7, 74.7))
            .sin()
            * 43758.547)
            .fract()
            .abs();
        h > 0.35
    });
    layout
}

/// Spend banked salvage on the next hull class as soon as it's affordable.
/// The new ship launches where the old one sailed, fully repaired.
pub fn upgrade_player(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut stats: ResMut<GameStats>,
    players: Query<(Entity, &Transform, &Ship), (With<PlayerShip>, Without<Sinking>)>,
) {
    if stats.tier >= UPGRADE_COSTS.len() {
        return;
    }
    let cost = UPGRADE_COSTS[stats.tier];
    if stats.salvage < cost {
        return;
    }
    let Ok((entity, transform, old_ship)) = players.single() else {
        return;
    };
    stats.salvage -= cost;
    stats.tier += 1;
    commands.entity(entity).despawn();
    ship::spawn_player(
        &mut commands,
        &assets,
        stats.tier,
        transform.translation.with_y(0.0),
        old_ship.yaw,
    );
    let message = format!(
        "Salvage spent — {} launched!",
        PLAYER_CLASSES[stats.tier].name
    );
    stats.announce(message);
}
