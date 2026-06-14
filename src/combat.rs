use std::collections::HashSet;

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::blocks;
use crate::ship::{BLOCK_SIZE, PlayerShip, Ship, ShipVoxels};

const GRAVITY: f32 = 9.8;
const CANNONBALL_SPEED: f32 = 22.0;
/// Upward component added at the muzzle; with CANNONBALL_SPEED this gives a
/// flat arc with roughly a 20-block range.
const CANNONBALL_LOFT: f32 = 3.0;
/// Blocks within this many cells of the impact cell are destroyed.
const BLAST_RADIUS: f32 = 1.6;
/// A ship sinks once it has lost this fraction of its designed blocks.
const SINK_LOSS_FRACTION: f32 = 0.35;
/// Quick-reject distance for ball/ship collision: farther than any block of
/// the largest hull can be from its ship origin.
const SHIP_BOUNDS_RADIUS: f32 = 15.0;
/// How far a gun may swing off its beam (perpendicular) to bear on an aimed
/// target, expressed as the minimum cosine of the bearing — cos(60°).
const GUN_TRAVERSE_COS: f32 = 0.5;

#[derive(Resource, Default)]
pub struct GameStats {
    pub kills: u32,
    /// Banked salvage blocks; spent automatically on hull upgrades.
    pub salvage: u32,
    /// Index into ship::PLAYER_CLASSES — survives death.
    pub tier: usize,
    pub player_sunk: bool,
    /// The Dreadnought has been sent to the bottom.
    pub victory: bool,
    pub announcement: String,
    pub announce_ttl: f32,
}

impl GameStats {
    pub fn announce(&mut self, message: impl Into<String>) {
        self.announcement = message.into();
        self.announce_ttl = 4.0;
    }
}

/// Bookkeeping when a ship starts going down, shared by cannon fire and
/// ramming. `player_credit` is whether the player dealt the killing blow.
pub fn record_sunk(
    stats: &mut GameStats,
    is_player: bool,
    is_derelict: bool,
    is_boss: bool,
    player_credit: bool,
) {
    if is_player {
        stats.player_sunk = true;
    } else if is_derelict {
        stats.announce("Derelict broke apart — salvage adrift!");
    } else {
        if player_credit {
            stats.kills += 1;
        }
        if is_boss {
            stats.victory = true;
            stats.announcement = "VICTORY — the Dreadnought is sunk! The seas are yours.".into();
            stats.announce_ttl = 12.0;
        }
    }
}

/// Broadside fire control. Intent flags are set by player input or AI and
/// consumed by [`fire_cannons`]; each side reloads independently.
///
/// Two firing modes per side: `fire_*` requests a fixed perpendicular volley
/// (AI and keyboard Q/E), while `aim_*` requests an *aimed* shot that
/// converges on a world point (mouse gunnery, see Spec 001). An aim point
/// takes priority over the perpendicular flag for that side.
#[derive(Component)]
pub struct Broadsides {
    pub reload_time: f32,
    pub reload_port: f32,
    pub reload_starboard: f32,
    pub fire_port: bool,
    pub fire_starboard: bool,
    /// World point the port/starboard guns should converge on this frame.
    pub aim_port: Option<Vec3>,
    pub aim_starboard: Option<Vec3>,
}

impl Broadsides {
    pub fn new(reload_time: f32) -> Self {
        Self {
            reload_time,
            reload_port: 0.0,
            reload_starboard: 0.0,
            fire_port: false,
            fire_starboard: false,
            aim_port: None,
            aim_starboard: None,
        }
    }
}

#[derive(Component)]
pub struct CannonBall {
    pub velocity: Vec3,
    pub shooter: Entity,
    pub age: f32,
}

/// A block knocked off a ship: tumbles, splashes down, and sinks.
#[derive(Component)]
pub struct Debris {
    velocity: Vec3,
    spin: Vec3,
    age: f32,
}

/// A short-lived expanding sphere (splash, gun smoke, impact puff).
#[derive(Component)]
pub struct Effect {
    age: f32,
    life: f32,
    from_scale: f32,
    to_scale: f32,
}

/// Marks a ship that has taken fatal damage; it stops sailing and slips
/// under, then despawns.
#[derive(Component)]
pub struct Sinking {
    pub age: f32,
}

