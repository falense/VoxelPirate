mod blocks;
mod ocean;
mod ship;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "CraftPirate".into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.75, 0.90)))
        .add_systems(
            Startup,
            (setup_scene, ocean::spawn_ocean, ship::spawn_starter_barge),
        )
        .add_systems(Update, (ship::sail_ship, ship::float_ships).chain())
        .run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(14.0, 10.0, 18.0).looking_at(Vec3::ZERO, Vec3::Y),
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
