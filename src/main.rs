//! Fungi — a terminal fungus-growth simulator.
//!
//! Entry point: set up the terminal, run the app loop, restore on exit.

mod app;
mod fungus;
mod settings;
mod sim;
mod terrain;
mod ui;

use app::App;

fn main() -> std::io::Result<()> {
    let terminal = ratatui::init();
    let result = App::new().run(terminal);
    ratatui::restore();
    result
}
