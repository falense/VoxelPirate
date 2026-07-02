use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;

use crate::ship::PlayerShip;

/// The wave-animated sea surface around the player. Also carries the
/// follow-player behaviour.
#[derive(Component)]
pub struct Ocean;

/// A huge flat skirt under the animated plane, reaching the true horizon so
/// the finite detail mesh never shows an edge against the sky.
#[derive(Component)]
pub struct OceanSkirt;

/// A slowly veering global wind. Sailing with it is a quarter faster,
/// beating into it a quarter slower — worth tacking for.
#[derive(Resource)]
pub struct Wind {
    pub angle: f32,
}

impl Default for Wind {
    fn default() -> Self {
        Self { angle: 0.7 }
    }
}

impl Wind {
    /// Unit vector the wind blows toward.
    pub fn dir(&self) -> Vec3 {
        Vec3::new(self.angle.cos(), 0.0, self.angle.sin())
    }
}

pub fn update_wind(time: Res<Time>, mut wind: ResMut<Wind>) {
    let t = time.elapsed_secs();
    wind.angle += (0.012 + (t * 0.05).sin() * 0.01) * time.delta_secs();
}

/// The swell trains that make up the sea state, as (direction, wavelength,
/// amplitude, angular speed). World-anchored, so the sea mesh, ship
/// buoyancy, flotsam, and splash heights all read the same surface through
/// [`wave_height`]. A long primary swell, a shorter cross-swell, and chop.
const SWELLS: [(Vec2, f32, f32, f32); 3] = [
    (Vec2::new(1.0, 0.35), 36.0, 0.30, 0.55),
    (Vec2::new(-0.45, 1.0), 22.0, 0.16, 0.85),
    (Vec2::new(0.7, -0.7), 13.0, 0.07, 1.40),
];

/// Height of the sea surface (world y) at a world xz position.
pub fn wave_height(p: Vec2, t: f32) -> f32 {
    wave_sample(p, t).0
}

/// Height and slope (dh/dx, dh/dz) of the sea surface in one pass — one
/// sin/cos pair per swell, which matters when the sea mesh samples this at
/// every vertex every frame.
pub fn wave_sample(p: Vec2, t: f32) -> (f32, Vec2) {
    let mut height = 0.0;
    let mut gradient = Vec2::ZERO;
    for (dir, wavelength, amplitude, speed) in SWELLS {
        let k = std::f32::consts::TAU / wavelength;
        let dir = dir.normalize();
        let (sin, cos) = (dir.dot(p) * k + t * speed).sin_cos();
        height += amplitude * sin;
        gradient += dir * (amplitude * k * cos);
    }
    (height, gradient)
}

/// Side length of the (finite, player-following) animated sea mesh; past it
/// the flat skirt takes over, far enough out that the seam sits low in view.
const OCEAN_SIZE: f32 = 480.0;
/// Grid resolution; cells of ~4 blocks resolve the shortest swell.
const OCEAN_SUBDIVISIONS: u32 = 120;

/// The waterline sits at y = 0. The surface itself is displaced every frame
/// by [`animate_ocean`]; a glossy material catches the sun on the swell.
/// A vast flat skirt slightly below the trough line carries the same water
/// out to the true horizon, where the atmosphere hazes it away.
pub fn spawn_ocean(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let water = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.19, 0.37),
        perceptual_roughness: 0.15,
        metallic: 0.0,
        ..default()
    });
    commands.spawn((
        Ocean,
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(OCEAN_SIZE, OCEAN_SIZE)
                    .subdivisions(OCEAN_SUBDIVISIONS),
            ),
        ),
        MeshMaterial3d(water.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    commands.spawn((
        OceanSkirt,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20_000.0, 20_000.0))),
        MeshMaterial3d(water),
        Transform::from_xyz(0.0, -0.6, 0.0),
    ));
}

/// Keep the (finite) ocean plane centered under the player so its edge never
/// comes into view however far they sail.
pub fn follow_player(
    players: Query<&Transform, (With<PlayerShip>, Without<Ocean>, Without<OceanSkirt>)>,
    mut oceans: Query<&mut Transform, Or<(With<Ocean>, With<OceanSkirt>)>>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    for mut transform in &mut oceans {
        transform.translation.x = player.translation.x;
        transform.translation.z = player.translation.z;
    }
}

/// Roll the swell through the sea mesh: every vertex takes the wave height
/// at its *world* position (the plane follows the player, the waves don't),
/// with normals from the analytic slope so the sun glints move with the sea.
/// Dropped a hair below [`wave_height`] so hulls never z-fight the surface.
pub fn animate_ocean(
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    oceans: Query<(&Mesh3d, &Transform), With<Ocean>>,
) {
    let t = time.elapsed_secs();
    for (mesh, transform) in &oceans {
        let Some(mesh) = meshes.get_mut(&mesh.0) else {
            continue;
        };
        let origin = transform.translation;
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        let gradients: Vec<Vec2> = positions
            .iter_mut()
            .map(|p| {
                let world = Vec2::new(p[0] + origin.x, p[2] + origin.z);
                let (height, gradient) = wave_sample(world, t);
                p[1] = height - 0.04 - origin.y;
                gradient
            })
            .collect();
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL)
        else {
            continue;
        };
        for (n, g) in normals.iter_mut().zip(&gradients) {
            *n = Vec3::new(-g.x, 1.0, -g.y).normalize().to_array();
        }
    }
}
