use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::combat::{Broadsides, Sinking};
use crate::ship::{self, Helm, PlayerShip, Ship};

// Enemies reload slower and sail slower than the player: a careful player
// should win a 2-vs-1, and an idle one should still have time to react.
const ENEMY_RELOAD: f32 = 6.0;
const ENEMY_TOP_SPEED: f32 = 5.0;
/// How many hostile ships the director keeps on the water.
const FLEET_SIZE: usize = 2;
const SPAWN_DISTANCE: f32 = 55.0;
/// Maximum range at which the AI bothers firing — just past the ballistic
/// range of a broadside.
const FIRE_RANGE: f32 = 22.0;

#[derive(Component)]
pub struct EnemyAi;

#[derive(Resource, Default)]
pub struct FleetDirector {
    spawned: u32,
}

/// Keep FLEET_SIZE hostiles on the water: whenever one goes down, a new
/// sloop appears over the horizon. Spawn bearings step around a golden-angle
/// sequence so reinforcements don't always come from the same direction.
pub fn maintain_fleet(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut director: ResMut<FleetDirector>,
    enemies: Query<(), (With<EnemyAi>, Without<Sinking>)>,
    players: Query<&Transform, With<PlayerShip>>,
) {
    let alive = enemies.iter().count();
    if alive >= FLEET_SIZE {
        return;
    }
    let Ok(player) = players.single() else {
        return;
    };
    for _ in alive..FLEET_SIZE {
        director.spawned += 1;
        let bearing = director.spawned as f32 * 2.399963; // golden angle
        let offset = Vec3::new(bearing.cos(), 0.0, bearing.sin()) * SPAWN_DISTANCE;
        let position = (player.translation + offset).with_y(0.0);
        let to_player = player.translation - position;
        let yaw = (-to_player.z).atan2(to_player.x);
        let sloop = ship::spawn_ship(
            &mut commands,
            &assets,
            ship::sloop_layout(),
            position,
            yaw,
            ENEMY_RELOAD,
            ENEMY_TOP_SPEED,
        );
        commands.entity(sloop).insert(EnemyAi);
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
        let to_player = player.translation - transform.translation;
        let flat = Vec3::new(to_player.x, 0.0, to_player.z);
        let distance = flat.length();
        let dir = flat / distance.max(0.01);

        // Tangent of the circle around the player; sailing along it keeps
        // the target abeam, which is exactly where the guns point.
        let tangent = Vec3::new(-dir.z, 0.0, dir.x);
        let desired = if distance > 20.0 {
            dir
        } else if distance < 8.0 {
            (tangent - dir).normalize()
        } else {
            (tangent * 0.85 + dir * 0.15).normalize()
        };

        let desired_yaw = (-desired.z).atan2(desired.x);
        let mut yaw_error = desired_yaw - enemy.yaw;
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
}
