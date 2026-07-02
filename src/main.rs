// Bevy system signatures routinely trip clippy's type_complexity and
// too_many_arguments lints; allowing them crate-wide is upstream practice.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

mod assets;
mod audio;
mod blocks;
mod build;
mod combat;
mod dock;
mod enemy;
mod hud;
mod ocean;
mod salvage;
mod selftest;
mod ship;

use bevy::camera::Exposure;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::pbr::{Atmosphere, ScatteringMedium};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use dock::GamePhase;
use ship::{PlayerShip, ShipVoxels};

fn main() {
    let mut app = App::new();
    if std::env::args().any(|arg| arg == "--selftest") {
        app.init_resource::<selftest::SelfTest>();
    }
    let mut stats = combat::GameStats::default();
    let mut waves = dock::WaveDirector::default();
    // Dev shortcut: start at the boss wave with a kitted-out frigate.
    if std::env::args().any(|arg| arg == "--boss") {
        stats.kills = 15;
        stats.tier = 2;
        stats.salvage = 60;
        waves.wave = enemy::BOSS_WAVE;
    }
    app.insert_resource(stats);
    app.insert_resource(waves);
    // Autopilot pacing test: the player ship fights on its own.
    if std::env::args().any(|arg| arg == "--demo") {
        app.init_resource::<enemy::DemoMode>();
    }
    // Log frame-rate diagnostics; per-cube ship rendering is the known perf
    // risk (see spec 002), so keep this handy until greedy meshing lands.
    if std::env::args().any(|arg| arg == "--diag") {
        app.add_plugins((
            bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
            bevy::diagnostic::LogDiagnosticsPlugin::default(),
        ));
    }
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "VoxelPirates".into(),
            ..default()
        }),
        ..default()
    }));
    // Harness runs (and --mute) shouldn't blare cannon fire at whoever is
    // sitting at the machine.
    if std::env::args().any(|arg| arg == "--selftest" || arg == "--demo" || arg == "--mute") {
        app.insert_resource(bevy::audio::GlobalVolume::new(bevy::audio::Volume::Linear(
            0.0,
        )));
    }
    app.add_plugins(ocean::OceanPlugin)
        .insert_resource(ClearColor(Color::srgb(0.55, 0.75, 0.90)))
        .init_state::<GamePhase>()
        .init_resource::<dock::SeaState>()
        .init_resource::<salvage::DerelictDirector>()
        .init_resource::<ocean::Wind>()
        .init_resource::<build::PlayMode>()
        .init_resource::<build::AimOverride>()
        .add_systems(
            Startup,
            (
                assets::setup_assets,
                audio::setup_sounds,
                setup_scene,
                ocean::spawn_ocean,
                dock::spawn_pier,
                build::setup_build,
                hud::setup_hud,
            )
                .chain(),
        )
        // The initial OnEnter(Dock) runs before PreStartup, so enter_dock also
        // owns spawning the player's first ship.
        .add_systems(OnEnter(GamePhase::Dock), dock::enter_dock)
        .add_systems(OnExit(GamePhase::Dock), dock::exit_dock)
        .add_systems(OnEnter(GamePhase::Battle), enemy::spawn_wave)
        .add_systems(
            Update,
            (
                (
                    selftest::run_selftest.run_if(resource_exists::<selftest::SelfTest>),
                    toggle_pause,
                    build::toggle_mode.run_if(in_state(GamePhase::Battle)),
                    ship::player_helm.run_if(in_state(GamePhase::Battle)),
                    ship::player_fire_mouse.run_if(in_state(GamePhase::Battle)),
                    enemy::demo_dock
                        .run_if(resource_exists::<enemy::DemoMode>.and(in_state(GamePhase::Dock))),
                    dock::dock_input.run_if(in_state(GamePhase::Dock)),
                    build::build_input,
                    enemy::demo_pilot.run_if(
                        resource_exists::<enemy::DemoMode>.and(in_state(GamePhase::Battle)),
                    ),
                    enemy::enemy_ai.run_if(in_state(GamePhase::Battle)),
                )
                    .chain(),
                (
                    ocean::update_wind,
                    dock::ease_sea_state,
                    dock::apply_sea_state,
                    ship::drive_ships.run_if(in_state(GamePhase::Battle)),
                    ship::separate_ships.run_if(in_state(GamePhase::Battle)),
                    ship::float_ships,
                )
                    .chain(),
                (
                    combat::fire_cannons,
                    combat::update_cannonballs,
                    combat::sink_ships,
                    combat::update_debris,
                    combat::update_effects,
                )
                    .chain()
                    .run_if(in_state(GamePhase::Battle)),
                (
                    salvage::update_flotsam,
                    salvage::maintain_derelicts,
                    dock::check_battle_over,
                )
                    .chain()
                    .run_if(in_state(GamePhase::Battle)),
                (
                    // After every system that can touch voxels this frame
                    // (combat, building, salvage repair): rebuild changed
                    // ship meshes exactly once.
                    ship::remesh_ships,
                    ocean::follow_player,
                    chase_camera.run_if(in_state(GamePhase::Battle)),
                    dock::dock_camera.run_if(in_state(GamePhase::Dock)),
                    hud::update_hud,
                    hud::update_intel,
                )
                    .chain(),
            )
                .chain(),
        )
        .run();
}

