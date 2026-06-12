//! Scripted in-game smoke test, run with `voxelpirates --selftest`.
//!
//! Drives the game through its own input resources (no OS-level synthetic
//! input, so it can't race a human using the desktop), saves screenshots to
//! /tmp/selftest_*.png, and logs SELFTEST lines with the values a harness
//! should assert on.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::build::{AimOverride, PlayMode};
use crate::combat::GameStats;
use crate::ship::PlayerShip;

#[derive(Resource, Default)]
pub struct SelfTest {
    elapsed: f32,
    step: usize,
}

pub fn run_selftest(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<SelfTest>,
    mut mode: ResMut<PlayMode>,
    mut aim: ResMut<AimOverride>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    stats: Res<GameStats>,
    players: Query<&Transform, With<PlayerShip>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut exit: MessageWriter<AppExit>,
) {
    state.elapsed += time.delta_secs();
    let t = state.elapsed;
    let Ok(player) = players.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let viewport_of = |world: Vec3| camera.world_to_viewport(camera_transform, world).ok();
    let deck_point = viewport_of(player.translation + Vec3::Y * 1.6);
    let sea_point = viewport_of(player.translation + player.rotation * Vec3::Z * 18.0);

    match state.step {
        0 if t > 1.5 => {
            screenshot(&mut commands, "/tmp/selftest_0_start.png");
            state.step += 1;
        }
        1 if t > 2.0 => {
            *mode = PlayMode::Build;
            aim.0 = deck_point;
            state.step += 1;
        }
        // Scrap one block off the deck.
        2 if t > 2.6 => {
            mouse.press(MouseButton::Right);
            state.step += 1;
        }
        3 => {
            mouse.release(MouseButton::Right);
            state.step += 1;
        }
        4 if t > 3.4 => {
            info!("SELFTEST scrap: salvage = {} (expect 1)", stats.salvage);
            screenshot(&mut commands, "/tmp/selftest_1_scrap.png");
            mouse.press(MouseButton::Left);
            state.step += 1;
        }
        5 => {
            mouse.release(MouseButton::Left);
            state.step += 1;
        }
        6 if t > 4.2 => {
            info!("SELFTEST place: salvage = {} (expect 0)", stats.salvage);
            screenshot(&mut commands, "/tmp/selftest_2_place.png");
            *mode = PlayMode::Sail;
            aim.0 = sea_point;
            state.step += 1;
        }
        // Fire the starboard broadside at open water.
        7 if t > 4.8 => {
            mouse.press(MouseButton::Left);
            state.step += 1;
        }
        8 => {
            mouse.release(MouseButton::Left);
            state.step += 1;
        }
        9 if t > 5.4 => {
            screenshot(&mut commands, "/tmp/selftest_3_fire.png");
            state.step += 1;
        }
        10 if t > 7.5 => {
            info!("SELFTEST complete: kills = {}", stats.kills);
            exit.write(AppExit::Success);
            state.step += 1;
        }
        _ => {}
    }
}

fn screenshot(commands: &mut Commands, path: &'static str) {
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}
