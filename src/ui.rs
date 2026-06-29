//! All ratatui rendering. The UI is "dumb": it reads the app/playback state and
//! the pre-computed frames and paints them. No simulation logic lives here.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Screen};
use crate::sim::{Simulation, Stage};
use crate::terrain::TerrainKind;

const PANEL_W: u16 = 30;
const HINT_H: u16 = 1;
const MIN_GRID_W: usize = 20;
const MIN_GRID_H: usize = 10;

/// "FUNGI" rendered in a clean geometric sans-serif (Chalet/Segoe-style) block.
const TITLE: [&str; 6] = [
    "█████  █   █  █   █  █████  ███",
    "█      █   █  ██  █  █       █ ",
    "████   █   █  █ █ █  █  ██   █ ",
    "█      █   █  █  ██  █   █   █ ",
    "█      █   █  █   █  █   █   █ ",
    "█      █████  █   █  █████  ███",
];

/// Grid dimensions that fit the animation area, or `None` if the terminal is
/// below the minimum playable size.
pub fn grid_dims(width: u16, height: u16) -> Option<(usize, usize)> {
    let gw = width.saturating_sub(PANEL_W + 2) as usize; // panel + grid borders
    let gh = height.saturating_sub(HINT_H + 2) as usize; // hint bar + grid borders
    if gw >= MIN_GRID_W && gh >= MIN_GRID_H {
        Some((gw, gh))
    } else {
        None
    }
}

pub fn render(f: &mut Frame, app: &App) {
    match app.screen {
        Screen::Title => render_title(f, f.area()),
        Screen::Settings => render_settings(f, f.area(), app),
        Screen::Animation => match &app.playback {
            Some(_) => render_animation(f, f.area(), app),
            None => render_center_message(
                f,
                f.area(),
                "Generating simulation…",
                Color::Indexed(214),
            ),
        },
    }
}

fn render_title(f: &mut Frame, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    let pad = (area.height as usize).saturating_sub(14) / 2;
    for _ in 0..pad {
        lines.push(Line::from(""));
    }
    for row in TITLE {
        lines.push(Line::styled(
            row,
            Style::new().fg(Color::Indexed(214)).add_modifier(Modifier::BOLD),
        ));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        "a fungus growth simulator",
        Style::new().fg(Color::Indexed(180)),
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(""));
    lines.push(Line::styled(
        "Enter  start    q  quit",
        Style::new().fg(Color::Indexed(244)),
    ));

    let p = Paragraph::new(lines).alignment(Alignment::Center);
    f.render_widget(p, area);
}

fn render_settings(f: &mut Frame, area: Rect, app: &App) {
    let s = &app.settings;
    let params = s.fungus.params();

    let rows: [(&str, String); 4] = [
        (
            "Fungus",
            format!("{} — {}", params.name, params.biome.name()),
        ),
        ("Aggressiveness", s.aggr.label().to_string()),
        ("Speed", s.speed.label().to_string()),
        ("", "Regenerate map".to_string()),
    ];

    let mut lines: Vec<Line> = vec![Line::from("")];
    for (i, (label, value)) in rows.iter().enumerate() {
        let selected = i == app.selected;
        let marker = if selected { "►" } else { " " };
        let style = if selected {
            Style::new().fg(Color::Indexed(214)).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(Color::Indexed(250))
        };
        let text = if label.is_empty() {
            // The action row.
            format!("  {marker}  [ {value} ]")
        } else if selected {
            format!("  {marker}  {label:<16}<  {value}  >")
        } else {
            format!("  {marker}  {label:<16}   {value}")
        };
        lines.push(Line::styled(text, style));
        lines.push(Line::from(""));
    }

    // Blurb for the selected fungus.
    lines.push(Line::from(""));
    lines.push(Line::styled(
        format!("  {} ({})", params.name, params.scientific),
        Style::new().fg(Color::Indexed(180)),
    ));
    lines.push(Line::styled(
        format!("  {}", params.blurb),
        Style::new().fg(Color::Indexed(244)),
    ));

    if let Some(msg) = &app.message {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            format!("  {msg}"),
            Style::new().fg(Color::Indexed(203)),
        ));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Settings ")
        .title_alignment(Alignment::Center);
    f.render_widget(Paragraph::new(lines).block(block), area);

    let hint = Paragraph::new(Line::styled(
        " ↑/↓ select   ←/→ change   g regenerate   Enter start   Esc back ",
        Style::new().fg(Color::Indexed(244)),
    ));
    let hint_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };
    f.render_widget(hint, hint_area);
}

fn render_animation(f: &mut Frame, area: Rect, app: &App) {
    let pb = app.playback.as_ref().unwrap();
    let sim = &pb.sim;
    let frame_idx = pb.frame_index();
    let complete = pb.complete();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(HINT_H)])
        .split(area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(MIN_GRID_W as u16), Constraint::Length(PANEL_W)])
        .split(outer[0]);

    render_grid(f, cols[0], sim, frame_idx);
    render_panel(f, cols[1], app, frame_idx);

    // Bottom hint bar.
    let hint = if complete {
        " ✓ complete    r replay    Esc settings    q quit "
    } else if pb.paused {
        " ⏸ paused    Space resume    ←/→ scrub    r restart    Esc menu    q quit "
    } else {
        " Space pause    r restart    Esc menu    q quit "
    };
    f.render_widget(
        Paragraph::new(Line::styled(hint, Style::new().fg(Color::Indexed(244)))),
        outer[1],
    );
}

