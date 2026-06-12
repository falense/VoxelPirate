use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::blocks::{self, BlockId};
use crate::combat::{GameStats, Sinking};
use crate::ship::{PlayerShip, ShipVoxels, Voxel};

/// How far from the camera a build raycast reaches.
const BUILD_REACH: f32 = 80.0;

/// Tab toggles between sailing (mouse fires) and building (mouse places
/// and removes blocks on your own ship).
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum PlayMode {
    #[default]
    Sail,
    Build,
}

#[derive(Resource)]
pub struct BuildState {
    pub selected: BlockId,
}

/// Translucent cube that previews where the next block will land.
#[derive(Component)]
pub struct BuildGhost;

pub fn setup_build(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let ghost_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.35),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        BuildGhost,
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(ghost_material),
        Transform::default().with_scale(Vec3::splat(1.05)),
        Visibility::Hidden,
    ));
    commands.insert_resource(BuildState {
        selected: BlockId::OakHull,
    });
}

pub fn toggle_mode(keys: Res<ButtonInput<KeyCode>>, mut mode: ResMut<PlayMode>) {
    if keys.just_pressed(KeyCode::Tab) {
        *mode = match *mode {
            PlayMode::Sail => PlayMode::Build,
            PlayMode::Build => PlayMode::Sail,
        };
    }
}

/// In build mode: 1-6 select a block type, left click places it on the face
/// under the cursor (costs the block's salvage price), right click removes
/// the targeted block for a refund. The ghost cube previews the placement.
pub fn build_input(
    mut commands: Commands,
    mode: Res<PlayMode>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    assets: Res<GameAssets>,
    sounds: Res<crate::audio::SoundBank>,
    mut state: ResMut<BuildState>,
    mut stats: ResMut<GameStats>,
    aim: Res<AimOverride>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut players: Query<
        (Entity, &Transform, &mut ShipVoxels),
        (With<PlayerShip>, Without<Sinking>, Without<BuildGhost>),
    >,
    mut ghosts: Query<(&mut Transform, &mut Visibility), With<BuildGhost>>,
) {
    let Ok((mut ghost_transform, mut ghost_visibility)) = ghosts.single_mut() else {
        return;
    };
    if *mode != PlayMode::Build {
        *ghost_visibility = Visibility::Hidden;
        return;
    }

    let digits = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
    ];
    for (key, id) in digits.iter().zip(blocks::ALL) {
        if keys.just_pressed(*key) {
            state.selected = id;
        }
    }

    *ghost_visibility = Visibility::Hidden;
    let Ok((ship_entity, ship_transform, mut voxels)) = players.single_mut() else {
        return;
    };
    let Some(ray) = cursor_ray(&windows, &cameras, &aim) else {
        return;
    };
    let Some((hit_cell, place_cell)) = raycast_grid(&voxels, ship_transform, ray) else {
        return;
    };

    if place_cell.y >= 0 && !voxels.blocks.contains_key(&place_cell) {
        ghost_transform.translation = voxels.grid_to_world(ship_transform, place_cell);
        ghost_transform.rotation = ship_transform.rotation;
        *ghost_visibility = Visibility::Visible;
    }

    if mouse.just_pressed(MouseButton::Left)
        && place_cell.y >= 0
        && !voxels.blocks.contains_key(&place_cell)
    {
        let cost = blocks::def(state.selected).cost;
        if stats.salvage < cost {
            stats.announce(format!(
                "Not enough salvage for {} ({cost} needed)",
                blocks::def(state.selected).name
            ));
            return;
        }
        stats.salvage -= cost;
        let id = state.selected;
        let child = commands
            .spawn((
                Mesh3d(assets.cube.clone()),
                MeshMaterial3d(assets.block_materials[&id].clone()),
                Transform::from_translation(voxels.local_offset(place_cell)),
                ChildOf(ship_entity),
            ))
            .id();
        voxels
            .blocks
            .insert(place_cell, Voxel { id, entity: child });
        voxels.plan.insert(place_cell, id);
        crate::audio::play(&mut commands, &sounds.ding, 0.35);
    }

    if mouse.just_pressed(MouseButton::Right)
        && let Some(voxel) = voxels.blocks.remove(&hit_cell)
    {
        commands.entity(voxel.entity).despawn();
        voxels.plan.remove(&hit_cell);
        stats.salvage += blocks::def(voxel.id).cost;
        crate::audio::play(&mut commands, &sounds.crunch, 0.3);
    }
}

/// When set (by the self-test harness), aiming uses this viewport point
/// instead of the OS cursor.
#[derive(Resource, Default)]
pub struct AimOverride(pub Option<Vec2>);

/// Project the cursor (or the aim override) into the world.
pub fn cursor_ray(
    windows: &Query<&Window>,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    aim: &AimOverride,
) -> Option<Ray3d> {
    let (camera, camera_transform) = cameras.single().ok()?;
    let cursor = match aim.0 {
        Some(point) => point,
        None => windows.single().ok()?.cursor_position()?,
    };
    camera.viewport_to_world(camera_transform, cursor).ok()
}

/// Walk the ray through the ship's voxel grid (Amanatides & Woo DDA) and
/// return the first occupied cell plus the empty cell just before it — the
/// removal and placement targets respectively.
fn raycast_grid(
    voxels: &ShipVoxels,
    ship_transform: &Transform,
    ray: Ray3d,
) -> Option<(IVec3, IVec3)> {
    let inverse = ship_transform.compute_affine().inverse();
    // Grid space: cell (i, j, k) spans [i, i+1) on each axis.
    let origin = inverse.transform_point3(ray.origin) + voxels.center;
    let dir = inverse
        .transform_vector3(*ray.direction)
        .normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }

    let mut cell = origin.floor().as_ivec3();
    let step = IVec3::new(
        if dir.x > 0.0 { 1 } else { -1 },
        if dir.y > 0.0 { 1 } else { -1 },
        if dir.z > 0.0 { 1 } else { -1 },
    );
    let next_boundary = |cell_coord: i32, step: i32, origin: f32, dir: f32| -> f32 {
        if dir.abs() < 1e-6 {
            f32::INFINITY
        } else {
            let boundary = cell_coord + if step > 0 { 1 } else { 0 };
            (boundary as f32 - origin) / dir
        }
    };
    let mut t_max = Vec3::new(
        next_boundary(cell.x, step.x, origin.x, dir.x),
        next_boundary(cell.y, step.y, origin.y, dir.y),
        next_boundary(cell.z, step.z, origin.z, dir.z),
    );
    let t_delta = Vec3::new(
        if dir.x.abs() < 1e-6 {
            f32::INFINITY
        } else {
            1.0 / dir.x.abs()
        },
        if dir.y.abs() < 1e-6 {
            f32::INFINITY
        } else {
            1.0 / dir.y.abs()
        },
        if dir.z.abs() < 1e-6 {
            f32::INFINITY
        } else {
            1.0 / dir.z.abs()
        },
    );

    let mut previous = cell;
    for _ in 0..256 {
        if voxels.blocks.contains_key(&cell) {
            return Some((cell, previous));
        }
        previous = cell;
        let travelled;
        if t_max.x < t_max.y && t_max.x < t_max.z {
            travelled = t_max.x;
            cell.x += step.x;
            t_max.x += t_delta.x;
        } else if t_max.y < t_max.z {
            travelled = t_max.y;
            cell.y += step.y;
            t_max.y += t_delta.y;
        } else {
            travelled = t_max.z;
            cell.z += step.z;
            t_max.z += t_delta.z;
        }
        if travelled > BUILD_REACH {
            return None;
        }
    }
    None
}
