use bevy::prelude::*;

/// Every placeable block in the game.
///
/// Gameplay systems must only read block properties through [`def`], never
/// match on the id directly — that keeps "add a new block" a two-line change.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BlockId {
    OakHull,
    OakDeck,
    Mast,
    Cannon,
}

/// Static properties of a block type. Mass feeds into buoyancy and handling
/// once real ship physics lands; color is a placeholder until textures.
#[allow(dead_code)]
pub struct BlockDef {
    pub name: &'static str,
    pub mass: f32,
    pub color: Color,
}

pub fn def(id: BlockId) -> BlockDef {
    match id {
        BlockId::OakHull => BlockDef {
            name: "Oak Hull",
            mass: 40.0,
            color: Color::srgb(0.42, 0.27, 0.15),
        },
        BlockId::OakDeck => BlockDef {
            name: "Oak Deck",
            mass: 20.0,
            color: Color::srgb(0.62, 0.45, 0.26),
        },
        BlockId::Mast => BlockDef {
            name: "Mast",
            mass: 15.0,
            color: Color::srgb(0.50, 0.38, 0.24),
        },
        BlockId::Cannon => BlockDef {
            name: "Cannon",
            mass: 120.0,
            color: Color::srgb(0.15, 0.15, 0.17),
        },
    }
}
