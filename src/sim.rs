//! Pure, deterministic simulation core: `(settings, size) -> frame history`.
//! No terminal, no ratatui — easy to test in isolation.

use crate::fungus::FungusKind;
use crate::settings::Settings;
use crate::terrain::{TerrainKind, TerrainMap};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Discrete fungal life stage held by each cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Stage {
    Empty = 0,
    Spore = 1,
    Hypha = 2,
    Mature = 3,
    Fruiting = 4,
    Decay = 5,
}

impl Stage {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Spore,
            2 => Self::Hypha,
            3 => Self::Mature,
            4 => Self::Fruiting,
            5 => Self::Decay,
            _ => Self::Empty,
        }
    }

    /// Glyph rendered for this stage (single display width).
    pub fn glyph(self) -> char {
        match self {
            Self::Empty => ' ',
            Self::Spore => '·',
            Self::Hypha => ',',
            Self::Mature => '*',
            Self::Fruiting => 'Y',
            Self::Decay => '˚',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Spore => "spore",
            Self::Hypha => "hypha",
            Self::Mature => "mature",
            Self::Fruiting => "fruiting",
            Self::Decay => "decay",
        }
    }

    fn is_alive(self) -> bool {
        self != Self::Empty
    }

    /// Index into `FungusParams::durations` for the current stage's threshold.
    /// `Decay` is terminal — a spent scar that never advances — so it has none.
    fn duration_idx(self) -> Option<usize> {
        match self {
            Self::Spore => Some(0),
            Self::Hypha => Some(1),
            Self::Mature => Some(2),
            Self::Fruiting => Some(3),
            Self::Decay | Self::Empty => None,
        }
    }

    fn advance(self) -> Self {
        match self {
            Self::Spore => Self::Hypha,
            Self::Hypha => Self::Mature,
            Self::Mature => Self::Fruiting,
            Self::Fruiting => Self::Decay,
            Self::Decay | Self::Empty => self,
        }
    }
}

/// A fully pre-computed run: static terrain plus one stage-snapshot per tick.
pub struct Simulation {
    pub terrain: TerrainMap,
    pub fungus: FungusKind,
    pub w: usize,
    pub h: usize,
    /// `frames[t][y*w+x]` = stage (as u8) of that cell at tick `t`.
    pub frames: Vec<Vec<u8>>,
}

impl Simulation {
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Colonisation %, fruiting-cell count, and the most-advanced stage present.
    pub fn stats(&self, frame: usize) -> FrameStats {
        let f = &self.frames[frame.min(self.frames.len() - 1)];
        let mut growable = 0usize;
        let mut colonised = 0usize;
        let mut fruiting = 0usize;
        let mut peak = Stage::Empty;
        for (i, &s) in f.iter().enumerate() {
            let kind = self.terrain.at(i % self.w, i / self.w);
            if self.fungus.suitability(kind) > 0.0 {
                growable += 1;
            }
            let stage = Stage::from_u8(s);
            if stage.is_alive() {
                colonised += 1;
            }
            if stage == Stage::Fruiting {
                fruiting += 1;
            }
            // Most-advanced stage currently present (Decay is the furthest).
            if s > peak as u8 {
                peak = stage;
            }
        }
        let pct = if growable == 0 {
            0.0
        } else {
            colonised as f32 / growable as f32 * 100.0
        };
        FrameStats {
            colonised_pct: pct,
            fruiting,
            peak,
        }
    }
}

pub struct FrameStats {
    pub colonised_pct: f32,
    pub fruiting: usize,
    pub peak: Stage,
}

const MAX_TICKS: usize = 1500;
const STABLE_LIMIT: usize = 8;

