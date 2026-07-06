//! The dock interlude: between waves the game returns to a sheltered cove
//! where the player repairs, rebuilds, and upgrades their ship in peace,
//! then sets sail into the next, harder wave (Spec 006).

use bevy::prelude::*;

use crate::combat::{CannonBall, Debris, Effect, GameStats, Sinking};
use crate::salvage::{Derelict, Flotsam};
use crate::ship::{PLAYER_CLASSES, PlayerShip, Ship, ShipVoxels, UPGRADE_COSTS};

/// The two halves of the game loop. Combat systems run in `Battle`; the
/// dock is a safe builder phase with a free camera and cold guns.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GamePhase {
    #[default]
    Dock,
    Battle,
}

/// Where the cove is. The pier is fixed scenery here and the player's ship
/// is towed back to this anchorage between waves.
pub const DOCK_ANCHOR: Vec3 = Vec3::ZERO;

/// Wave progression. `wave` is the next wave to fight (1-based); death
/// retries the same wave, victory advances it.
#[derive(Resource)]
pub struct WaveDirector {
    pub wave: u32,
    /// Seconds since the current battle started, gating the cleared check
    /// so it can't fire before the wave has actually spawned.
    pub battle_time: f32,
}

impl Default for WaveDirector {
    fn default() -> Self {
        Self {
            wave: 1,
            battle_time: 0.0,
        }
    }
}

/// Sea-state factor over the base swell table: ~0.15 is the sheltered
/// cove, 1.0 the open fight. Eases toward `target` so the water calms and
/// rises smoothly across phase changes; both the CPU wave sampling and the
/// GPU ocean material read `current`.
#[derive(Resource)]
pub struct SeaState {
    pub current: f32,
    pub target: f32,
}

pub const SEA_CALM: f32 = 0.15;
pub const SEA_OPEN: f32 = 1.0;

impl Default for SeaState {
    fn default() -> Self {
        Self {
            current: SEA_CALM,
            target: SEA_CALM,
        }
    }
}

pub fn ease_sea_state(time: Res<Time>, mut sea: ResMut<SeaState>) {
    let step = 1.0 - (-time.delta_secs() * 0.8).exp();
    // Only dirty the resource (and thus the GPU uniforms) while moving.
    if (sea.current - sea.target).abs() > 0.001 {
        sea.current += (sea.target - sea.current) * step;
    }
}

/// Push the eased sea state into the ocean material's swell amplitudes.
pub fn apply_sea_state(
    sea: Res<SeaState>,
    mut materials: ResMut<Assets<crate::ocean::OceanMaterial>>,
    oceans: Query<&MeshMaterial3d<crate::ocean::OceanMaterial>, With<crate::ocean::Ocean>>,
) {
    if !sea.is_changed() {
        return;
    }
    for handle in &oceans {
        if let Some(material) = materials.get_mut(&handle.0) {
            material.extension.set_amplitude_scale(sea.current);
        }
    }
}

/// The pier: fixed scenery built from ordinary blocks as a static voxel
/// grid — the ship renderer draws the scenery too. A plank walkway on
/// tarred posts, lantern poles, and a low quay platform at the shore end.
pub fn spawn_pier(mut commands: Commands) {
    use crate::blocks::BlockId;
    let mut blocks = std::collections::HashMap::new();
    // Walkway: 16 long, 2 wide, deck one level above the waterline.
    for x in 0..16 {
        for z in 0..2 {
            blocks.insert(IVec3::new(x, 1, z), BlockId::OakDeck);
        }
        if x % 3 == 0 {
            blocks.insert(IVec3::new(x, 0, 0), BlockId::Trim);
            blocks.insert(IVec3::new(x, 0, 1), BlockId::Trim);
        }
    }
    // Lantern poles along the outer edge.
    for x in [1, 7, 13] {
        blocks.insert(IVec3::new(x, 2, 0), BlockId::Mast);
        blocks.insert(IVec3::new(x, 3, 0), BlockId::Lantern);
    }
    // Quay platform at the shore end with a crate of plunder.
    for x in 13..16 {
        for z in -2..4 {
            blocks.insert(IVec3::new(x, 1, z), BlockId::OakDeck);
            if x == 14 && (z == -2 || z == 3) {
                blocks.insert(IVec3::new(x, 0, z), BlockId::Trim);
            }
        }
    }
    blocks.insert(IVec3::new(14, 2, -1), BlockId::Gold);
    blocks.insert(IVec3::new(14, 2, 2), BlockId::OakHull);

    let (mut min, mut max) = (IVec3::MAX, IVec3::MIN);
    for pos in blocks.keys() {
        min = min.min(*pos);
        max = max.max(*pos);
    }
    let center = Vec3::new(
        (min.x + max.x + 1) as f32 * 0.5,
        1.0,
        (min.z + max.z + 1) as f32 * 0.5,
    );
    commands.spawn((
        ShipVoxels {
            blocks: blocks.clone(),
            plan: blocks,
            center,
            radius: 0.0,
        },
        Transform::from_translation(DOCK_ANCHOR + Vec3::new(2.0, 0.35, -9.0)),
        Visibility::default(),
    ));
}