/// Consume fire intents: every gun block on the firing side spawns a
/// cannonball. A perpendicular volley fires straight off the beam and
/// inherits the ship's velocity; an aimed shot (mouse gunnery) converges on
/// the requested point and only fires from guns that can bear on it.
pub fn fire_cannons(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    sounds: Res<crate::audio::SoundBank>,
    mut ships: Query<(Entity, &Ship, &Transform, &ShipVoxels, &mut Broadsides), Without<Sinking>>,
) {
    let dt = time.delta_secs();
    for (entity, ship, transform, voxels, mut guns) in &mut ships {
        guns.reload_port = (guns.reload_port - dt).max(0.0);
        guns.reload_starboard = (guns.reload_starboard - dt).max(0.0);
        let aim_port = guns.aim_port.take();
        let aim_starboard = guns.aim_starboard.take();
        let want_port =
            (aim_port.is_some() || std::mem::take(&mut guns.fire_port)) && guns.reload_port <= 0.0;
        let want_starboard = (aim_starboard.is_some() || std::mem::take(&mut guns.fire_starboard))
            && guns.reload_starboard <= 0.0;
        if !want_port && !want_starboard {
            continue;
        }

        let ship_velocity = transform.rotation * Vec3::X * ship.speed;
        let (mut fired_port, mut fired_starboard) = (false, false);
        for (pos, voxel) in &voxels.blocks {
            if !blocks::def(voxel.id).gun {
                continue;
            }
            let port_side = (pos.z as f32 + 0.5) * BLOCK_SIZE < voxels.center.z;
            if (port_side && !want_port) || (!port_side && !want_starboard) {
                continue;
            }
            // The gun port faces along the beam normal; its blast and the ball
            // leave from here even when an aimed shot angles the trajectory.
            let normal = transform.rotation * if port_side { -Vec3::Z } else { Vec3::Z };
            let muzzle = voxels.grid_to_world(transform, *pos) + normal * BLOCK_SIZE;
            let aim = if port_side { aim_port } else { aim_starboard };
            let velocity = match aim {
                // Aimed shot: converge on the click point, ground-frame so the
                // ball lands where the player clicked. Silent if it can't bear.
                Some(target) => match aim_velocity(muzzle, normal, target) {
                    Some(v) => v,
                    None => continue,
                },
                None => normal * CANNONBALL_SPEED + Vec3::Y * CANNONBALL_LOFT + ship_velocity,
            };
            commands.spawn((
                CannonBall {
                    velocity,
                    shooter: entity,
                    age: 0.0,
                },
                Mesh3d(assets.ball_mesh.clone()),
                MeshMaterial3d(assets.ball_material.clone()),
                Transform::from_translation(muzzle),
            ));
            spawn_effect(
                &mut commands,
                &assets,
                assets.smoke_material.clone(),
                muzzle + normal * 0.5,
                0.35,
                0.4,
                1.6,
            );
            // Brief muzzle flash; update_effects despawns it with the smoke.
            commands.spawn((
                Effect {
                    age: 0.0,
                    life: 0.12,
                    from_scale: 1.0,
                    to_scale: 1.0,
                },
                PointLight {
                    color: Color::srgb(1.0, 0.72, 0.35),
                    intensity: 600_000.0,
                    range: 16.0,
                    ..default()
                },
                Transform::from_translation(muzzle + normal * 0.6),
            ));
            if port_side {
                fired_port = true;
            } else {
                fired_starboard = true;
            }
        }
        if fired_port {
            guns.reload_port = guns.reload_time;
        }
        if fired_starboard {
            guns.reload_starboard = guns.reload_time;
        }
        if fired_port || fired_starboard {
            crate::audio::play(&mut commands, &sounds.boom, 0.5);
        }
    }
}

