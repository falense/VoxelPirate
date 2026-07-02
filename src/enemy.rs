use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::blocks::BlockId;
use crate::combat::{Broadsides, GameStats, Sinking};
use crate::ship::{self, Helm, PlayerShip, Ship};

const SPAWN_DISTANCE: f32 = 55.0;
/// Maximum range at which the AI bothers firing — just past the ballistic
/// range of a broadside.
const FIRE_RANGE: f32 = 22.0;

#[derive(Component)]
pub struct EnemyAi;

/// The boss ship; sinking it is the campaign's victory condition.
#[derive(Component)]
pub struct Dreadnought;

/// Player kill count that summons the Dreadnought.
const BOSS_AT_KILLS: u32 = 15;

#[derive(Resource, Default)]
pub struct FleetDirector {
    spawned: u32,
    /// Highest class index already announced, so each new hostile type gets
    /// one "sighted!" call-out.
    announced: usize,
    boss_spawned: bool,
}

/// Hostile classes. Enemies reload slower and sail slower than the player's
/// equivalent hull: a careful player wins outnumbered fights.
struct EnemyClass {
    name: &'static str,
    layout: fn() -> HashMap<IVec3, BlockId>,
    reload: f32,
    top_speed: f32,
    /// Player kill count at which this class starts appearing.
    unlock_kills: u32,
}

const ENEMY_CLASSES: [EnemyClass; 3] = [
    EnemyClass {
        name: "sloop",
        layout: ship::sloop_layout,
        reload: 6.0,
        top_speed: 5.0,
        unlock_kills: 0,
    },
    EnemyClass {
        name: "brig",
        layout: ship::brig_layout,
        reload: 5.5,
        top_speed: 5.4,
        unlock_kills: 4,
    },
    EnemyClass {
        name: "frigate",
        layout: ship::frigate_layout,
        reload: 5.0,
        top_speed: 5.8,
        unlock_kills: 10,
    },
];

/// Keep the hostile fleet topped up: it grows from two to four ships as the
/// player racks up kills, and tougher classes join the rotation. Spawn
/// bearings step around a golden-angle sequence so reinforcements don't
/// always come from the same direction.
pub fn maintain_fleet(
    mut commands: Commands,
    time: Res<Time>,
    mut stats: ResMut<GameStats>,
    mut director: ResMut<FleetDirector>,
    enemies: Query<(), (With<EnemyAi>, Without<Sinking>)>,
    players: Query<&Transform, With<PlayerShip>>,
) {
    // A short grace period after launch: let the player find the helm
    // before the first hostiles appear over the horizon.
    if time.elapsed_secs() < 12.0 {
        return;
    }
    let Ok(player) = players.single() else {
        return;
    };

    // The hunt's climax: one Dreadnought, summoned at BOSS_AT_KILLS.
    if stats.kills >= BOSS_AT_KILLS && !director.boss_spawned && !stats.victory {
        director.boss_spawned = true;
        director.spawned += 1;
        let bearing = director.spawned as f32 * 2.399963;
        let offset = Vec3::new(bearing.cos(), 0.0, bearing.sin()) * (SPAWN_DISTANCE + 20.0);
        let position = (player.translation + offset).with_y(0.0);
        let to_player = player.translation - position;
        let boss = ship::spawn_ship(
            &mut commands,
            ship::dreadnought_layout(),
            position,
            (-to_player.z).atan2(to_player.x),
            5.0,
            4.6,
        );
        commands.entity(boss).insert((EnemyAi, Dreadnought));
        stats.announce("The DREADNOUGHT has come for you. Sink it and the seas are yours!");
    }

    let target_fleet = (2 + stats.kills as usize / 5).min(4);
    let alive = enemies.iter().count();
    if alive >= target_fleet {
        return;
    }
    for _ in alive..target_fleet {
        director.spawned += 1;
        let unlocked: Vec<(usize, &EnemyClass)> = ENEMY_CLASSES
            .iter()
            .enumerate()
            .filter(|(_, class)| stats.kills >= class.unlock_kills)
            .collect();
        let (class_index, class) = unlocked[director.spawned as usize % unlocked.len()];
        if class_index > director.announced {
            director.announced = class_index;
            stats.announce(format!("A hostile {} has been sighted!", class.name));
        }

        let bearing = director.spawned as f32 * 2.399963; // golden angle
        let offset = Vec3::new(bearing.cos(), 0.0, bearing.sin()) * SPAWN_DISTANCE;
        let position = (player.translation + offset).with_y(0.0);
        let to_player = player.translation - position;
        let yaw = (-to_player.z).atan2(to_player.x);
        let hostile = ship::spawn_ship(
            &mut commands,
            (class.layout)(),
            position,
            yaw,
            class.reload,
            class.top_speed,
        );
        commands.entity(hostile).insert(EnemyAi);
    }
}

