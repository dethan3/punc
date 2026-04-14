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

use app::{App, Mode, QuitAction};
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
    let file_watcher = FileWatcher::new(app.buffer.path.clone()).ok();

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
    if app.mode != Mode::QuitConfirm
        && (key.modifiers, key.code) == (KeyModifiers::ALT, KeyCode::Char('q'))
    {
        app.request_quit();
        return;
    }

    match app.mode {
        Mode::Edit => handle_edit_key(app, key),
        Mode::Preview => handle_preview_key(app, key),
        Mode::Outline => handle_outline_key(app, key),
        Mode::Diff => handle_diff_key(app, key),
        Mode::QuitConfirm => handle_quit_confirm_key(app, key),
    }
}

fn handle_edit_key(app: &mut App, key: KeyEvent) {
    match (key.modifiers, key.code) {
        // Ctrl combos
        (KeyModifiers::ALT, KeyCode::Char('s')) => match app.buffer.save() {
            Ok(()) => app.message = Some("Saved".to_string()),
            Err(e) => app.message = Some(format!("Save failed: {}", e)),
        },
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
            } else {
                app.message = Some("No external changes".to_string());
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
    for (program, args) in clipboard_commands() {
        if let Ok(output) = Command::new(program).args(*args).output() {
            if output.status.success() {
                return String::from_utf8(output.stdout).map_err(|_| ());
            }
        }
    }
    Err(())
}

fn clipboard_commands() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        ("/usr/bin/xclip", &["-selection", "clipboard", "-o"]),
        ("/usr/local/bin/xclip", &["-selection", "clipboard", "-o"]),
        ("/bin/xclip", &["-selection", "clipboard", "-o"]),
        ("/usr/bin/xsel", &["--clipboard", "--output"]),
        ("/usr/local/bin/xsel", &["--clipboard", "--output"]),
        ("/bin/xsel", &["--clipboard", "--output"]),
        ("/usr/bin/pbpaste", &[]),
        ("/usr/local/bin/pbpaste", &[]),
        ("/opt/homebrew/bin/pbpaste", &[]),
        ("/opt/local/bin/pbpaste", &[]),
        ("/bin/pbpaste", &[]),
    ]
}

#[cfg(test)]
mod tests {
    use super::{clipboard_commands, handle_quit_confirm_key};
    use crate::app::{App, QuitAction};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("punc-{name}-{unique}.md"))
    }

    #[test]
    fn clipboard_commands_use_absolute_paths() {
        assert!(clipboard_commands()
            .iter()
            .all(|(program, _)| program.starts_with('/')));
    }

    #[test]
    fn enter_uses_selected_quit_action() {
        let path = temp_file_path("quit-enter");
        fs::write(&path, "hello\n").unwrap();

        let mut app = App::new(&path).unwrap();
        app.buffer.insert_char('!');
        app.request_quit();
        app.quit_selected = QuitAction::Discard;

        handle_quit_confirm_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(app.should_quit);

        fs::remove_file(&path).unwrap();
    }
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

fn handle_quit_confirm_key(app: &mut App, key: KeyEvent) {
    match (key.modifiers, key.code) {
        (_, KeyCode::Char('s')) | (_, KeyCode::Char('S')) => app.save_and_quit(),
        (_, KeyCode::Char('d')) | (_, KeyCode::Char('D')) => app.discard_and_quit(),
        (_, KeyCode::Esc) | (_, KeyCode::Char('c')) | (_, KeyCode::Char('C')) => {
            app.cancel_quit();
        }
        (_, KeyCode::Left) => app.select_prev_quit_action(),
        (_, KeyCode::Right) | (_, KeyCode::Tab) => app.select_next_quit_action(),
        (_, KeyCode::BackTab) => app.select_prev_quit_action(),
        (_, KeyCode::Enter) => match app.quit_selected {
            QuitAction::Save => app.save_and_quit(),
            QuitAction::Discard => app.discard_and_quit(),
            QuitAction::Cancel => app.cancel_quit(),
        },
        _ => {}
    }
}