/// Fly cannonballs under gravity and resolve hits. The flight step is
/// sampled at sub-block resolution so fast balls can't tunnel through a
/// one-block hull. A hit blasts every block within BLAST_RADIUS off the
/// grid as debris, and a ship that has lost enough blocks starts sinking.
pub fn update_cannonballs(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    sounds: Res<crate::audio::SoundBank>,
    mut stats: ResMut<GameStats>,
    mut balls: Query<(Entity, &mut CannonBall, &mut Transform), Without<Ship>>,
    mut ships: Query<
        (
            Entity,
            &Transform,
            &mut ShipVoxels,
            Has<PlayerShip>,
            Has<crate::salvage::Derelict>,
            Has<crate::enemy::Dreadnought>,
        ),
        (With<Ship>, Without<Sinking>),
    >,
    player_shooters: Query<(), With<PlayerShip>>,
) {
    let dt = time.delta_secs();
    let mut newly_sunk: HashSet<Entity> = HashSet::new();
    'balls: for (ball_entity, mut ball, mut ball_transform) in &mut balls {
        ball.age += dt;
        if ball.age > 8.0 {
            commands.entity(ball_entity).despawn();
            continue;
        }
        ball.velocity.y -= GRAVITY * dt;
        let step = ball.velocity * dt;
        let substeps = (step.length() / (BLOCK_SIZE * 0.4)).ceil().max(1.0) as u32;

        for i in 1..=substeps {
            let point = ball_transform.translation + step * (i as f32 / substeps as f32);

            for (ship_entity, ship_transform, mut voxels, is_player, is_derelict, is_boss) in
                &mut ships
            {
                if ship_entity == ball.shooter || newly_sunk.contains(&ship_entity) {
                    continue;
                }
                if point.distance_squared(ship_transform.translation)
                    > SHIP_BOUNDS_RADIUS * SHIP_BOUNDS_RADIUS
                {
                    continue;
                }
                let cell = voxels.world_to_grid(ship_transform, point);
                if !voxels.blocks.contains_key(&cell) {
                    continue;
                }

                let sank = apply_blast(
                    &mut commands,
                    &assets,
                    &mut voxels,
                    ship_transform,
                    cell,
                    BLAST_RADIUS,
                    ball.velocity * 0.15,
                );
                spawn_effect(
                    &mut commands,
                    &assets,
                    assets.smoke_material.clone(),
                    point,
                    0.45,
                    0.5,
                    2.4,
                );
                crate::audio::play(&mut commands, &sounds.crunch, 0.7);

                if sank {
                    newly_sunk.insert(ship_entity);
                    start_sinking(&mut commands, &assets, ship_entity, ship_transform, &voxels);
                    record_sunk(
                        &mut stats,
                        is_player,
                        is_derelict,
                        is_boss,
                        player_shooters.contains(ball.shooter),
                    );
                }

                commands.entity(ball_entity).despawn();
                continue 'balls;
            }

            if point.y < 0.0 {
                spawn_effect(
                    &mut commands,
                    &assets,
                    assets.splash_material.clone(),
                    Vec3::new(point.x, 0.1, point.z),
                    0.5,
                    0.4,
                    2.0,
                );
                crate::audio::play(&mut commands, &sounds.splash, 0.25);
                commands.entity(ball_entity).despawn();
                continue 'balls;
            }
        }
        ball_transform.translation += step;
    }
}

pub fn update_debris(
    mut commands: Commands,
    time: Res<Time>,
    mut debris: Query<(Entity, &mut Debris, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut piece, mut transform) in &mut debris {
        piece.age += dt;
        if transform.translation.y < 0.0 {
            // In the water: drag hard, then sink slowly.
            let drag = 1.0 - (2.5 * dt).min(0.9);
            piece.velocity *= drag;
            piece.velocity.y -= GRAVITY * 0.05 * dt;
        } else {
            piece.velocity.y -= GRAVITY * 0.7 * dt;
        }
        transform.translation += piece.velocity * dt;
        transform.rotation = Quat::from_scaled_axis(piece.spin * dt) * transform.rotation;
        if piece.age > 6.0 || transform.translation.y < -5.0 {
            commands.entity(entity).despawn();
        }
    }
}

pub fn update_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut effects: Query<(Entity, &mut Effect, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut effect, mut transform) in &mut effects {
        effect.age += dt;
        if effect.age >= effect.life {
            commands.entity(entity).despawn();
            continue;
        }
        let t = effect.age / effect.life;
        transform.scale =
            Vec3::splat(effect.from_scale + (effect.to_scale - effect.from_scale) * t);
    }
}

