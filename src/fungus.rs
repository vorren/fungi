//! The four fungus types. One shared engine; each species is a bundle of
//! parameters plus a few signature-behaviour flags consumed by the sim.

use crate::terrain::{Biome, TerrainKind};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FungusKind {
    Honey,
    FairyRing,
    Marine,
    DesertTruffle,
}

/// Tuning + behaviour flags for one species.
pub struct FungusParams {
    pub name: &'static str,
    pub scientific: &'static str,
    pub blurb: &'static str,
    pub biome: Biome,
    /// Base probability a hypha colonises an adjacent suitable empty cell.
    pub spread_prob: f32,
    /// Radius (in cells) that fruiting bodies disperse spores.
    pub fruiting_reach: i32,
    /// Honey: occasionally fling a spore two cells out (rhizomorph tendril).
    pub tendril: bool,
    /// Fairy ring: fully-surrounded interior cells die back fast (hollow ring).
    pub ring: bool,
    /// Marine: spores drift along this direction.
    pub drift: Option<(i32, i32)>,
    /// Energy thresholds to leave each stage:
    /// [Spore→Hypha, Hypha→Mature, Mature→Fruiting, Fruiting→Decay, Decay→Empty].
    pub durations: [f32; 5],
}

impl FungusKind {
    pub fn all() -> [FungusKind; 4] {
        [
            Self::Honey,
            Self::FairyRing,
            Self::Marine,
            Self::DesertTruffle,
        ]
    }

    pub fn params(self) -> FungusParams {
        match self {
            Self::Honey => FungusParams {
                name: "Honey Fungus",
                scientific: "Armillaria",
                blurb: "Reaching rhizomorph tendrils; the largest organism on Earth.",
                biome: Biome::Forest,
                spread_prob: 0.20,
                fruiting_reach: 2,
                tendril: true,
                ring: false,
                drift: None,
                durations: [3.0, 5.0, 7.0, 5.0, 4.0],
            },
            Self::FairyRing => FungusParams {
                name: "Fairy Ring",
                scientific: "Marasmius oreades",
                blurb: "Expands as a ring; the centre dies back as the edge advances.",
                biome: Biome::Grassland,
                spread_prob: 0.30,
                fruiting_reach: 2,
                tendril: false,
                ring: true,
                drift: None,
                durations: [2.0, 4.0, 5.0, 3.0, 2.0],
            },
            Self::Marine => FungusParams {
                name: "Marine Mycelium",
                scientific: "Corollospora sp.",
                blurb: "Lives on kelp & seafloor; spores drift with the current.",
                biome: Biome::Sea,
                spread_prob: 0.18,
                fruiting_reach: 3,
                tendril: false,
                ring: false,
                drift: Some((1, 0)),
                durations: [3.0, 5.0, 6.0, 5.0, 4.0],
            },
            Self::DesertTruffle => FungusParams {
                name: "Desert Truffle",
                scientific: "Terfezia",
                blurb: "Slow, patchy growth clustered around shrub roots.",
                biome: Biome::Desert,
                spread_prob: 0.10,
                fruiting_reach: 1,
                tendril: false,
                ring: false,
                drift: None,
                durations: [5.0, 8.0, 11.0, 8.0, 5.0],
            },
        }
    }

    /// Growth-rate / spread multiplier for a terrain kind. `0.0` means the
    /// fungus cannot live there (spread is blocked).
    pub fn suitability(self, k: TerrainKind) -> f32 {
        use TerrainKind::*;
        match self {
            Self::Honey => match k {
                Plant => 1.6,
                Ground => 1.0,
                Barren => 0.5,
                Rock => 0.3,
                Water => 0.0,
            },
            Self::FairyRing => match k {
                Ground => 1.2,
                Plant => 1.3,
                Barren => 0.4,
                Rock => 0.2,
                Water => 0.0,
            },
            // Marine flips the script: open water is a home, not a wall.
            Self::Marine => match k {
                Plant => 1.6,
                Water => 1.1,
                Ground => 0.6,
                Barren => 0.3,
                Rock => 0.2,
            },
            Self::DesertTruffle => match k {
                Plant => 1.8,
                Barren => 0.6,
                Ground => 0.5,
                Rock => 0.2,
                Water => 0.0,
            },
        }
    }

    /// Signature foreground tint as (dim, normal, bright) 256-palette indices.
    pub fn tint(self) -> (u8, u8, u8) {
        match self {
            Self::Honey => (94, 172, 214),         // amber
            Self::FairyRing => (143, 185, 229),     // pale gold
            Self::Marine => (30, 37, 51),           // teal/cyan
            Self::DesertTruffle => (97, 134, 171),  // violet
        }
    }
}
