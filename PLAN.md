# Fungi — TUI Fungus Growth Simulator

A terminal application (Rust + `ratatui`) that simulates and animates fungal
growth across procedurally generated terrain, plants, and biomes. Each run is
pre-computed and played back, paced to fit a ~60-second window.

## Stack

- **Language:** Rust
- **TUI:** `ratatui` + `crossterm`
- **Terrain noise:** `noise` crate (Perlin/Simplex)
- **RNG:** `rand` with a seedable PRNG (deterministic from a seed)
- Ships as a single binary.

## Screens

1. **Title** — clean sans-serif (Chalet/Segoe-style) block ASCII rendering of
   "Fungi" + tagline. `Enter` → Settings, `q` → quit.
2. **Settings** — Fungus type (also fixes biome), Aggressiveness
   (Mild / Moderate / Rampant), Speed (0.5× / 1× / 2×), and a Regenerate-map
   action. `↑↓` move rows, `←→` change value, `g` regenerate map, `Enter` start,
   `Esc` back to Title.
3. **Animation** — grid on the left, info panel on the right, control hints on
   the bottom bar. `Space` pause, `←→` scrub while paused, `r` restart,
   `Esc` back to Settings, `q` quit. Holds the final frame + summary on
   completion (replay `r` / back `Esc` / quit `q`). No auto-loop.

## Simulation model

2D grid cellular automaton. Each cell = a **static terrain type** + a
**discrete fungus stage**:

```
Empty → Spore → Hypha → Mature → Fruiting → Decay
```

- Each cell has an internal **age/energy counter**; it advances a stage when the
  counter crosses a threshold.
- Advancement speed is **scaled by terrain suitability** (good terrain advances
  fast; hostile terrain stalls or dies).
- **Hypha** cells infect adjacent **Empty** cells (short-range contact growth).
- **Fruiting** cells disperse spores at slightly longer range.
- **Aggressiveness** controls spread probability + dispersal reach (it changes
  the *pattern* and arc length, not the watch speed).
- Neighborhood: **Moore (8-way)** for organic-looking spread.
- The simulation reaches a **terminal state** — full colonization then die-back
  to a stable scattering, or total die-off.

## Four fungus types (one shared engine, signature behaviors via parameters)

| Fungus | Biome | Signature behavior |
|---|---|---|
| **Honey Fungus** (*Armillaria*) | Forest | Reaching rhizomorph tendrils / long-range network spread |
| **Fairy Ring** (*Marasmius oreades*) | Grassland | Expanding ring; center dies back as the edge advances |
| **Marine Mycelium** (*Corollospora*-type) | Sea | Spores drift with a current → directional spread bias |
| **Desert Truffle** (*Terfezia*) | Desert | Slow, patchy, clustered around shrub/plant roots |

Behaviors implemented as small rule/parameter variations on one engine:
directional-bias vector (Marine), kill-the-center flag (Fairy), prefer-plant
weighting (Desert), long-range tendril reach (Honey).

## Terrain

- **One biome per simulation** (implied by the chosen fungus).
- Generated from **seeded Perlin/Simplex noise** so terrain comes in coherent
  regions (clustered water, plant patches, veins of rock) — not random static.
- Each biome supplies its own palette + thresholds (forest / grassland / sea /
  desert).
- **Plants are high-suitability spread highways** (fungus races through them);
  rock is slow; open water blocks spread (except Marine).
- **Per-fungus seeded inoculation:** Fairy Ring from a single grass point; Honey
  from one tree; Marine from a few spores along one edge (drifting inward);
  Desert Truffle from 1–2 shrub-anchored clusters.
- The seed drives both reproducibility and the Regenerate-map action.

## Time & pacing

- Biology is **real and tick-based**; aggressiveness/terrain/type determine how
  many ticks the arc takes.
- The **full arc is pre-computed** into a frame history on "Start" (with a
  brief "Generating simulation…" indicator).
- Playback is **paced to a Speed-scaled ~60s target** (0.5× ≈ 120s, 1× ≈ 60s,
  2× ≈ 30s) by mapping the fixed frame history onto the target window
  (skip/hold frames as needed). Guarantees the fit and makes pause / scrub /
  restart trivial.

## Visual encoding (256-color, glyph-first)

- **Background color = terrain** (always visible underneath the fungus).
- **Glyph = fungus stage:** `·` spore, `,` hypha, `*` mature, `Y` fruiting,
  `˚` decay, ` ` empty.
- **Foreground color = fungus-type tint + brightness by vitality** (bright at
  fruiting, dim/desaturated at decay).
- **Glyph-first**: stage is always distinguishable by character, so it degrades
  gracefully and stays colorblind-legible; color is reinforcement.
- Target **256-color** for near-universal terminal support.

## Rendering & performance

- Grid **auto-fits the terminal's left region** at startup, with a **minimum
  size guard** ("please enlarge your terminal" below the minimum).
- **Terrain stored once** (static); each pre-computed frame stores only the
  **fungus stage per cell (1 byte)** — a few MB at most.
- Playback renders at a capped **~20–30 fps**, paced to the target duration.
- **Resize** affects the next run only (mid-run resize re-centers/letterboxes).

## Code structure

Pure simulation core, thin UI layer:

- `main.rs` — terminal setup/teardown, top-level loop.
- `app.rs` — screen state machine (Title / Settings / Animation), input.
- `sim/` — `cell`, `stage`, `grid`, `tick`, and the precompute →
  frame-history entry point. **Pure, deterministic, no UI.**
- `terrain.rs` — seeded noise generation + per-biome palettes/thresholds.
- `fungus.rs` — the 4 type definitions (params + signature-behavior flags).
- `ui/` — ratatui rendering per screen + legend/info panel.
- `settings.rs` — config struct.

Plus a handful of engine unit tests: seed determinism, spread actually happens,
fairy-ring center dies, sim reaches a terminal state.
