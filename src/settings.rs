//! User-facing configuration for a simulation run.

use crate::fungus::FungusKind;

/// How hard the fungus pushes outward. Changes the *pattern* and arc length,
/// not the playback speed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Aggressiveness {
    Mild,
    Moderate,
    Rampant,
}

impl Aggressiveness {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mild => "Mild",
            Self::Moderate => "Moderate",
            Self::Rampant => "Rampant",
        }
    }

    /// Multiplier applied to spread probability and dispersal.
    pub fn mult(self) -> f32 {
        match self {
            Self::Mild => 0.6,
            Self::Moderate => 1.0,
            Self::Rampant => 1.55,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Mild => Self::Moderate,
            Self::Moderate => Self::Rampant,
            Self::Rampant => Self::Mild,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Mild => Self::Rampant,
            Self::Moderate => Self::Mild,
            Self::Rampant => Self::Moderate,
        }
    }
}

/// Playback watch-rate. Stretches or compresses the ~60s target window.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    Half,
    One,
    Two,
}

impl Speed {
    pub fn label(self) -> &'static str {
        match self {
            Self::Half => "0.5x",
            Self::One => "1x",
            Self::Two => "2x",
        }
    }

    /// Target playback duration in seconds (Speed-scaled ~60s).
    pub fn target_secs(self) -> f32 {
        match self {
            Self::Half => 120.0,
            Self::One => 60.0,
            Self::Two => 30.0,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Half => Self::One,
            Self::One => Self::Two,
            Self::Two => Self::Half,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Half => Self::Two,
            Self::One => Self::Half,
            Self::Two => Self::One,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Settings {
    pub fungus: FungusKind,
    pub aggr: Aggressiveness,
    pub speed: Speed,
    /// Drives both terrain generation and the per-fungus inoculation.
    pub seed: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fungus: FungusKind::Honey,
            aggr: Aggressiveness::Moderate,
            speed: Speed::One,
            seed: 0x5eed_1234_abcd_0001,
        }
    }
}

impl Settings {
    /// Reroll the terrain seed (the "Regenerate map" action).
    pub fn reroll(&mut self) {
        // A cheap deterministic LCG step — no wall-clock entropy needed.
        self.seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
    }
}