/// `--demo` flag: the player's ship sails itself with the same broadside
/// AI, for end-to-end pacing tests (kills, salvage, upgrades, boss).
#[derive(Resource, Default)]
pub struct DemoMode;

pub fn demo_pilot(
    mut commands: Commands,
    mut stats: ResMut<GameStats>,
    targets: Query<&Transform, (With<EnemyAi>, Without<Sinking>, Without<PlayerShip>)>,
    mut players: Query<
        (&Transform, &Ship, &mut Helm, &mut Broadsides),
        (With<PlayerShip>, Without<Sinking>, Without<EnemyAi>),
    >,
    any_player: Query<(), With<PlayerShip>>,
) {
    // Auto-relaunch after going down, like a player pressing R.
    if any_player.is_empty() {
        ship::spawn_player(&mut commands, stats.tier, Vec3::ZERO, 0.0);
        stats.player_sunk = false;
        return;
    }
    let Ok((transform, player, mut helm, mut guns)) = players.single_mut() else {
        return;
    };
    let nearest = targets.iter().min_by(|a, b| {
        let da = a.translation.distance_squared(transform.translation);
        let db = b.translation.distance_squared(transform.translation);
        da.total_cmp(&db)
    });
    let Some(target) = nearest else {
        helm.thrust = 0.5;
        helm.turn = 0.1;
        return;
    };
    steer_broadside(
        transform,
        player.yaw,
        target.translation,
        &mut helm,
        &mut guns,
    );
}

/// Shared broadside-circling brain: close to gun range, hold the target
/// abeam, and fire the facing side.
fn steer_broadside(
    transform: &Transform,
    yaw: f32,
    target: Vec3,
    helm: &mut Helm,
    guns: &mut Broadsides,
) {
    let to_target = target - transform.translation;
    let flat = Vec3::new(to_target.x, 0.0, to_target.z);
    let distance = flat.length();
    let dir = flat / distance.max(0.01);

    let tangent = Vec3::new(-dir.z, 0.0, dir.x);
    let desired = if distance > 20.0 {
        dir
    } else if distance < 8.0 {
        (tangent - dir).normalize()
    } else {
        (tangent * 0.85 + dir * 0.15).normalize()
    };

    let desired_yaw = (-desired.z).atan2(desired.x);
    let mut yaw_error = desired_yaw - yaw;
    yaw_error = (yaw_error + PI).rem_euclid(TAU) - PI;
    helm.turn = (yaw_error * 2.0).clamp(-1.0, 1.0);
    helm.thrust = if distance > 14.0 { 1.0 } else { 0.55 };

    if distance < FIRE_RANGE {
        let starboard = transform.rotation * Vec3::Z;
        let bearing = starboard.dot(dir);
        if bearing > 0.92 {
            guns.fire_starboard = true;
        } else if bearing < -0.92 {
            guns.fire_port = true;
        }
    }
}

/// Simple broadside AI: close to gun range, then circle the player so the
/// broadside naturally bears, and fire whichever side faces them.
pub fn enemy_ai(
    players: Query<&Transform, (With<PlayerShip>, Without<EnemyAi>, Without<Sinking>)>,
    mut enemies: Query<
        (&Transform, &Ship, &mut Helm, &mut Broadsides),
        (With<EnemyAi>, Without<Sinking>),
    >,
) {
    let Ok(player) = players.single() else {
        // No target: idle in lazy circles.
        for (_, _, mut helm, _) in &mut enemies {
            helm.thrust = 0.3;
            helm.turn = 0.2;
        }
        return;
    };

    for (transform, enemy, mut helm, mut guns) in &mut enemies {
        steer_broadside(
            transform,
            enemy.yaw,
            player.translation,
            &mut helm,
            &mut guns,
        );
    }
}