/// Run the whole arc to a terminal state, recording every tick.
pub fn precompute(settings: &Settings, w: usize, h: usize) -> Simulation {
    let fungus = settings.fungus;
    let p = fungus.params();
    let terrain = TerrainMap::generate(p.biome, w, h, settings.seed);
    let mut rng = StdRng::seed_from_u64(settings.seed ^ 0xA5A5_0F0F);

    let n = w * h;
    let mut stage = vec![Stage::Empty; n];
    let mut energy = vec![0.0f32; n];

    inoculate(fungus, &terrain, &mut stage, &mut rng);

    let mut frames: Vec<Vec<u8>> = vec![snapshot(&stage)];
    let mut stable = 0usize;

    for _ in 0..MAX_TICKS {
        let changed = step(settings, &terrain, &mut stage, &mut energy, &mut rng);
        frames.push(snapshot(&stage));

        let alive = stage.iter().any(|s| s.is_alive());
        if !alive {
            break; // never established
        }
        if changed {
            stable = 0;
        } else {
            stable += 1;
            if stable >= STABLE_LIMIT {
                break; // everything has reached terminal decay
            }
        }
    }

    Simulation {
        terrain,
        fungus,
        w,
        h,
        frames,
    }
}

fn snapshot(stage: &[Stage]) -> Vec<u8> {
    stage.iter().map(|s| *s as u8).collect()
}

/// Place the initial fungus according to each species' personality.
fn inoculate(fungus: FungusKind, terrain: &TerrainMap, stage: &mut [Stage], rng: &mut StdRng) {
    let w = terrain.w;
    let h = terrain.h;
    let cx = w / 2;
    let cy = h / 2;
    let growable = |x: usize, y: usize| fungus.suitability(terrain.at(x, y)) > 0.0;

    match fungus {
        FungusKind::FairyRing => {
            // Single origin on suitable grass so the ring expands cleanly.
            if let Some((x, y)) = nearest(terrain, fungus, cx, cy, None) {
                stage[y * w + x] = Stage::Hypha;
            }
        }
        FungusKind::Honey => {
            // Born on a tree near the centre.
            let start = nearest(terrain, fungus, cx, cy, Some(TerrainKind::Plant))
                .or_else(|| nearest(terrain, fungus, cx, cy, None));
            if let Some((x, y)) = start {
                stage[y * w + x] = Stage::Hypha;
            }
        }
        FungusKind::Marine => {
            // A few spores along the left edge, ready to drift right.
            let mut placed = 0;
            for _ in 0..40 {
                if placed >= 3 {
                    break;
                }
                let x = rng.random_range(0..(w / 6).max(1));
                let y = rng.random_range(0..h);
                if growable(x, y) {
                    stage[y * w + x] = Stage::Spore;
                    placed += 1;
                }
            }
        }
        FungusKind::DesertTruffle => {
            // One or two clusters anchored on shrubs.
            let mut clusters = 0;
            let mut tries = 0;
            while clusters < 2 && tries < 60 {
                tries += 1;
                let x = rng.random_range(0..w);
                let y = rng.random_range(0..h);
                if terrain.at(x, y) == TerrainKind::Plant {
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let nx = x as i32 + dx;
                            let ny = y as i32 + dy;
                            if in_bounds(nx, ny, w, h) {
                                let (nx, ny) = (nx as usize, ny as usize);
                                if growable(nx, ny) {
                                    stage[ny * w + nx] = Stage::Hypha;
                                }
                            }
                        }
                    }
                    clusters += 1;
                }
            }
            if clusters == 0 {
                if let Some((x, y)) = nearest(terrain, fungus, cx, cy, None) {
                    stage[y * w + x] = Stage::Hypha;
                }
            }
        }
    }
}

/// Find the suitable cell nearest (cx, cy), optionally restricted to one kind.
fn nearest(
    terrain: &TerrainMap,
    fungus: FungusKind,
    cx: usize,
    cy: usize,
    kind: Option<TerrainKind>,
) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    let mut best_d = i64::MAX;
    for y in 0..terrain.h {
        for x in 0..terrain.w {
            let k = terrain.at(x, y);
            if fungus.suitability(k) <= 0.0 {
                continue;
            }
            if let Some(want) = kind {
                if k != want {
                    continue;
                }
            }
            let dx = x as i64 - cx as i64;
            let dy = y as i64 - cy as i64;
            let d = dx * dx + dy * dy;
            if d < best_d {
                best_d = d;
                best = Some((x, y));
            }
        }
    }
    best
}

