// Bevy system signatures routinely trip clippy's type_complexity lint;
// allowing it crate-wide is the upstream-recommended practice.
#![allow(clippy::type_complexity)]

mod assets;
mod blocks;
mod combat;
mod enemy;
mod hud;
mod ocean;
mod salvage;
mod ship;

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

use ship::PlayerShip;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
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
        .add_systems(
            Startup,
            (
                assets::setup_assets,
                setup_scene,
                ocean::spawn_ocean,
                ship::spawn_player_start,
                hud::setup_hud,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                (ship::player_helm, ship::player_fire_mouse, enemy::enemy_ai).chain(),
                (ship::drive_ships, ship::separate_ships, ship::float_ships).chain(),
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
