//! Procedural, seeded terrain generation and per-biome presentation.
//!
//! Terrain is generic across biomes via [`TerrainKind`]; each [`Biome`] supplies
//! its own colour palette, labels, and noise thresholds.

use noise::{NoiseFn, Perlin};
use ratatui::style::Color;

/// The mechanical role a terrain cell plays for fungus growth. Generic across
/// biomes — the biome decides how each kind looks and is named.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerrainKind {
    /// Normal walkable ground (soil / grass / seafloor / sand).
    Ground,
    /// High-suitability "highway" (tree / shrub / kelp / desert shrub).
    Plant,
    /// Slow going.
    Rock,
    /// Open water — blocking for land fungi, the medium for the marine one.
    Water,
    /// Low-suitability bare patches (cracked clay, etc.).
    Barren,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Biome {
    Forest,
    Grassland,
    Sea,
    Desert,
}

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Self::Forest => "Forest",
            Self::Grassland => "Grassland",
            Self::Sea => "Sea",
            Self::Desert => "Desert",
        }
    }

    /// Background colour for a terrain cell (256-colour palette index).
    pub fn color(self, k: TerrainKind) -> Color {
        let idx = match (self, k) {
            (Self::Forest, TerrainKind::Ground) => 94,  // soil brown
            (Self::Forest, TerrainKind::Plant) => 22,   // dark green tree
            (Self::Forest, TerrainKind::Rock) => 240,   // grey
            (Self::Forest, TerrainKind::Water) => 25,    // blue
            (Self::Forest, TerrainKind::Barren) => 58,   // dim olive

            (Self::Grassland, TerrainKind::Ground) => 28, // green grass
            (Self::Grassland, TerrainKind::Plant) => 64,  // olive shrub
            (Self::Grassland, TerrainKind::Rock) => 243,
            (Self::Grassland, TerrainKind::Water) => 25,
            (Self::Grassland, TerrainKind::Barren) => 143, // tan

            (Self::Sea, TerrainKind::Water) => 18,   // deep blue
            (Self::Sea, TerrainKind::Plant) => 23,   // teal kelp
            (Self::Sea, TerrainKind::Ground) => 180, // sandy seafloor
            (Self::Sea, TerrainKind::Rock) => 239,
            (Self::Sea, TerrainKind::Barren) => 24,

            (Self::Desert, TerrainKind::Ground) => 180, // sand
            (Self::Desert, TerrainKind::Plant) => 100,  // olive shrub
            (Self::Desert, TerrainKind::Rock) => 137,   // brown rock
            (Self::Desert, TerrainKind::Water) => 31,   // oasis
            (Self::Desert, TerrainKind::Barren) => 179, // cracked clay
        };
        Color::Indexed(idx)
    }

    /// Human label for the legend, e.g. "soil", "tree".
    pub fn terrain_label(self, k: TerrainKind) -> &'static str {
        match (self, k) {
            (Self::Forest, TerrainKind::Ground) => "soil",
            (Self::Forest, TerrainKind::Plant) => "tree",
            (Self::Forest, TerrainKind::Rock) => "rock",
            (Self::Forest, TerrainKind::Water) => "water",
            (Self::Forest, TerrainKind::Barren) => "bare",

            (Self::Grassland, TerrainKind::Ground) => "grass",
            (Self::Grassland, TerrainKind::Plant) => "shrub",
            (Self::Grassland, TerrainKind::Rock) => "stone",
            (Self::Grassland, TerrainKind::Water) => "pond",
            (Self::Grassland, TerrainKind::Barren) => "dirt",

            (Self::Sea, TerrainKind::Water) => "water",
            (Self::Sea, TerrainKind::Plant) => "kelp",
            (Self::Sea, TerrainKind::Ground) => "sand",
            (Self::Sea, TerrainKind::Rock) => "reef",
            (Self::Sea, TerrainKind::Barren) => "silt",

            (Self::Desert, TerrainKind::Ground) => "sand",
            (Self::Desert, TerrainKind::Plant) => "shrub",
            (Self::Desert, TerrainKind::Rock) => "rock",
            (Self::Desert, TerrainKind::Water) => "oasis",
            (Self::Desert, TerrainKind::Barren) => "clay",
        }
    }

    /// The three most representative terrain kinds for this biome's legend.
    pub fn legend_kinds(self) -> [TerrainKind; 3] {
        match self {
            Self::Sea => [TerrainKind::Water, TerrainKind::Plant, TerrainKind::Ground],
            _ => [TerrainKind::Ground, TerrainKind::Plant, TerrainKind::Water],
        }
    }
}

/// A static map of terrain for one simulation run.
pub struct TerrainMap {
    pub w: usize,
    pub h: usize,
    pub biome: Biome,
    cells: Vec<TerrainKind>,
}

impl TerrainMap {
    pub fn at(&self, x: usize, y: usize) -> TerrainKind {
        self.cells[y * self.w + x]
    }

    /// Generate coherent terrain regions from two seeded Perlin fields.
    pub fn generate(biome: Biome, w: usize, h: usize, seed: u64) -> Self {
        let elev = Perlin::new(seed as u32);
        let veg = Perlin::new((seed >> 32) as u32 ^ 0x9e37);
        // Cell aspect ~2:1 (taller than wide); stretch X so blobs look round.
        let sx = 0.11;
        let sy = 0.18;

        let mut cells = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let e = norm(elev.get([x as f64 * sx, y as f64 * sy]));
                let v = norm(veg.get([x as f64 * sx + 100.0, y as f64 * sy + 100.0]));
                cells.push(classify(biome, e, v));
            }
        }
        Self { w, h, biome, cells }
    }
}

/// Map noise output (~-1..1) into 0..1.
fn norm(n: f64) -> f32 {
    ((n + 1.0) / 2.0).clamp(0.0, 1.0) as f32
}

/// Turn the two noise samples into a terrain kind for the given biome.
fn classify(biome: Biome, e: f32, v: f32) -> TerrainKind {
    use TerrainKind::*;
    match biome {
        Biome::Forest => {
            if e < 0.22 {
                Water
            } else if e > 0.80 {
                Rock
            } else if v > 0.58 {
                Plant
            } else if e < 0.30 {
                Barren
            } else {
                Ground
            }
        }
        Biome::Grassland => {
            if e < 0.15 {
                Water
            } else if e > 0.86 {
                Rock
            } else if v > 0.62 {
                Plant
            } else if v < 0.18 {
                Barren
            } else {
                Ground
            }
        }
        Biome::Sea => {
            // Water-dominant; sandy seafloor only on the highest "shallows".
            if e > 0.82 {
                Ground
            } else if e > 0.74 {
                Rock
            } else if v > 0.60 {
                Plant // kelp/coral, grows in water
            } else {
                Water
            }
        }
        Biome::Desert => {
            if e < 0.10 {
                Water
            } else if e > 0.84 {
                Rock
            } else if v > 0.70 {
                Plant
            } else if v < 0.40 {
                Barren // cracked clay
            } else {
                Ground // dune sand
            }
        }
    }
}
