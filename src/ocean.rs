use bevy::asset::uuid_handle;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::{Shader, ShaderRef};

use crate::ship::PlayerShip;

/// The sea's `StandardMaterial` extended with a vertex stage that rolls the
/// swell through the mesh on the GPU. The CPU animated the vertices at
/// first, but rewriting and re-uploading ~15k vertices every frame cost
/// more than the entire rest of the game.
pub type OceanMaterial = ExtendedMaterial<StandardMaterial, OceanExtension>;

/// The three [`SWELLS`], packed for the shader as
/// `(dir.x * k, dir.y * k, angular speed, amplitude)`, `k = tau / wavelength`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct OceanExtension {
    #[uniform(100)]
    swell_a: Vec4,
    #[uniform(101)]
    swell_b: Vec4,
    #[uniform(102)]
    swell_c: Vec4,
}

impl Default for OceanExtension {
    fn default() -> Self {
        let pack = |i: usize| {
            let (dir, wavelength, amplitude, speed) = SWELLS[i];
            let d = dir.normalize() * (std::f32::consts::TAU / wavelength);
            Vec4::new(d.x, d.y, speed, amplitude)
        };
        Self {
            swell_a: pack(0),
            swell_b: pack(1),
            swell_c: pack(2),
        }
    }
}

impl MaterialExtension for OceanExtension {
    fn vertex_shader() -> ShaderRef {
        OCEAN_SHADER_HANDLE.into()
    }
}

const OCEAN_SHADER_HANDLE: Handle<Shader> = uuid_handle!("6f3b9c42-8a51-4d0e-9b7a-51c2ad38e6f1");

/// Displace the water in the vertex stage by the same swell sum as
/// [`wave_sample`], with analytic normals. `globals.time` is the wrapped
/// game clock, which is why the CPU side samples wrapped time too.
const OCEAN_WGSL: &str = r#"
#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> swell_a: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> swell_b: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> swell_c: vec4<f32>;

// Returns (d height / dx, d height / dz, height) for one swell train.
fn swell(s: vec4<f32>, p: vec2<f32>, t: f32) -> vec3<f32> {
    let phase = dot(s.xy, p) + t * s.z;
    return vec3<f32>(s.xy * (s.w * cos(phase)), s.w * sin(phase));
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    var world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local, vec4<f32>(vertex.position, 1.0));
    let t = globals.time;
    let a = swell(swell_a, world_position.xz, t);
    let b = swell(swell_b, world_position.xz, t);
    let c = swell(swell_c, world_position.xz, t);
    let grad = a.xy + b.xy + c.xy;
    // A hair below the sampled height so hulls never z-fight the surface.
    world_position.y = a.z + b.z + c.z - 0.04;
    out.world_position = world_position;
    out.position = position_world_to_clip(world_position.xyz);
    out.world_normal = normalize(vec3<f32>(-grad.x, 1.0, -grad.y));
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
    return out;
}
"#;

/// Registers the ocean material pipeline and its embedded shader.
pub struct OceanPlugin;

impl Plugin for OceanPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<OceanMaterial>::default());
        let _ = app.world_mut().resource_mut::<Assets<Shader>>().insert(
            OCEAN_SHADER_HANDLE.id(),
            Shader::from_wgsl(OCEAN_WGSL, "ocean.wgsl"),
        );
    }
}

/// The wave-displaced sea surface around the player (follows the player).
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

/// Height and slope (dh/dx, dh/dz) of the sea surface in one pass. The
/// GPU vertex stage (`OCEAN_WGSL`) computes exactly this sum; keep the two
/// in sync through the packed [`OceanExtension`] uniforms.
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
/// Grid resolution: 6 m cells — enough for the two big swell trains, and a
/// deliberate ceiling on vertex-shader load for integrated GPUs.
const OCEAN_SUBDIVISIONS: u32 = 80;

/// The waterline sits at y = 0. The surface is displaced on the GPU by the
/// [`OceanMaterial`] vertex stage; a glossy material catches the sun on the
/// swell. A vast flat skirt slightly below the trough line carries the same
/// water out to the true horizon, where the atmosphere hazes it away.
pub fn spawn_ocean(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ocean_materials: ResMut<Assets<OceanMaterial>>,
) {
    let water = StandardMaterial {
        base_color: Color::srgb(0.03, 0.19, 0.37),
        perceptual_roughness: 0.15,
        metallic: 0.0,
        ..default()
    };
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
        MeshMaterial3d(ocean_materials.add(ExtendedMaterial {
            base: water.clone(),
            extension: OceanExtension::default(),
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    commands.spawn((
        OceanSkirt,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20_000.0, 20_000.0))),
        MeshMaterial3d(materials.add(water)),
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
