use bevy::prelude::*;

/// The waterline sits at y = 0; the ocean plane is nudged slightly below so
/// hull blocks at the waterline don't z-fight with it.
pub fn spawn_ocean(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
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
