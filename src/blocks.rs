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
}

/// Every block id, for systems that need to pre-build per-block assets.
/// Keep in sync with [`BlockId`] when adding blocks.
pub const ALL: [BlockId; 6] = [
    BlockId::OakHull,
    BlockId::IronHull,
    BlockId::OakDeck,
    BlockId::Mast,
    BlockId::Sail,
    BlockId::Cannon,
];

/// Static properties of a block type. Mass feeds into buoyancy and handling
/// once real ship physics lands; color is a placeholder until textures.
#[allow(dead_code)]
pub struct BlockDef {
    pub name: &'static str,
    pub mass: f32,
    pub color: Color,
    /// Whether this block fires cannonballs when the ship's broadside fires.
    pub gun: bool,
}

pub fn def(id: BlockId) -> BlockDef {
    match id {
        BlockId::OakHull => BlockDef {
            name: "Oak Hull",
            mass: 40.0,
            color: Color::srgb(0.42, 0.27, 0.15),
            gun: false,
        },
        BlockId::IronHull => BlockDef {
            name: "Iron Hull",
            mass: 80.0,
            color: Color::srgb(0.36, 0.38, 0.42),
            gun: false,
        },
        BlockId::OakDeck => BlockDef {
            name: "Oak Deck",
            mass: 20.0,
            color: Color::srgb(0.62, 0.45, 0.26),
            gun: false,
        },
        BlockId::Mast => BlockDef {
            name: "Mast",
            mass: 15.0,
            color: Color::srgb(0.50, 0.38, 0.24),
            gun: false,
        },
        BlockId::Sail => BlockDef {
            name: "Sail",
            mass: 5.0,
            color: Color::srgb(0.93, 0.91, 0.83),
            gun: false,
        },
        BlockId::Cannon => BlockDef {
            name: "Cannon",
            mass: 120.0,
            color: Color::srgb(0.15, 0.15, 0.17),
            gun: true,
        },
    }
}
