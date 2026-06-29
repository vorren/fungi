//! Top-level screen state machine, input handling, and the render/event loop.

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::settings::Settings;
use crate::sim::{precompute, Simulation};
use crate::ui;

const POLL: Duration = Duration::from_millis(33); // ~30 fps

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Title,
    Settings,
    Animation,
}

/// Timed playback over a pre-computed frame history.
pub struct Playback {
    pub sim: Simulation,
    duration: Duration,
    elapsed: Duration,
    pub paused: bool,
    last: Instant,
}

impl Playback {
    fn new(sim: Simulation, target_secs: f32) -> Self {
        Self {
            sim,
            duration: Duration::from_secs_f32(target_secs.max(1.0)),
            elapsed: Duration::ZERO,
            paused: false,
            last: Instant::now(),
        }
    }

    /// Advance the playback clock once per loop iteration.
    fn tick_clock(&mut self) {
        let now = Instant::now();
        if !self.paused && !self.complete() {
            self.elapsed += now - self.last;
        }
        self.last = now;
    }

    pub fn complete(&self) -> bool {
        self.elapsed >= self.duration
    }

    pub fn elapsed_secs(&self) -> f32 {
        self.elapsed.as_secs_f32().min(self.duration.as_secs_f32())
    }

    pub fn duration_secs(&self) -> f32 {
        self.duration.as_secs_f32()
    }

    /// Map elapsed time onto the fixed frame history.
    pub fn frame_index(&self) -> usize {
        let n = self.sim.frame_count();
        if n <= 1 {
            return 0;
        }
        let t = (self.elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0);
        ((t * (n - 1) as f32).round() as usize).min(n - 1)
    }

    fn restart(&mut self) {
        self.elapsed = Duration::ZERO;
        self.paused = false;
        self.last = Instant::now();
    }

    /// Scrub by a number of frames (used while paused).
    fn scrub(&mut self, delta: i64) {
        let n = self.sim.frame_count();
        if n <= 1 {
            return;
        }
        let cur = self.frame_index() as i64;
        let target = (cur + delta).clamp(0, n as i64 - 1) as f32;
        let t = target / (n - 1) as f32;
        self.elapsed = self.duration.mul_f32(t);
    }
}

pub struct App {
    pub screen: Screen,
    pub settings: Settings,
    pub selected: usize,
    pub playback: Option<Playback>,
    pub message: Option<String>,
    pending_start: bool,
    should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Title,
            settings: Settings::default(),
            selected: 0,
            playback: None,
            message: None,
            pending_start: false,
            should_quit: false,
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        while !self.should_quit {
            // Draw a "Generating…" frame, then synchronously build the run.
            if self.pending_start {
                terminal.draw(|f| ui::render(f, &self))?;
                let size = terminal.size()?;
                self.start_simulation(size.width, size.height);
                self.pending_start = false;
                continue;
            }

            if let Some(pb) = self.playback.as_mut() {
                pb.tick_clock();
            }
            terminal.draw(|f| ui::render(f, &self))?;

            if event::poll(POLL)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        let size = terminal.size()?;
                        self.handle_key(key.code, size.width, size.height);
                    }
                }
            }
        }
        Ok(())
    }

    fn start_simulation(&mut self, width: u16, height: u16) {
        match ui::grid_dims(width, height) {
            Some((w, h)) => {
                let sim = precompute(&self.settings, w, h);
                self.playback = Some(Playback::new(sim, self.settings.speed.target_secs()));
            }
            None => {
                // Shouldn't happen (guarded on Enter), but stay safe.
                self.screen = Screen::Settings;
                self.message = Some("Terminal too small — please enlarge.".into());
            }
        }
    }

    fn handle_key(&mut self, code: KeyCode, width: u16, height: u16) {
        // `q` quits from anywhere.
        if matches!(code, KeyCode::Char('q')) {
            self.should_quit = true;
            return;
        }

        match self.screen {
            Screen::Title => {
                if matches!(code, KeyCode::Enter) {
                    self.screen = Screen::Settings;
                    self.message = None;
                }
            }
            Screen::Settings => self.handle_settings_key(code, width, height),
            Screen::Animation => self.handle_animation_key(code),
        }
    }

    fn handle_settings_key(&mut self, code: KeyCode, width: u16, height: u16) {
        match code {
            KeyCode::Up => {
                self.selected = (self.selected + 3) % 4;
            }
            KeyCode::Down => {
                self.selected = (self.selected + 1) % 4;
            }
            KeyCode::Left => self.change_setting(false),
            KeyCode::Right => self.change_setting(true),
            KeyCode::Char('g') => {
                self.settings.reroll();
                self.message = Some("Map regenerated.".into());
            }
            KeyCode::Enter => {
                // Row 3 is the regenerate action; others + Enter start the run.
                if self.selected == 3 {
                    self.settings.reroll();
                    self.message = Some("Map regenerated.".into());
                } else if ui::grid_dims(width, height).is_some() {
                    self.screen = Screen::Animation;
                    self.playback = None;
                    self.pending_start = true;
                    self.message = None;
                } else {
                    self.message = Some("Terminal too small — please enlarge.".into());
                }
            }
            KeyCode::Esc => {
                self.screen = Screen::Title;
                self.message = None;
            }
            _ => {}
        }
    }

    fn change_setting(&mut self, forward: bool) {
        self.message = None;
        match self.selected {
            0 => self.cycle_fungus(forward),
            1 => {
                self.settings.aggr = if forward {
                    self.settings.aggr.next()
                } else {
                    self.settings.aggr.prev()
                }
            }
            2 => {
                self.settings.speed = if forward {
                    self.settings.speed.next()
                } else {
                    self.settings.speed.prev()
                }
            }
            _ => {}
        }
    }

    fn cycle_fungus(&mut self, forward: bool) {
        let all = crate::fungus::FungusKind::all();
        let idx = all.iter().position(|f| *f == self.settings.fungus).unwrap_or(0);
        let next = if forward {
            (idx + 1) % all.len()
        } else {
            (idx + all.len() - 1) % all.len()
        };
        self.settings.fungus = all[next];
    }

    fn handle_animation_key(&mut self, code: KeyCode) {
        let Some(pb) = self.playback.as_mut() else {
            return;
        };
        match code {
            KeyCode::Char(' ') => pb.paused = !pb.paused,
            KeyCode::Char('r') => pb.restart(),
            KeyCode::Left if pb.paused => pb.scrub(-1),
            KeyCode::Right if pb.paused => pb.scrub(1),
            KeyCode::Esc => {
                self.screen = Screen::Settings;
                self.playback = None;
            }
            _ => {}
        }
    }
}