/// Entering the dock: clear the battlefield, tow the player home, calm the
/// water, and hand the mouse to the builder.
pub fn enter_dock(
    mut commands: Commands,
    stats: Res<GameStats>,
    mut sea: ResMut<SeaState>,
    mut mode: ResMut<crate::build::PlayMode>,
    leftovers: Query<
        Entity,
        Or<(
            With<CannonBall>,
            With<Debris>,
            With<Effect>,
            With<Flotsam>,
            With<Derelict>,
        )>,
    >,
    enemies: Query<Entity, (With<Ship>, Without<PlayerShip>)>,
    mut players: Query<(&mut Ship, &mut crate::ship::Helm, &mut Transform), With<PlayerShip>>,
) {
    for entity in leftovers.iter().chain(enemies.iter()) {
        commands.entity(entity).despawn();
    }
    sea.target = SEA_CALM;
    *mode = crate::build::PlayMode::Build;
    if let Ok((mut ship, mut helm, mut transform)) = players.single_mut() {
        ship.speed = 0.0;
        helm.thrust = 0.0;
        helm.turn = 0.0;
        transform.translation = DOCK_ANCHOR;
        ship.yaw = 0.0;
    } else {
        // The last ship went down: launch a fresh hull of the same class.
        crate::ship::spawn_player(&mut commands, stats.tier, DOCK_ANCHOR, 0.0);
    }
}

/// Leaving the dock: swell up, mouse back to the guns.
pub fn exit_dock(mut sea: ResMut<SeaState>, mut mode: ResMut<crate::build::PlayMode>) {
    sea.target = SEA_OPEN;
    *mode = crate::build::PlayMode::Sail;
}

/// Dock actions besides building: Enter sets sail, R repairs the plan
/// block-by-block while salvage lasts, U buys the next hull class.
pub fn dock_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    sounds: Res<crate::audio::SoundBank>,
    mut stats: ResMut<GameStats>,
    mut next_phase: ResMut<NextState<GamePhase>>,
    mut players: Query<(Entity, &mut ShipVoxels, &Ship), With<PlayerShip>>,
) {
    if keys.just_pressed(KeyCode::Enter) {
        next_phase.set(GamePhase::Battle);
        return;
    }
    let Ok((entity, mut voxels, ship)) = players.single_mut() else {
        return;
    };

    if keys.just_pressed(KeyCode::KeyR) {
        let repaired = repair_ship(&mut stats, &mut voxels);
        if repaired > 0 {
            stats.announce(format!("Shipwrights fitted {repaired} blocks."));
            crate::audio::play(&mut commands, &sounds.ding, 0.5);
        } else {
            stats.announce("Nothing to repair (or no salvage left).");
        }
    }

    if keys.just_pressed(KeyCode::KeyU) {
        if stats.tier + 1 >= PLAYER_CLASSES.len() {
            stats.announce("No grander hull exists: this is the pride of the seas.");
            return;
        }
        let cost = UPGRADE_COSTS[stats.tier];
        if stats.salvage < cost {
            let next_name = PLAYER_CLASSES[stats.tier + 1].name;
            stats.announce(format!("The {next_name} costs {cost} salvage."));
            return;
        }
        // Refund whatever the player's ship is worth beyond its stock
        // class, so custom guns and armor carry their value forward.
        let stock: u32 = (PLAYER_CLASSES[stats.tier].layout)()
            .values()
            .map(|id| crate::blocks::def(*id).cost)
            .sum();
        let actual: u32 = voxels
            .blocks
            .values()
            .map(|id| crate::blocks::def(*id).cost)
            .sum();
        let refund = actual.saturating_sub(stock);
        stats.salvage = stats.salvage - cost + refund;
        stats.tier += 1;
        let yaw = ship.yaw;
        commands.entity(entity).despawn();
        crate::ship::spawn_player(&mut commands, stats.tier, DOCK_ANCHOR, yaw);
        let message = if refund > 0 {
            format!(
                "{} launched! ({refund} salvage refunded for custom fittings)",
                PLAYER_CLASSES[stats.tier].name
            )
        } else {
            format!("{} launched!", PLAYER_CLASSES[stats.tier].name)
        };
        stats.announce(message);
        crate::audio::play(&mut commands, &sounds.fanfare, 0.6);
    }
}

