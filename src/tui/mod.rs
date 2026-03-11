mod app;
mod input;
mod ui;

use std::io;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::store::types::load_store;

use self::app::App;

type Term = Terminal<CrosstermBackend<io::Stdout>>;

pub fn run() -> Result<()> {
    let path = crate::paths::store_path()?;
    let store = load_store(&path)?;
    let mut app = App::new(store, path);

    enable_raw_mode()?;
    crossterm::execute!(io::stdout(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    crossterm::execute!(io::stdout(), LeaveAlternateScreen)?;

    result
}

fn event_loop(terminal: &mut Term, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            input::handle_key(app, key);
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
