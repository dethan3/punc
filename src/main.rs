mod app;
mod diff;
mod editor;
mod highlight;
mod ui;
mod watcher;

use std::env;
use std::io;
use std::path::Path;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, Mode};
use watcher::FileWatcher;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: punc <file.md>");
        std::process::exit(1);
    }
    let path = Path::new(&args[1]);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(path)?;
    let file_watcher = FileWatcher::new(path.canonicalize().unwrap_or(path.to_path_buf()))
        .ok();

    // Main event loop
    let result = run_loop(&mut terminal, &mut app, file_watcher.as_ref());

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    watcher: Option<&FileWatcher>,
) -> io::Result<()> {
    loop {
        // Render
        terminal.draw(|f| ui::render(f, app))?;

        if app.should_quit {
            return Ok(());
        }

        // Check file watcher
        if let Some(w) = watcher {
            if w.poll() {
                if let Ok(content) = std::fs::read_to_string(&w.path) {
                    app.handle_external_change(content);
                }
            }
        }

        // Wait for event
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.message = None;
                handle_key(app, key);
            }
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) {
    match app.mode {
        Mode::Edit => handle_edit_key(app, key),
        Mode::Preview => handle_preview_key(app, key),
        Mode::Outline => handle_outline_key(app, key),
        Mode::Diff => handle_diff_key(app, key),
    }
}

fn handle_edit_key(app: &mut App, key: KeyEvent) {
    match (key.modifiers, key.code) {
        // Ctrl combos
        (KeyModifiers::ALT, KeyCode::Char('q')) => {
            app.should_quit = true;
        }
        (KeyModifiers::ALT, KeyCode::Char('s')) => {
            match app.buffer.save() {
                Ok(()) => app.message = Some("Saved".to_string()),
                Err(e) => app.message = Some(format!("Save failed: {}", e)),
            }
        }
        (KeyModifiers::ALT, KeyCode::Char('p')) => {
            app.preview_scroll = 0;
            app.mode = Mode::Preview;
        }
        (KeyModifiers::ALT, KeyCode::Char('z')) => {
            app.buffer.undo();
        }
        (KeyModifiers::ALT, KeyCode::Char('y')) => {
            app.buffer.redo();
        }
        (KeyModifiers::ALT, KeyCode::Char('v')) => {
            if let Ok(clip) = get_clipboard() {
                app.buffer.paste(&clip);
            }
        }

        // Navigation
        (_, KeyCode::Up) => app.buffer.cursor.move_up(&app.buffer.rope),
        (_, KeyCode::Down) => app.buffer.cursor.move_down(&app.buffer.rope),
        (_, KeyCode::Left) => app.buffer.cursor.move_left(&app.buffer.rope),
        (_, KeyCode::Right) => app.buffer.cursor.move_right(&app.buffer.rope),
        (_, KeyCode::Home) => app.buffer.cursor.move_home(),
        (_, KeyCode::End) => app.buffer.cursor.move_end(&app.buffer.rope),
        (_, KeyCode::PageUp) => {
            if let Ok((_, rows)) = crossterm::terminal::size() {
                app.buffer.page_up(rows.saturating_sub(2) as usize);
            }
        }
        (_, KeyCode::PageDown) => {
            if let Ok((_, rows)) = crossterm::terminal::size() {
                app.buffer.page_down(rows.saturating_sub(2) as usize);
            }
        }

        (KeyModifiers::ALT, KeyCode::Char('o')) => {
            app.build_outline();
            app.mode = Mode::Outline;
        }
        (KeyModifiers::ALT, KeyCode::Char('d')) => {
            if app.external_change {
                app.open_diff();
            }
        }
        (_, KeyCode::Esc) => {}

        // Editing
        (_, KeyCode::Char(ch)) => app.buffer.insert_char(ch),
        (_, KeyCode::Enter) => app.buffer.insert_char('\n'),
        (_, KeyCode::Backspace) => app.buffer.backspace(),
        (_, KeyCode::Delete) => app.buffer.delete(),
        (_, KeyCode::Tab) => {
            // Insert 4 spaces for tab
            for _ in 0..4 {
                app.buffer.insert_char(' ');
            }
        }

        _ => {}
    }

    // Adjust scroll after cursor movement
    if let Ok((_, rows)) = crossterm::terminal::size() {
        let editor_height = rows.saturating_sub(2) as usize; // minus status + hint bars
        app.buffer.adjust_scroll(editor_height);
    }
}