fn in_bounds(x: i32, y: i32, w: usize, h: usize) -> bool {
    x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h
}

/// Advance one tick. Returns whether any cell's stage changed.
fn step(
    settings: &Settings,
    terrain: &TerrainMap,
    stage: &mut [Stage],
    energy: &mut [f32],
    rng: &mut StdRng,
) -> bool {
    let p = settings.fungus.params();
    let aggr = settings.aggr.mult();
    let w = terrain.w;
    let h = terrain.h;

    let mut next = stage.to_vec();
    let mut next_energy = energy.to_vec();

    // --- Aging pass: accrue energy by suitability, advance stages. ---
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let s = stage[i];
            if !s.is_alive() {
                continue;
            }
            let suit = settings.fungus.suitability(terrain.at(x, y)).max(0.05);
            let mut gain = suit;

            // Fairy ring: fully-surrounded interior cells die back fast.
            if p.ring && s != Stage::Decay && all_neighbours_alive(stage, x, y, w, h) {
                gain += 3.0;
            }

            next_energy[i] += gain;
            if let Some(di) = s.duration_idx() {
                if next_energy[i] >= p.durations[di] {
                    next[i] = s.advance();
                    next_energy[i] = 0.0;
                }
            }
        }
    }

    // --- Spread pass: hyphae and fruiting bodies seed new spores. ---
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            match stage[i] {
                Stage::Hypha => {
                    // Contact growth into the 8 neighbours.
                    for (dx, dy) in NEIGHBOURS {
                        try_seed(settings, terrain, stage, &mut next, x, y, dx, dy, aggr, 1.0, rng);
                    }
                    // Honey: occasional long-range rhizomorph tendril.
                    if p.tendril && rng.random::<f32>() < 0.06 * aggr {
                        let (dx, dy) = NEIGHBOURS[rng.random_range(0..8)];
                        try_seed(settings, terrain, stage, &mut next, x, y, dx * 2, dy * 2, aggr, 0.9, rng);
                    }
                }
                Stage::Fruiting => {
                    let r = p.fruiting_reach;
                    for dy in -r..=r {
                        for dx in -r..=r {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            try_seed(settings, terrain, stage, &mut next, x, y, dx, dy, aggr, 0.55, rng);
                        }
                    }
                    // Marine: extra drift-biased dispersal downstream.
                    if let Some((ddx, ddy)) = p.drift {
                        for step in 1..=p.fruiting_reach {
                            try_seed(
                                settings, terrain, stage, &mut next, x, y, ddx * step, ddy * step,
                                aggr, 0.9, rng,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let changed = next != *stage;
    stage.copy_from_slice(&next);
    energy.copy_from_slice(&next_energy);
    changed
}

const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1), (0, -1), (1, -1),
    (-1, 0), (1, 0),
    (-1, 1), (0, 1), (1, 1),
];

fn all_neighbours_alive(stage: &[Stage], x: usize, y: usize, w: usize, h: usize) -> bool {
    for (dx, dy) in NEIGHBOURS {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if !in_bounds(nx, ny, w, h) {
            return false;
        }
        if !stage[ny as usize * w + nx as usize].is_alive() {
            return false;
        }
    }
    true
}

/// Attempt to colonise the empty cell at (x+dx, y+dy) from a source at (x, y).
#[allow(clippy::too_many_arguments)]
fn try_seed(
    settings: &Settings,
    terrain: &TerrainMap,
    stage: &[Stage],
    next: &mut [Stage],
    x: usize,
    y: usize,
    dx: i32,
    dy: i32,
    aggr: f32,
    factor: f32,
    rng: &mut StdRng,
) {
    let nx = x as i32 + dx;
    let ny = y as i32 + dy;
    if !in_bounds(nx, ny, terrain.w, terrain.h) {
        return;
    }
    let (nx, ny) = (nx as usize, ny as usize);
    let ni = ny * terrain.w + nx;
    // Only colonise cells that are empty both now and after this tick's aging.
    // (Decay is terminal and counts as alive, so scars block recolonisation.)
    if stage[ni].is_alive() || next[ni].is_alive() {
        return;
    }
    let suit = settings.fungus.suitability(terrain.at(nx, ny));
    if suit <= 0.0 {
        return;
    }
    let prob = (settings.fungus.params().spread_prob * aggr * suit.min(1.0) * factor).min(0.95);
    if rng.random::<f32>() < prob {
        next[ni] = Stage::Spore;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Aggressiveness, Settings, Speed};
    use crate::terrain::Biome;

    fn settings(fungus: FungusKind, seed: u64) -> Settings {
        Settings {
            fungus,
            aggr: Aggressiveness::Moderate,
            speed: Speed::One,
            seed,
        }
    }

    /// Dev-only visual sanity check:
    /// `cargo test dump_overview -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dump_overview() {
        for fungus in FungusKind::all() {
            let s = settings(fungus, 2024);
            let sim = precompute(&s, 72, 32);
            let n = sim.frame_count();
            print!("\n=== {} ({} frames) coverage: ", fungus.params().name, n);
            for q in 0..=4 {
                let f = (q * (n - 1)) / 4;
                print!("{:.0}% ", sim.stats(f).colonised_pct);
            }
            println!();
            // Final frame, downsampled glyphs.
            let last = &sim.frames[n - 1];
            for y in (0..sim.h).step_by(2) {
                let row: String = (0..sim.w)
                    .step_by(2)
                    .map(|x| Stage::from_u8(last[y * sim.w + x]).glyph())
                    .collect();
                println!("{row}");
            }
        }
    }

    #[test]
    fn terrain_is_deterministic_from_seed() {
        let a = TerrainMap::generate(Biome::Forest, 40, 24, 99);
        let b = TerrainMap::generate(Biome::Forest, 40, 24, 99);
        for y in 0..24 {
            for x in 0..40 {
                assert_eq!(a.at(x, y), b.at(x, y));
            }
        }
    }

    #[test]
    fn precompute_is_deterministic_from_seed() {
        let s = settings(FungusKind::Honey, 7);
        let a = precompute(&s, 50, 30);
        let b = precompute(&s, 50, 30);
        assert_eq!(a.frames, b.frames);
    }

    #[test]
    fn fungus_actually_spreads() {
        // Every species should colonise more than its inoculation seed.
        for fungus in FungusKind::all() {
            let s = settings(fungus, 12345);
            let sim = precompute(&s, 60, 36);
            let start = sim.frames.first().unwrap().iter().filter(|&&v| v != 0).count();
            let peak = sim
                .frames
                .iter()
                .map(|f| f.iter().filter(|&&v| v != 0).count())
                .max()
                .unwrap();
            assert!(
                peak > start,
                "{} did not spread (start {start}, peak {peak})",
                fungus.params().name
            );
        }
    }

    #[test]
    fn reaches_a_terminal_state() {
        // Should stop well before the hard cap (settled or died off).
        let s = settings(FungusKind::FairyRing, 555);
        let sim = precompute(&s, 60, 36);
        assert!(sim.frame_count() < MAX_TICKS, "ran to the tick cap");
        assert!(sim.frame_count() > 2, "ended implausibly early");
    }

    #[test]
    fn fairy_ring_centre_dies_back() {
        // The ring mechanic must vacate interior cells: at the busiest frame
        // there should exist an empty cell with living cells on opposite sides.
        let s = settings(FungusKind::FairyRing, 2024);
        let sim = precompute(&s, 70, 40);
        let (busiest, _) = sim
            .frames
            .iter()
            .enumerate()
            .max_by_key(|(_, f)| f.iter().filter(|&&v| v != 0).count())
            .unwrap();
        let f = &sim.frames[busiest];
        let (w, h) = (sim.w, sim.h);
        let mut found_hole = false;
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let alive = |xx: usize, yy: usize| f[yy * w + xx] != 0;
                if !alive(x, y)
                    && alive(x - 1, y)
                    && alive(x + 1, y)
                    && alive(x, y - 1)
                    && alive(x, y + 1)
                {
                    found_hole = true;
                }
            }
        }
        assert!(found_hole, "fairy ring never hollowed out a centre");
    }
}