/// P pauses the virtual clock; every gameplay system reads Time, so the
/// whole battle freezes in place.
fn toggle_pause(keys: Res<ButtonInput<KeyCode>>, mut time: ResMut<Time<Virtual>>) {
    if keys.just_pressed(KeyCode::KeyP) {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
}

fn setup_scene(mut commands: Commands, mut mediums: ResMut<Assets<ScatteringMedium>>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-16.0, 9.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        // Physical sky: scattering atmosphere with a real sun (implies an
        // HDR camera), sunlight-calibrated exposure, and bloom so muzzle
        // flashes and stern lanterns actually glow.
        Atmosphere::earthlike(mediums.add(ScatteringMedium::default())),
        Exposure::SUNLIGHT,
        Bloom::NATURAL,
        // Sky fill so faces turned away from the sun (the whole ship, seen
        // from the chase camera) stay readable instead of going black.
        // Daylight-sky levels to match the sunlight exposure.
        AmbientLight {
            color: Color::srgb(0.75, 0.85, 1.0),
            brightness: 15_000.0,
            ..default()
        },
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 100_000.0,
            shadows_enabled: true,
            ..default()
        },
        // Two nearby cascades instead of the default four: the action all
        // happens within ~150 m, and shadow passes are dear on an iGPU.
        bevy::light::CascadeShadowConfigBuilder {
            num_cascades: 2,
            maximum_distance: 150.0,
            ..default()
        }
        .build(),
        // Mid-afternoon sun: high enough for readable shadows, low enough
        // to drag a glitter path across the swell.
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.4, 0.0)),
    ));
}

/// Third-person chase camera: trail the player's ship from behind and above,
/// easing toward the target so turns and wave bob read smoothly. Scroll
/// zooms between close action and a tactical overview.
fn chase_camera(
    time: Res<Time>,
    scroll: Res<AccumulatedMouseScroll>,
    mut zoom: Local<f32>,
    players: Query<(&Transform, &ShipVoxels), (With<PlayerShip>, Without<Camera3d>)>,
    mut cameras: Query<&mut Transform, With<Camera3d>>,
) {
    let Ok((target, voxels)) = players.single() else {
        return;
    };
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    *zoom = (*zoom + scroll.delta.y * 1.5).clamp(-18.0, 8.0);
    // Base the framing on hull size so upgrading to a bigger ship (or the
    // tall-rigged hulls generally) never leaves the camera inside the rig.
    let distance = (voxels.radius * 3.0 - *zoom).max(voxels.radius * 1.5);
    let forward = target.rotation * Vec3::X;
    let forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    let desired = target.translation - forward * distance + Vec3::Y * (distance * 0.6);
    let ease = 1.0 - (-time.delta_secs() * 3.0).exp();
    camera.translation = camera.translation.lerp(desired, ease);
    let look_at = target.translation + forward * (voxels.radius * 0.4) + Vec3::Y * 3.0;
    camera.look_at(look_at, Vec3::Y);
}