fn get_clipboard() -> Result<String, ()> {
    // Try reading from terminal paste bracket (crossterm handles this),
    // fallback: try system clipboard via CLI tools
    use std::process::Command;
    // Try xclip first, then xsel, then pbpaste (macOS)
    for cmd in &["xclip -selection clipboard -o", "xsel --clipboard --output", "pbpaste"] {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if let Ok(output) = Command::new(parts[0]).args(&parts[1..]).output() {
            if output.status.success() {
                return String::from_utf8(output.stdout).map_err(|_| ());
            }
        }
    }
    Err(())
}

fn handle_diff_key(app: &mut App, key: KeyEvent) {
    match (key.modifiers, key.code) {
        (_, KeyCode::Char('a')) | (_, KeyCode::Char('A')) => {
            app.accept_external();
            app.message = Some("External changes accepted".to_string());
        }
        (_, KeyCode::Char('r')) | (_, KeyCode::Char('R')) => {
            app.reject_external();
            app.message = Some("External changes rejected".to_string());
        }
        (_, KeyCode::Char('e')) | (_, KeyCode::Char('E')) => {
            app.accept_external();
            app.message = Some("Changes applied — edit to adjust".to_string());
        }
        (_, KeyCode::Esc) => {
            app.mode = Mode::Edit;
        }
        (_, KeyCode::Up) | (_, KeyCode::Char('k')) => {
            app.diff_scroll = app.diff_scroll.saturating_sub(1);
        }
        (_, KeyCode::Down) | (_, KeyCode::Char('j')) => {
            app.diff_scroll += 1;
        }
        (_, KeyCode::PageUp) => {
            app.diff_scroll = app.diff_scroll.saturating_sub(20);
        }
        (_, KeyCode::PageDown) => {
            app.diff_scroll += 20;
        }
        _ => {}
    }
}

fn handle_outline_key(app: &mut App, key: KeyEvent) {
    match (key.modifiers, key.code) {
        (_, KeyCode::Esc) => {
            app.mode = Mode::Edit;
        }
        (_, KeyCode::Up) | (_, KeyCode::Char('k')) => {
            if app.outline_selected > 0 {
                app.outline_selected -= 1;
            }
        }
        (_, KeyCode::Down) | (_, KeyCode::Char('j')) => {
            if app.outline_selected + 1 < app.outline_entries.len() {
                app.outline_selected += 1;
            }
        }
        (_, KeyCode::Enter) => {
            if let Some(entry) = app.outline_entries.get(app.outline_selected) {
                app.buffer.cursor.line = entry.line;
                app.buffer.cursor.col = 0;
                app.buffer.cursor.clamp_col(&app.buffer.rope);
                if let Ok((_, rows)) = crossterm::terminal::size() {
                    let h = rows.saturating_sub(2) as usize;
                    app.buffer.scroll_offset = entry.line.saturating_sub(h / 3);
                    app.buffer.adjust_scroll(h);
                }
                app.mode = Mode::Edit;
            }
        }
        _ => {}
    }
}

fn handle_preview_key(app: &mut App, key: KeyEvent) {
    match (key.modifiers, key.code) {
        (_, KeyCode::Esc) | (KeyModifiers::ALT, KeyCode::Char('p')) => {
            app.mode = Mode::Edit;
        }
        (_, KeyCode::Up) | (_, KeyCode::Char('k')) => {
            app.preview_scroll = app.preview_scroll.saturating_sub(1);
        }
        (_, KeyCode::Down) | (_, KeyCode::Char('j')) => {
            app.preview_scroll += 1;
        }
        (_, KeyCode::PageUp) => {
            app.preview_scroll = app.preview_scroll.saturating_sub(20);
        }
        (_, KeyCode::PageDown) => {
            app.preview_scroll += 20;
        }
        _ => {}
    }
}
