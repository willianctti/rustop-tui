mod app;
mod docker;
mod system;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::stdout;
use std::time::{Duration, Instant};

const ANIMATION_INTERVAL: Duration = Duration::from_millis(100);
const METRICS_INTERVAL: Duration = Duration::from_millis(1000);

fn main() -> Result<()> {
    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal);
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    let mut app = App::new()?;
    let mut last_animation = Instant::now();
    let mut last_metrics = Instant::now();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        let timeout = ANIMATION_INTERVAL
            .checked_sub(last_animation.elapsed())
            .unwrap_or(Duration::ZERO);

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(&mut app, key.code);
                }
            }
        }

        if app.should_quit {
            break;
        }

        if last_animation.elapsed() >= ANIMATION_INTERVAL {
            app.on_animation_tick();
            last_animation = Instant::now();
        }

        if last_metrics.elapsed() >= METRICS_INTERVAL {
            app.on_metrics_tick()?;
            last_metrics = Instant::now();
        }
    }

    Ok(())
}

fn handle_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Tab | KeyCode::Right => app.next_tab(),
        KeyCode::Left => app.prev_tab(),
        KeyCode::Down => app.move_selection(1),
        KeyCode::Up => app.move_selection(-1),
        _ => {}
    }
}
