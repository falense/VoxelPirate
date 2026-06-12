use std::collections::HashMap;

use bevy::prelude::*;

use crate::blocks::{self, BlockId};
use crate::ship::BLOCK_SIZE;

/// Shared meshes and materials, built once at startup so ships, cannonballs
/// and effects can be spawned from any system without asset churn.
#[derive(Resource)]
pub struct GameAssets {
    pub cube: Handle<Mesh>,
    pub ball_mesh: Handle<Mesh>,
    pub effect_mesh: Handle<Mesh>,
    pub block_materials: HashMap<BlockId, Handle<StandardMaterial>>,
    pub ball_material: Handle<StandardMaterial>,
    pub splash_material: Handle<StandardMaterial>,
    pub smoke_material: Handle<StandardMaterial>,
}

pub fn setup_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut block_materials = HashMap::new();
    for id in blocks::ALL {
        block_materials.insert(
            id,
            materials.add(StandardMaterial {
                base_color: blocks::def(id).color,
                perceptual_roughness: 0.9,
                ..default()
            }),
        );
    }

    commands.insert_resource(GameAssets {
        cube: meshes.add(Cuboid::from_length(BLOCK_SIZE)),
        ball_mesh: meshes.add(Sphere::new(0.22)),
        effect_mesh: meshes.add(Sphere::new(0.5)),
        block_materials,
        ball_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.08, 0.09),
            perceptual_roughness: 0.6,
            ..default()
        }),
        splash_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.93, 0.97, 1.0, 0.45),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        smoke_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.45, 0.44, 0.42, 0.4),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    });
}
