// Bevy system signatures routinely trip clippy's type_complexity and
// too_many_arguments lints; allowing them crate-wide is upstream practice.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

mod assets;
mod audio;
mod blocks;
mod build;
mod combat;
mod enemy;
mod hud;
mod ocean;
mod salvage;
mod selftest;
mod ship;

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;

use ship::PlayerShip;

fn main() {
    let mut app = App::new();
    if std::env::args().any(|arg| arg == "--selftest") {
        app.init_resource::<selftest::SelfTest>();
    }
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "VoxelPirates".into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::srgb(0.55, 0.75, 0.90)))
    .init_resource::<combat::GameStats>()
    .init_resource::<enemy::FleetDirector>()
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
            ship::spawn_player_start,
            build::setup_build,
            hud::setup_hud,
        )
            .chain(),
    )
    .add_systems(
        Update,
        (
            (
                selftest::run_selftest.run_if(resource_exists::<selftest::SelfTest>),
                build::toggle_mode,
                ship::player_helm,
                ship::player_fire_mouse,
                build::build_input,
                enemy::enemy_ai,
            )
                .chain(),
            (
                ocean::update_wind,
                ship::drive_ships,
                ship::separate_ships,
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
                .chain(),
            (
                salvage::update_flotsam,
                salvage::upgrade_player,
                salvage::maintain_derelicts,
            )
                .chain(),
            (
                enemy::maintain_fleet,
                ship::respawn_player,
                ocean::follow_player,
                chase_camera,
                hud::update_hud,
                hud::update_intel,
            )
                .chain(),
        )
            .chain(),
    )
    .run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-16.0, 9.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        // Haze the horizon into the sky so the finite ocean plane never
        // shows an edge.
        DistanceFog {
            color: Color::srgb(0.55, 0.75, 0.90),
            falloff: FogFalloff::Linear {
                start: 90.0,
                end: 220.0,
            },
            ..default()
        },
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.4, 0.0)),
    ));
}

/// Third-person chase camera: trail the player's ship from behind and above,
/// easing toward the target so turns and wave bob read smoothly. Scroll
/// zooms between close action and a tactical overview.
fn chase_camera(
    time: Res<Time>,
    scroll: Res<AccumulatedMouseScroll>,
    mut zoom: Local<f32>,
    players: Query<&Transform, (With<PlayerShip>, Without<Camera3d>)>,
    mut cameras: Query<&mut Transform, With<Camera3d>>,
) {
    let Ok(target) = players.single() else {
        return;
    };
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    *zoom = (*zoom + scroll.delta.y * 1.5).clamp(-18.0, 6.0);
    let distance = 16.0 - *zoom; // 10 (close) .. 34 (overview)
    let forward = target.rotation * Vec3::X;
    let forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    let desired = target.translation - forward * distance + Vec3::Y * (distance * 0.5);
    let ease = 1.0 - (-time.delta_secs() * 3.0).exp();
    camera.translation = camera.translation.lerp(desired, ease);
    let look_at = target.translation + forward * 6.0 + Vec3::Y;
    camera.look_at(look_at, Vec3::Y);
}
