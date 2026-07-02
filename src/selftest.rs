//! Scripted in-game smoke test, run with `voxelpirates --selftest`.
//!
//! Drives the whole loop through the game's own input resources (no
//! OS-level synthetic input, so it can't race a human using the desktop):
//! builds at the dock, sets sail, and fires the first broadside of wave 1.
//! Saves screenshots to /tmp/selftest_*.png and logs SELFTEST lines with
//! the values a harness should assert on.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::build::AimOverride;
use crate::combat::GameStats;
use crate::dock::GamePhase;
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
    mut aim: ResMut<AimOverride>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut next_phase: ResMut<NextState<GamePhase>>,
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
    // Aim at the afterdeck, behind the mainmast, so the build ray lands on an
    // OakDeck block (refund 1) rather than snagging the mast or a sail.
    let deck_point = viewport_of(player.translation + player.rotation * Vec3::new(-3.0, 2.1, 0.0));
    let sea_point = viewport_of(player.translation + player.rotation * Vec3::Z * 18.0);

    match state.step {
        // The game opens at the dock, already in build mode.
        0 if t > 1.5 => {
            screenshot(&mut commands, "/tmp/selftest_0_dock.png");
            aim.0 = deck_point;
            state.step += 1;
        }
        // Scrap one block off the deck.
        1 if t > 2.6 => {
            mouse.press(MouseButton::Right);
            state.step += 1;
        }
        2 => {
            mouse.release(MouseButton::Right);
            state.step += 1;
        }
        3 if t > 3.4 => {
            info!("SELFTEST scrap: salvage = {} (expect 1)", stats.salvage);
            screenshot(&mut commands, "/tmp/selftest_1_scrap.png");
            mouse.press(MouseButton::Left);
            state.step += 1;
        }
        4 => {
            mouse.release(MouseButton::Left);
            state.step += 1;
        }
        // Set sail: wave 1 spawns, the swell rises, guns go live.
        5 if t > 4.2 => {
            info!("SELFTEST place: salvage = {} (expect 0)", stats.salvage);
            screenshot(&mut commands, "/tmp/selftest_2_place.png");
            next_phase.set(GamePhase::Battle);
            state.step += 1;
        }
        // Fire the starboard broadside at open water (sea_point is to
        // starboard, so right mouse = starboard guns; see Spec 001).
        6 if t > 5.8 => {
            aim.0 = sea_point;
            mouse.press(MouseButton::Right);
            state.step += 1;
        }
        7 => {
            mouse.release(MouseButton::Right);
            state.step += 1;
        }
        8 if t > 6.6 => {
            screenshot(&mut commands, "/tmp/selftest_3_fire.png");
            state.step += 1;
        }
        9 if t > 9.0 => {
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