fn render_grid(f: &mut Frame, area: Rect, sim: &Simulation, frame_idx: usize) {
    let block = Block::default().borders(Borders::ALL).title(format!(
        " {} ",
        sim.terrain.biome.name()
    ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let frame = &sim.frames[frame_idx];
    let rows = (inner.height as usize).min(sim.h);
    let cols = (inner.width as usize).min(sim.w);

    let mut lines: Vec<Line> = Vec::with_capacity(rows);
    for y in 0..rows {
        let mut spans: Vec<Span> = Vec::with_capacity(cols);
        for x in 0..cols {
            let terrain = sim.terrain.at(x, y);
            let stage = Stage::from_u8(frame[y * sim.w + x]);
            spans.push(Span::styled(
                stage.glyph().to_string(),
                cell_style(sim, terrain, stage),
            ));
        }
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn cell_style(sim: &Simulation, terrain: TerrainKind, stage: Stage) -> Style {
    let bg = sim.terrain.biome.color(terrain);
    let (dim, normal, bright) = sim.fungus.tint();
    let base = Style::new().bg(bg);
    match stage {
        Stage::Empty => base,
        Stage::Spore => base.fg(Color::Indexed(dim)).add_modifier(Modifier::DIM),
        Stage::Hypha => base.fg(Color::Indexed(normal)),
        Stage::Mature => base.fg(Color::Indexed(bright)),
        Stage::Fruiting => base.fg(Color::Indexed(bright)).add_modifier(Modifier::BOLD),
        Stage::Decay => base.fg(Color::Indexed(244)).add_modifier(Modifier::DIM),
    }
}

fn render_panel(f: &mut Frame, area: Rect, app: &App, frame_idx: usize) {
    let pb = app.playback.as_ref().unwrap();
    let sim = &pb.sim;
    let params = sim.fungus.params();
    let stats = sim.stats(frame_idx);
    let biome = sim.terrain.biome;

    let label = Style::new().fg(Color::Indexed(244));
    let value = Style::new().fg(Color::Indexed(252));
    let head = Style::new().fg(Color::Indexed(214)).add_modifier(Modifier::BOLD);

    let mut lines: Vec<Line> = Vec::new();

    // Identity.
    lines.push(Line::styled(params.name, head));
    lines.push(Line::styled(params.scientific, Style::new().fg(Color::Indexed(180))));
    lines.push(Line::from(vec![
        Span::styled("Biome: ", label),
        Span::styled(biome.name(), value),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::styled(params.blurb, Style::new().fg(Color::Indexed(245))));
    lines.push(Line::from(""));

    // Live stats.
    lines.push(Line::styled("── Live ──", label));
    lines.push(stat_line("Colonised", format!("{:>4.0}%", stats.colonised_pct), label, value));
    lines.push(stat_line("Fruiting", format!("{}", stats.fruiting), label, value));
    lines.push(stat_line("Peak stage", stats.peak.label().to_string(), label, value));
    lines.push(stat_line(
        "Time",
        format!("{} / {}", fmt_time(pb.elapsed_secs()), fmt_time(pb.duration_secs())),
        label,
        value,
    ));
    lines.push(stat_line(
        "Frame",
        format!("{} / {}", frame_idx + 1, sim.frame_count()),
        label,
        value,
    ));

    if pb.complete() {
        lines.push(Line::from(""));
        lines.push(Line::styled("Simulation complete", head));
        let (msg, col) = outcome(sim);
        lines.push(Line::styled(msg, Style::new().fg(col)));
    }

    lines.push(Line::from(""));
    // Legend — terrain.
    lines.push(Line::styled("── Legend ──", label));
    let mut terr_spans: Vec<Span> = Vec::new();
    for k in biome.legend_kinds() {
        terr_spans.push(Span::styled("  ", Style::new().bg(biome.color(k))));
        terr_spans.push(Span::styled(format!(" {} ", biome.terrain_label(k)), value));
    }
    lines.push(Line::from(terr_spans));

    // Legend — stages.
    for chunk in [
        [Stage::Spore, Stage::Hypha, Stage::Mature],
        [Stage::Fruiting, Stage::Decay, Stage::Empty],
    ] {
        let mut spans: Vec<Span> = Vec::new();
        for st in chunk {
            if st == Stage::Empty {
                continue;
            }
            spans.push(Span::styled(
                format!("{} ", st.glyph()),
                Style::new().fg(Color::Indexed(sim.fungus.tint().2)),
            ));
            spans.push(Span::styled(format!("{:<9}", st.label()), value));
        }
        lines.push(Line::from(spans));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Info ");
    f.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn stat_line(label: &str, value: String, lstyle: Style, vstyle: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<11}"), lstyle),
        Span::styled(value, vstyle),
    ])
}

/// Describe how the run ended, from the final frame's coverage.
fn outcome(sim: &Simulation) -> (String, Color) {
    let last = sim.frame_count() - 1;
    let pct = sim.stats(last).colonised_pct;
    if pct < 1.0 {
        ("Failed to establish.".to_string(), Color::Indexed(244))
    } else if pct < 35.0 {
        (format!("Reached {pct:.0}% — a patchy colony."), Color::Indexed(180))
    } else {
        (format!("Reached {pct:.0}% coverage."), Color::Indexed(114))
    }
}

fn fmt_time(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn render_center_message(f: &mut Frame, area: Rect, msg: &str, color: Color) {
    let pad = (area.height as usize).saturating_sub(1) / 2;
    let mut lines: Vec<Line> = Vec::new();
    for _ in 0..pad {
        lines.push(Line::from(""));
    }
    lines.push(Line::styled(msg, Style::new().fg(color).add_modifier(Modifier::BOLD)));
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
}