/// A sinking ship lists to one side and slips under with gathering speed,
/// despawning once well below the surface.
pub fn sink_ships(
    mut commands: Commands,
    time: Res<Time>,
    mut ships: Query<(Entity, &mut Sinking, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut sinking, mut transform) in &mut ships {
        sinking.age += dt;
        transform.translation.y -= (0.3 + sinking.age * 0.35) * dt;
        transform.rotate_local_z(0.06 * dt);
        transform.rotate_local_x(0.025 * dt);
        if transform.translation.y < -9.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Blast every block within `radius` cells of the impact cell off the grid
/// as tumbling debris. Returns whether the ship has now taken fatal damage.
pub fn apply_blast(
    commands: &mut Commands,
    assets: &GameAssets,
    voxels: &mut ShipVoxels,
    ship_transform: &Transform,
    center_cell: IVec3,
    radius: f32,
    kick: Vec3,
) -> bool {
    let blasted: Vec<IVec3> = voxels
        .blocks
        .keys()
        .filter(|c| (**c - center_cell).as_vec3().length() <= radius)
        .copied()
        .collect();
    for cell in blasted {
        let voxel = voxels.blocks.remove(&cell).unwrap();
        commands.entity(voxel.entity).despawn();
        let world = voxels.grid_to_world(ship_transform, cell);
        commands.spawn((
            Debris {
                velocity: jitter(world) * 2.5 + Vec3::Y * 2.0 + kick,
                spin: jitter(world + Vec3::splat(31.7)) * 6.0,
                age: 0.0,
            },
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.block_materials[&voxel.id].clone()),
            Transform::from_translation(world).with_scale(Vec3::splat(0.85)),
        ));
    }
    voxels.damage_fraction() >= SINK_LOSS_FRACTION
}

/// Mark a ship as going down and bob part of the wreck up as flotsam,
/// plus one chest of gold plunder.
pub fn start_sinking(
    commands: &mut Commands,
    assets: &GameAssets,
    ship_entity: Entity,
    ship_transform: &Transform,
    voxels: &ShipVoxels,
) {
    commands.entity(ship_entity).insert(Sinking { age: 0.0 });
    for (count, (cell, voxel)) in voxels.blocks.iter().step_by(3).enumerate() {
        if count >= 10 {
            break;
        }
        let world = voxels.grid_to_world(ship_transform, *cell);
        crate::salvage::spawn_flotsam(
            commands,
            assets,
            voxel.id,
            Vec3::new(world.x, 0.15, world.z),
        );
    }
    crate::salvage::spawn_flotsam(
        commands,
        assets,
        blocks::BlockId::Gold,
        ship_transform.translation.with_y(0.15),
    );
}

fn spawn_effect(
    commands: &mut Commands,
    assets: &GameAssets,
    material: Handle<StandardMaterial>,
    position: Vec3,
    life: f32,
    from_scale: f32,
    to_scale: f32,
) {
    commands.spawn((
        Effect {
            age: 0.0,
            life,
            from_scale,
            to_scale,
        },
        Mesh3d(assets.effect_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(position).with_scale(Vec3::splat(from_scale)),
    ));
}

/// Solve for the launch velocity that sends a ball from `muzzle` onto the sea
/// point `target`, but only if the gun (beam `normal`) can swing to bear
/// within its traverse arc. Returns `None` when the target is outside the arc
/// so the gun stays silent. Velocity is in the world (ground) frame — ship
/// motion is intentionally not inherited, so the ball lands where the player
/// clicked. Out-of-range targets get the 45° max-range shot toward the point.
fn aim_velocity(muzzle: Vec3, normal: Vec3, target: Vec3) -> Option<Vec3> {
    let to = target - muzzle;
    let flat = Vec3::new(to.x, 0.0, to.z);
    let d = flat.length();
    if d < 0.5 {
        return None;
    }
    let hdir = flat / d;
    let beam = Vec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
    if beam.dot(hdir) < GUN_TRAVERSE_COS {
        return None;
    }
    let s = CANNONBALL_SPEED;
    let h = muzzle.y; // target sits at the waterline, world y = 0
    // Hitting (d, -h) at fixed speed s reduces to k·u² − d·u + (k − h) = 0 for
    // u = tan(elevation), with k = g·d² / (2·s²). The smaller root is the flat
    // (low-angle) trajectory.
    let k = GRAVITY * d * d / (2.0 * s * s);
    let disc = d * d - 4.0 * k * (k - h);
    let elevation = if disc >= 0.0 {
        ((d - disc.sqrt()) / (2.0 * k)).atan()
    } else {
        std::f32::consts::FRAC_PI_4
    };
    let (sin, cos) = elevation.sin_cos();
    Some(hdir * (s * cos) + Vec3::Y * (s * sin))
}

/// Cheap position-seeded pseudo-random vector in roughly [-1, 1]^3; good
/// enough to scatter debris without pulling in a rand crate.
fn jitter(seed: Vec3) -> Vec3 {
    Vec3::new(
        (seed.dot(Vec3::new(127.1, 311.7, 74.7)).sin() * 43758.547).fract(),
        (seed.dot(Vec3::new(269.5, 183.3, 246.1)).sin() * 43758.547).fract(),
        (seed.dot(Vec3::new(113.5, 271.9, 124.6)).sin() * 43758.547).fract(),
    )
}
