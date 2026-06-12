use std::collections::HashMap;

use bevy::prelude::*;

use crate::blocks::{self, BlockId};

pub const BLOCK_SIZE: f32 = 1.0;

/// A sailable vessel. Yaw and speed live here (not in the Transform) because
/// the float system rewrites the transform's rotation every frame to combine
/// heading with wave roll/pitch.
#[derive(Component)]
pub struct Ship {
    pub yaw: f32,
    pub speed: f32,
}

/// The ship's voxel grid in local block coordinates. Block (0,0,0) is the
/// aft-port-bottom corner; y = 0 is the layer at the waterline.
#[derive(Component)]
pub struct ShipVoxels {
    #[allow(dead_code)]
    pub blocks: HashMap<IVec3, BlockId>,
}

/// The starter vessel: a flat 8x4 barge with a mast and two cannons.
pub fn spawn_starter_barge(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut voxels: HashMap<IVec3, BlockId> = HashMap::new();
    for x in 0..8 {
        for z in 0..4 {
            voxels.insert(IVec3::new(x, 0, z), BlockId::OakHull);
            voxels.insert(IVec3::new(x, 1, z), BlockId::OakDeck);
        }
    }
    for y in 2..6 {
        voxels.insert(IVec3::new(4, y, 2), BlockId::Mast);
    }
    voxels.insert(IVec3::new(2, 2, 0), BlockId::Cannon);
    voxels.insert(IVec3::new(2, 2, 3), BlockId::Cannon);

    let cube = meshes.add(Cuboid::from_length(BLOCK_SIZE));
    let mut block_materials: HashMap<BlockId, Handle<StandardMaterial>> = HashMap::new();

    // Ship origin = center of the grid footprint at the waterline, so wave
    // roll/pitch rotates the hull about its midpoint.
    let center = Vec3::new(4.0, 1.0, 2.0) * BLOCK_SIZE;

    commands
        .spawn((
            Ship {
                yaw: 0.0,
                speed: 0.0,
            },
            ShipVoxels {
                blocks: voxels.clone(),
            },
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (pos, id) in &voxels {
                let material = block_materials
                    .entry(*id)
                    .or_insert_with(|| {
                        materials.add(StandardMaterial {
                            base_color: blocks::def(*id).color,
                            perceptual_roughness: 0.9,
                            ..default()
                        })
                    })
                    .clone();
                parent.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(
                        (pos.as_vec3() + Vec3::splat(0.5)) * BLOCK_SIZE - center,
                    ),
                ));
            }
        });
}

/// WASD sailing: W/S for thrust, A/D to turn. Turning authority scales with
/// speed — a ship dead in the water barely answers the helm.
pub fn sail_ship(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut ships: Query<(&mut Ship, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (mut ship, mut transform) in &mut ships {
        let mut thrust = 0.0;
        if keys.pressed(KeyCode::KeyW) {
            thrust += 3.0;
        }
        if keys.pressed(KeyCode::KeyS) {
            thrust -= 1.2;
        }
        let mut turn = 0.0;
        if keys.pressed(KeyCode::KeyA) {
            turn += 1.0;
        }
        if keys.pressed(KeyCode::KeyD) {
            turn -= 1.0;
        }

        ship.yaw += turn * dt * (0.2 + 0.15 * ship.speed.abs());
        let speed = (ship.speed + thrust * dt) * (1.0 - 0.3 * dt);
        ship.speed = speed.clamp(-2.0, 6.0);

        let forward = Quat::from_rotation_y(ship.yaw) * Vec3::X;
        let velocity = forward * ship.speed * dt;
        transform.translation += velocity;
    }
}

/// Fake buoyancy until real per-block physics: bob on a sine swell and
/// combine heading with a gentle roll/pitch.
pub fn float_ships(time: Res<Time>, mut ships: Query<(&Ship, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (ship, mut transform) in &mut ships {
        transform.translation.y = (t * 0.9).sin() * 0.12;
        let roll = (t * 0.7).sin() * 0.025;
        let pitch = (t * 0.5).cos() * 0.015;
        transform.rotation = Quat::from_rotation_y(ship.yaw)
            * Quat::from_rotation_x(roll)
            * Quat::from_rotation_z(pitch);
    }
}
