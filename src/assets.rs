use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::blocks::{self, BlockId};
use crate::ship::BLOCK_SIZE;

/// Pixel side of one block's texture tile.
pub const TILE: u32 = 16;
/// Tiles per atlas row/column; `TILE_COLS`² must cover `blocks::ALL`.
pub const TILE_COLS: u32 = 4;

/// Shared meshes and materials, built once at startup so ships, cannonballs
/// and effects can be spawned from any system without asset churn.
///
/// Ship hulls render through the two atlas materials (see
/// [`crate::ship::remesh_ships`]); the per-block materials cover loose
/// cubes — debris, flotsam, and the build ghost.
#[derive(Resource)]
pub struct GameAssets {
    pub cube: Handle<Mesh>,
    pub ball_mesh: Handle<Mesh>,
    pub effect_mesh: Handle<Mesh>,
    pub block_materials: HashMap<BlockId, Handle<StandardMaterial>>,
    /// The 16x16 tile of each block, for UI icons (hotbar slots).
    pub block_tiles: HashMap<BlockId, Handle<Image>>,
    /// All opaque blocks, atlas-textured (color + metal/rough + emissive).
    pub atlas_opaque: Handle<StandardMaterial>,
    /// Translucent blocks (sails), alpha-blended, casting no shadow.
    pub atlas_translucent: Handle<StandardMaterial>,
    pub ball_material: Handle<StandardMaterial>,
    pub splash_material: Handle<StandardMaterial>,
    pub smoke_material: Handle<StandardMaterial>,
}

/// The atlas UV rectangle (min, max) of a block's tile, inset half a texel
/// so face quads never bleed into a neighbouring tile.
pub fn tile_uv(id: BlockId) -> (Vec2, Vec2) {
    let index = blocks::ALL.iter().position(|b| *b == id).unwrap() as u32;
    let (col, row) = (index % TILE_COLS, index / TILE_COLS);
    let size = 1.0 / TILE_COLS as f32;
    let inset = 0.5 / (TILE_COLS * TILE) as f32;
    let min = Vec2::new(col as f32, row as f32) * size;
    (min + Vec2::splat(inset), min + Vec2::splat(size - inset))
}

