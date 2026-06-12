use bevy::prelude::*;

use crate::ship::PlayerShip;

#[derive(Component)]
pub struct Ocean;

/// The waterline sits at y = 0; the ocean plane is nudged slightly below so
/// hull blocks at the waterline don't z-fight with it.
pub fn spawn_ocean(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Ocean,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(600.0, 600.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.04, 0.22, 0.40),
            perceptual_roughness: 0.25,
            metallic: 0.1,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.05, 0.0),
    ));
}

/// Keep the (finite) ocean plane centered under the player so its edge never
/// comes into view however far they sail.
pub fn follow_player(
    players: Query<&Transform, (With<PlayerShip>, Without<Ocean>)>,
    mut oceans: Query<&mut Transform, With<Ocean>>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    for mut transform in &mut oceans {
        transform.translation.x = player.translation.x;
        transform.translation.z = player.translation.z;
    }
}
