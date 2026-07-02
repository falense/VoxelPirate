use bevy::prelude::*;

/// Every placeable block in the game.
///
/// Gameplay systems must only read block properties through [`def`], never
/// match on the id directly — that keeps "add a new block" a two-line change.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BlockId {
    OakHull,
    IronHull,
    OakDeck,
    Mast,
    Sail,
    Cannon,
    Gold,
    Flag,
    Culverin,
    Carronade,
    Trim,
    Lantern,
}

/// Every block id, for systems that need to pre-build per-block assets.
/// Keep in sync with [`BlockId`] when adding blocks. Order matters: build
/// mode maps digit keys 1-0 onto the first ten entries.
pub const ALL: [BlockId; 12] = [
    BlockId::OakHull,
    BlockId::IronHull,
    BlockId::OakDeck,
    BlockId::Mast,
    BlockId::Sail,
    BlockId::Cannon,
    BlockId::Gold,
    BlockId::Flag,
    BlockId::Culverin,
    BlockId::Carronade,
    BlockId::Trim,
    BlockId::Lantern,
];

/// Ballistics of a gun block. Every block with `gun: Some(..)` joins the
/// ship's broadside; the shot's character comes entirely from these numbers.
pub struct GunDef {
    /// Muzzle speed — faster shoots flatter and farther.
    pub speed: f32,
    /// Blocks within this many cells of the impact are blown off the grid.
    pub blast: f32,
    /// After impact the ball drills on, destroying up to this many further
    /// blocks along its flight line before it spends itself.
    pub pierce: i32,
    /// Visual scale of the ball mesh.
    pub ball_scale: f32,
}

/// Static properties of a block type. Mass feeds into buoyancy and handling
/// once real ship physics lands; color is a placeholder until textures. A
/// color with alpha < 1 renders translucent and casts no shadow, so rigging
/// doesn't wall off the chase camera's view of the sea.
#[allow(dead_code)]
pub struct BlockDef {
    pub name: &'static str,
    pub mass: f32,
    pub color: Color,
    /// If set, this block fires with the ship's broadside, using these
    /// ballistics.
    pub gun: Option<GunDef>,
    /// Salvage price to place one in build mode (removal refunds it).
    pub cost: u32,
    /// PBR metalness: 0 for wood and cloth, high for iron and gold.
    pub metallic: f32,
    /// PBR perceptual roughness: low values catch the sun.
    pub roughness: f32,
    /// Emissive strength (multiplies `color`); > 0 glows through bloom.
    pub emissive: f32,
}

pub fn def(id: BlockId) -> BlockDef {
    match id {
        BlockId::OakHull => BlockDef {
            name: "Oak Hull",
            mass: 40.0,
            color: Color::srgb(0.42, 0.27, 0.15),
            gun: None,
            cost: 1,
            metallic: 0.0,
            roughness: 0.9,
            emissive: 0.0,
        },
        BlockId::IronHull => BlockDef {
            name: "Iron Hull",
            mass: 80.0,
            color: Color::srgb(0.36, 0.38, 0.42),
            gun: None,
            cost: 3,
            metallic: 0.75,
            roughness: 0.5,
            emissive: 0.0,
        },
        BlockId::OakDeck => BlockDef {
            name: "Oak Deck",
            mass: 20.0,
            color: Color::srgb(0.62, 0.45, 0.26),
            gun: None,
            cost: 1,
            metallic: 0.0,
            roughness: 0.85,
            emissive: 0.0,
        },
        BlockId::Mast => BlockDef {
            name: "Mast",
            mass: 15.0,
            color: Color::srgb(0.50, 0.38, 0.24),
            gun: None,
            cost: 2,
            metallic: 0.0,
            roughness: 0.9,
            emissive: 0.0,
        },
        BlockId::Sail => BlockDef {
            name: "Sail",
            mass: 5.0,
            color: Color::srgba(0.93, 0.91, 0.83, 0.62),
            gun: None,
            cost: 2,
            metallic: 0.0,
            roughness: 0.8,
            emissive: 0.0,
        },
        BlockId::Cannon => BlockDef {
            name: "Cannon",
            mass: 120.0,
            color: Color::srgb(0.15, 0.15, 0.17),
            gun: Some(GunDef {
                speed: 22.0,
                blast: 1.6,
                pierce: 0,
                ball_scale: 1.0,
            }),
            cost: 8,
            metallic: 0.8,
            roughness: 0.4,
            emissive: 0.0,
        },
        BlockId::Gold => BlockDef {
            name: "Gold Plunder",
            mass: 60.0,
            color: Color::srgb(0.95, 0.78, 0.22),
            gun: None,
            cost: 5,
            metallic: 1.0,
            roughness: 0.3,
            emissive: 0.0,
        },
        BlockId::Flag => BlockDef {
            name: "Pennant",
            mass: 2.0,
            color: Color::srgb(0.74, 0.11, 0.14),
            gun: None,
            cost: 1,
            metallic: 0.0,
            roughness: 0.8,
            emissive: 0.0,
        },
        // Long bronze gun: a fast, flat shot that drills a line of blocks
        // clean through a hull instead of cratering it.
        BlockId::Culverin => BlockDef {
            name: "Culverin",
            mass: 100.0,
            color: Color::srgb(0.45, 0.36, 0.16),
            gun: Some(GunDef {
                speed: 30.0,
                blast: 0.6,
                pierce: 4,
                ball_scale: 0.7,
            }),
            cost: 12,
            metallic: 0.85,
            roughness: 0.35,
            emissive: 0.0,
        },
        // Stubby smasher: slow and short-ranged, but the hit blows a crater.
        BlockId::Carronade => BlockDef {
            name: "Carronade",
            mass: 90.0,
            color: Color::srgb(0.30, 0.17, 0.13),
            gun: Some(GunDef {
                speed: 15.0,
                blast: 2.8,
                pierce: 0,
                ball_scale: 1.5,
            }),
            cost: 14,
            metallic: 0.7,
            roughness: 0.5,
            emissive: 0.0,
        },
        BlockId::Trim => BlockDef {
            name: "Tarred Trim",
            mass: 30.0,
            color: Color::srgb(0.12, 0.10, 0.08),
            gun: None,
            cost: 1,
            metallic: 0.0,
            roughness: 0.95,
            emissive: 0.0,
        },
        BlockId::Lantern => BlockDef {
            name: "Stern Lantern",
            mass: 10.0,
            color: Color::srgb(0.98, 0.82, 0.45),
            gun: None,
            cost: 2,
            metallic: 0.0,
            roughness: 0.6,
            emissive: 3.0,
        },
    }
}