pub fn setup_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Per-block color tiles, composed into one atlas plus standalone images
    // for the loose-cube materials.
    let atlas_px = TILE_COLS * TILE;
    let mut color_atlas = vec![0u8; (atlas_px * atlas_px * 4) as usize];
    let mut surface_atlas = vec![0u8; (atlas_px * atlas_px * 4) as usize];
    let mut emissive_atlas = vec![0u8; (atlas_px * atlas_px * 4) as usize];
    let mut block_materials = HashMap::new();
    let mut block_tiles = HashMap::new();
    for (index, id) in blocks::ALL.into_iter().enumerate() {
        let def = blocks::def(id);
        let mut tile = vec![0u8; (TILE * TILE * 4) as usize];
        let (col, row) = (index as u32 % TILE_COLS, index as u32 / TILE_COLS);
        for py in 0..TILE {
            for px in 0..TILE {
                let rgba = shade(id, px, py);
                let t = ((py * TILE + px) * 4) as usize;
                tile[t..t + 4].copy_from_slice(&rgba);
                let a = (((row * TILE + py) * atlas_px + col * TILE + px) * 4) as usize;
                color_atlas[a..a + 4].copy_from_slice(&rgba);
                // glTF convention: G = roughness, B = metallic.
                surface_atlas[a..a + 4].copy_from_slice(&[
                    0,
                    (def.roughness * 255.0) as u8,
                    (def.metallic * 255.0) as u8,
                    255,
                ]);
                let glow = emissive_shade(id, px, py);
                emissive_atlas[a..a + 4].copy_from_slice(&glow);
            }
        }
        let image = images.add(tile_image(tile, TILE, TextureFormat::Rgba8UnormSrgb));
        block_tiles.insert(id, image.clone());
        block_materials.insert(
            id,
            materials.add(StandardMaterial {
                base_color_texture: Some(image),
                perceptual_roughness: def.roughness,
                metallic: def.metallic,
                emissive: LinearRgba::WHITE * def.emissive,
                alpha_mode: if def.color.alpha() < 1.0 {
                    AlphaMode::Blend
                } else {
                    AlphaMode::Opaque
                },
                ..default()
            }),
        );
    }

    let color_atlas = images.add(tile_image(
        color_atlas,
        atlas_px,
        TextureFormat::Rgba8UnormSrgb,
    ));
    let surface_atlas = images.add(tile_image(
        surface_atlas,
        atlas_px,
        TextureFormat::Rgba8Unorm,
    ));
    let emissive_atlas = images.add(tile_image(
        emissive_atlas,
        atlas_px,
        TextureFormat::Rgba8UnormSrgb,
    ));

    let atlas_opaque = materials.add(StandardMaterial {
        base_color_texture: Some(color_atlas.clone()),
        // Textures multiply these, so leave them at full scale.
        perceptual_roughness: 1.0,
        metallic: 1.0,
        metallic_roughness_texture: Some(surface_atlas),
        emissive: LinearRgba::WHITE * 4.0,
        emissive_texture: Some(emissive_atlas),
        ..default()
    });
    let atlas_translucent = materials.add(StandardMaterial {
        base_color_texture: Some(color_atlas),
        perceptual_roughness: 0.8,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands.insert_resource(GameAssets {
        cube: meshes.add(Cuboid::from_length(BLOCK_SIZE)),
        ball_mesh: meshes.add(Sphere::new(0.22)),
        effect_mesh: meshes.add(Sphere::new(0.5)),
        block_materials,
        block_tiles,
        atlas_opaque,
        atlas_translucent,
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

fn tile_image(data: Vec<u8>, size: u32, format: TextureFormat) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::default(),
    );
    // Crisp voxel-game pixels rather than blurry interpolation.
    image.sampler = ImageSampler::nearest();
    image
}

/// Deterministic per-pixel hash in [0, 1) — the texture equivalent of the
/// debris jitter, so tiles need no rand crate and never change run to run.
fn hash(x: u32) -> f32 {
    let mut x = x.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    x ^= x >> 17;
    x = x.wrapping_mul(0xed5a_d4bb);
    x ^= x >> 11;
    (x & 0xffff) as f32 / 65535.0
}

/// A block's base color scaled by `factor`, as sRGB bytes with alpha kept.
fn tint(id: BlockId, factor: f32) -> [u8; 4] {
    let c = blocks::def(id).color.to_srgba();
    [
        (c.red * factor).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8,
        (c.green * factor).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8,
        (c.blue * factor).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8,
        (c.alpha * 255.0) as u8,
    ]
}

/// Procedural texel color for a block tile. Every pattern is built from the
/// block's registry color, so recoloring a block re-skins it too.
fn shade(id: BlockId, px: u32, py: u32) -> [u8; 4] {
    let seed = px + py * TILE + blocks::ALL.iter().position(|b| *b == id).unwrap() as u32 * 289;
    let noise = hash(seed) - 0.5;
    match id {
        // Horizontal hull strakes: 4-texel planks with dark seams and
        // staggered butt joints, each plank its own shade.
        BlockId::OakHull => {
            let plank = py / 4;
            let joint = (hash(plank * 31 + 7) * TILE as f32) as u32;
            let mut f = 0.88 + 0.24 * hash(plank * 97 + 13) + noise * 0.10;
            if py.is_multiple_of(4) {
                f *= 0.55;
            }
            if px == joint {
                f *= 0.6;
            }
            tint(id, f)
        }
        // Deck planking runs the other way, scrubbed lighter.
        BlockId::OakDeck => {
            let plank = px / 4;
            let joint = (hash(plank * 53 + 3) * TILE as f32) as u32;
            let mut f = 0.92 + 0.20 * hash(plank * 71 + 29) + noise * 0.08;
            if px.is_multiple_of(4) {
                f *= 0.6;
            }
            if py == joint {
                f *= 0.65;
            }
            tint(id, f)
        }
        // Vertical grain with occasional dark streaks.
        BlockId::Mast => {
            let mut f = 0.9 + 0.2 * hash(px * 41 + 11) + noise * 0.08;
            if hash(px * 13 + 5) > 0.75 {
                f *= 0.78;
            }
            tint(id, f)
        }
        // Riveted plates: an 8-texel grid with bright rivet heads (the
        // carronade shares the pattern on its stubby red-brown casting).
        BlockId::IronHull | BlockId::Carronade => {
            let plate = (px / 8) + (py / 8) * 2;
            let mut f = 0.9 + 0.2 * hash(plate * 61 + 17) + noise * 0.08;
            if px.is_multiple_of(8) || py.is_multiple_of(8) {
                f *= 0.62;
            }
            if (px % 8 == 2 || px % 8 == 6) && (py % 8 == 2 || py % 8 == 6) {
                f *= 1.45;
            }
            tint(id, f)
        }
        // Canvas: alternating weave, darker seam every five courses.
        BlockId::Sail => {
            let weave = if (px + py).is_multiple_of(2) {
                1.03
            } else {
                0.97
            };
            let mut f = weave + noise * 0.05;
            if py.is_multiple_of(5) {
                f *= 0.88;
            }
            tint(id, f)
        }
        // Gun metal with paler reinforcing rings.
        BlockId::Cannon | BlockId::Culverin => {
            let mut f = 0.95 + noise * 0.12;
            if matches!(py, 3 | 4 | 11 | 12) {
                f *= 1.5;
            }
            tint(id, f)
        }
        // Treasure: glinting facets over crevices.
        BlockId::Gold => {
            let sparkle = hash(seed * 7 + 1);
            let f = if sparkle > 0.92 {
                1.6
            } else if sparkle < 0.14 {
                0.66
            } else {
                1.0 + noise * 0.1
            };
            tint(id, f)
        }
        // Woven bunting with a ragged fly edge.
        BlockId::Flag => {
            let mut f = 0.95 + 0.12 * hash(py * 19 + 3) + noise * 0.06;
            if px >= 14 && hash(py * 23 + 9) > 0.5 {
                f *= 0.7;
            }
            tint(id, f)
        }
        // Tar: matte near-black with faint brush streaks.
        BlockId::Trim => tint(id, 0.9 + 0.2 * hash(py * 37 + 5) + noise * 0.05),
        // A lit lamp: dark frame and muntins around warm glass, brightest
        // in the middle (the emissive tile mirrors the glass).
        BlockId::Lantern => {
            if lantern_frame(px, py) {
                return tint(BlockId::Trim, 1.0);
            }
            let dx = px as f32 - 7.5;
            let dy = py as f32 - 7.5;
            let f = 1.15 - (dx * dx + dy * dy) * 0.012 + noise * 0.05;
            tint(id, f)
        }
    }
}

/// The lantern tile's dark frame and muntin bars (the rest is glass).
fn lantern_frame(px: u32, py: u32) -> bool {
    !(2..=13).contains(&px) || !(2..=13).contains(&py) || px == 7 || px == 8
}

/// Emissive texel for a block tile: black except glowing glass.
fn emissive_shade(id: BlockId, px: u32, py: u32) -> [u8; 4] {
    if id == BlockId::Lantern && !lantern_frame(px, py) {
        let dx = px as f32 - 7.5;
        let dy = py as f32 - 7.5;
        let f = (1.0 - (dx * dx + dy * dy) * 0.014).max(0.2);
        return [(250.0 * f) as u8, (200.0 * f) as u8, (110.0 * f) as u8, 255];
    }
    [0, 0, 0, 255]
}