/// Refit missing plan blocks (hull first, lowest cells up) while salvage
/// lasts. Dock shipwrights work at half the block's registry cost, so
/// battle damage doesn't eat the upgrade fund. Returns how many blocks
/// were fitted.
fn repair_ship(stats: &mut GameStats, voxels: &mut ShipVoxels) -> u32 {
    let mut missing: Vec<(IVec3, crate::blocks::BlockId)> = voxels
        .plan
        .iter()
        .filter(|(cell, _)| !voxels.blocks.contains_key(*cell))
        .map(|(cell, id)| (*cell, *id))
        .collect();
    missing.sort_by_key(|(cell, _)| (cell.y, cell.x, cell.z));
    let mut repaired = 0;
    for (cell, id) in missing {
        let cost = crate::blocks::def(id).cost.div_ceil(2);
        if stats.salvage < cost {
            break;
        }
        stats.salvage -= cost;
        voxels.blocks.insert(cell, id);
        repaired += 1;
    }
    repaired
}

/// Free orbit camera for the builder: A/D swing around the ship, W/S raise
/// and lower the eye, scroll zooms. No sailing at the dock, so WASD is free.
pub fn dock_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut state: Local<Option<(f32, f32, f32)>>,
    players: Query<(&Transform, &ShipVoxels), (With<PlayerShip>, Without<Camera3d>)>,
    mut cameras: Query<&mut Transform, With<Camera3d>>,
) {
    let Ok((target, voxels)) = players.single() else {
        return;
    };
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    let (mut yaw, mut pitch, mut distance) =
        state.unwrap_or((2.6, 0.45, voxels.radius * 2.5 + 6.0));
    let dt = time.delta_secs();
    if keys.pressed(KeyCode::KeyA) {
        yaw -= dt * 1.2;
    }
    if keys.pressed(KeyCode::KeyD) {
        yaw += dt * 1.2;
    }
    if keys.pressed(KeyCode::KeyW) {
        pitch += dt * 0.8;
    }
    if keys.pressed(KeyCode::KeyS) {
        pitch -= dt * 0.8;
    }
    pitch = pitch.clamp(0.08, 1.35);
    distance = (distance - scroll.delta.y * 2.0).clamp(voxels.radius + 3.0, 60.0);
    *state = Some((yaw, pitch, distance));

    let offset = Vec3::new(
        yaw.cos() * pitch.cos(),
        pitch.sin(),
        yaw.sin() * pitch.cos(),
    ) * distance;
    let ease = 1.0 - (-dt * 5.0).exp();
    let focus = target.translation + Vec3::Y * 2.0;
    camera.translation = camera.translation.lerp(focus + offset, ease);
    camera.look_at(focus, Vec3::Y);
}

/// Battle ends one of two ways: the wave is swept (bank the flotsam, sail
/// home victorious) or the player's ship is gone (towed home at a price).
pub fn check_battle_over(
    mut commands: Commands,
    time: Res<Time>,
    mut stats: ResMut<GameStats>,
    mut director: ResMut<WaveDirector>,
    mut next_phase: ResMut<NextState<GamePhase>>,
    enemies: Query<(), (With<crate::enemy::EnemyAi>, Without<Sinking>)>,
    players: Query<(), With<PlayerShip>>,
    flotsam: Query<(Entity, &Flotsam)>,
) {
    director.battle_time += time.delta_secs();
    // Grace: the wave spawns via commands on the enter-battle frame.
    if director.battle_time < 2.0 {
        return;
    }

    if players.is_empty() {
        let penalty = stats.salvage / 3;
        stats.salvage -= penalty;
        stats.player_sunk = false;
        stats.announce(format!(
            "Fished out of the drink... the tow home cost {penalty} salvage."
        ));
        info!("WAVE {} lost; back to the dock", director.wave);
        next_phase.set(GamePhase::Dock);
        return;
    }

    if enemies.is_empty() {
        let mut swept = 0;
        for (entity, piece) in &flotsam {
            swept += crate::blocks::def(piece.id).cost;
            commands.entity(entity).despawn();
        }
        stats.salvage += swept;
        stats.announce(format!(
            "Wave {} cleared! Crews swept {swept} salvage aboard.",
            director.wave
        ));
        info!("WAVE {} cleared (salvage {})", director.wave, stats.salvage);
        director.wave += 1;
        next_phase.set(GamePhase::Dock);
    }
}
