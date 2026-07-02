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

/// Hostile classes. Enemies reload slower and sail slower than the player's
/// equivalent hull: a careful player wins outnumbered fights.
#[derive(Clone, Copy)]
struct EnemyClass {
    layout: fn() -> HashMap<IVec3, BlockId>,
    reload: f32,
    top_speed: f32,
    boss: bool,
}

const SLOOP: EnemyClass = EnemyClass {
    layout: ship::sloop_layout,
    reload: 6.0,
    top_speed: 5.0,
    boss: false,
};
const BRIG: EnemyClass = EnemyClass {
    layout: ship::brig_layout,
    reload: 5.5,
    top_speed: 5.4,
    boss: false,
};
const FRIGATE: EnemyClass = EnemyClass {
    layout: ship::frigate_layout,
    reload: 5.0,
    top_speed: 5.8,
    boss: false,
};
const DREADNOUGHT: EnemyClass = EnemyClass {
    layout: ship::dreadnought_layout,
    reload: 5.0,
    top_speed: 4.6,
    boss: true,
};

/// The Dreadnought's wave; sink it there and the campaign is won, but the
/// seas keep escalating for as long as you keep sailing out.
pub const BOSS_WAVE: u32 = 8;

/// What sails out against the player in a given wave. Waves 1-8 are a
/// hand-tuned ramp to the Dreadnought; past that a strength budget keeps
/// growing, with another Dreadnought joining every eighth wave.
fn wave_composition(wave: u32) -> Vec<EnemyClass> {
    match wave {
        1 => vec![SLOOP, SLOOP],
        2 => vec![SLOOP, SLOOP, SLOOP],
        3 => vec![BRIG, SLOOP, SLOOP],
        4 => vec![BRIG, BRIG, SLOOP],
        5 => vec![BRIG, BRIG, BRIG],
        6 => vec![FRIGATE, BRIG, BRIG],
        7 => vec![FRIGATE, FRIGATE, BRIG],
        8 => vec![DREADNOUGHT, FRIGATE],
        wave => {
            let mut fleet = Vec::new();
            let mut budget = 6 + (wave - 8) as i32;
            if wave.is_multiple_of(8) {
                fleet.push(DREADNOUGHT);
                budget -= 6;
            }
            while budget > 0 && fleet.len() < 5 {
                let class = match budget {
                    1 => SLOOP,
                    2 => BRIG,
                    _ => FRIGATE,
                };
                budget -= match budget {
                    1 => 1,
                    2 => 2,
                    _ => 3,
                };
                fleet.push(class);
            }
            fleet
        }
    }
}

/// Entering battle: the whole wave sails over the horizon at once, spread
/// around the compass on golden-angle bearings so it can't be memorized.
pub fn spawn_wave(
    mut commands: Commands,
    mut stats: ResMut<GameStats>,
    mut director: ResMut<crate::dock::WaveDirector>,
    players: Query<&Transform, With<PlayerShip>>,
) {
    director.battle_time = 0.0;
    info!("WAVE {} spawning", director.wave);
    let Ok(player) = players.single() else {
        return;
    };
    let fleet = wave_composition(director.wave);
    let count = fleet.len();
    for (i, class) in fleet.into_iter().enumerate() {
        let bearing = (director.wave * 3 + i as u32) as f32 * 2.399963; // golden angle
        let distance = SPAWN_DISTANCE + if class.boss { 20.0 } else { 0.0 };
        let offset = Vec3::new(bearing.cos(), 0.0, bearing.sin()) * distance;
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
        if class.boss {
            commands.entity(hostile).insert(Dreadnought);
        }
    }
    if director.wave == BOSS_WAVE {
        stats.announce("The DREADNOUGHT has come for you. Sink it and the seas are yours!");
    } else {
        stats.announce(format!(
            "Wave {}: {count} hostile sails on the horizon!",
            director.wave
        ));
    }
}

/// `--demo` flag: the player's ship sails itself with the same broadside
/// AI, for end-to-end pacing tests (kills, salvage, upgrades, boss).
#[derive(Resource, Default)]
pub struct DemoMode;

pub fn demo_pilot(
    targets: Query<&Transform, (With<EnemyAi>, Without<Sinking>, Without<PlayerShip>)>,
    mut players: Query<
        (&Transform, &Ship, &mut Helm, &mut Broadsides),
        (With<PlayerShip>, Without<Sinking>, Without<EnemyAi>),
    >,
) {
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

/// `--demo` at the dock: repair, buy any affordable hull, and set sail —
/// by pressing the dock's own keys, so the autopilot exercises exactly the
/// player-facing flow.
pub fn demo_dock(time: Res<Time>, mut delay: Local<f32>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    let before = *delay;
    *delay += time.delta_secs();
    let crossed = |mark: f32| before < mark && *delay >= mark;
    if crossed(0.5) {
        keys.press(KeyCode::KeyR);
    }
    if crossed(1.0) {
        keys.release(KeyCode::KeyR);
        keys.press(KeyCode::KeyU);
    }
    if crossed(1.5) {
        keys.release(KeyCode::KeyU);
        keys.press(KeyCode::Enter);
    }
    if crossed(2.0) {
        keys.release(KeyCode::Enter);
        *delay = 0.0;
    }
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
