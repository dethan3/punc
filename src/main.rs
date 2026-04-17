mod app;
mod diff;
mod editor;
mod highlight;
mod ui;
mod watcher;

use std::env;
use std::fs::{self, File};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, Mode, QuitAction};
use watcher::FileWatcher;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, PartialEq, Eq)]
enum CliCommand {
    Edit(PathBuf),
    Help,
    Version,
    Keys,
    Doctor,
}

fn main() -> io::Result<()> {
    match parse_cli(env::args()) {
        Ok(CliCommand::Edit(path)) => run_editor(&path),
        Ok(CliCommand::Help) => {
            print!("{}", help_text());
            Ok(())
        }
        Ok(CliCommand::Version) => {
            println!("punc {}", VERSION);
            Ok(())
        }
        Ok(CliCommand::Keys) => {
            print!("{}", keys_text());
            Ok(())
        }
        Ok(CliCommand::Doctor) => run_doctor(io::stdout()),
        Err(err) => {
            eprintln!("{err}");
            process::exit(2);
        }
    }
}

fn parse_cli<I>(args: I) -> Result<CliCommand, String>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let program = args.next().unwrap_or_else(|| "punc".to_string());
    let Some(first) = args.next() else {
        return Err(format!("No file provided.\n\n{}", usage_text(&program)));
    };

    match first.as_str() {
        "-h" | "--help" => {
            ensure_no_extra_args(&program, &first, args.next())?;
            Ok(CliCommand::Help)
        }
        "-V" | "--version" => {
            ensure_no_extra_args(&program, &first, args.next())?;
            Ok(CliCommand::Version)
        }
        "--keys" => {
            ensure_no_extra_args(&program, &first, args.next())?;
            Ok(CliCommand::Keys)
        }
        "doctor" => {
            ensure_no_extra_args(&program, &first, args.next())?;
            Ok(CliCommand::Doctor)
        }
        "--" => {
            let Some(path) = args.next() else {
                return Err(format!(
                    "Missing file path after `--`.\n\n{}",
                    usage_text(&program)
                ));
            };
            if let Some(extra) = args.next() {
                return Err(format!(
                    "punc accepts exactly one file path. Unexpected argument `{extra}`.\n\n{}",
                    usage_text(&program)
                ));
            }
            Ok(CliCommand::Edit(PathBuf::from(path)))
        }
        _ if first.starts_with('-') => Err(format!(
            "Unknown option `{first}`.\n\n{}",
            usage_text(&program)
        )),
        _ => {
            if let Some(extra) = args.next() {
                return Err(format!(
                    "punc accepts exactly one file path. Unexpected argument `{extra}`.\n\n{}",
                    usage_text(&program)
                ));
            }
            Ok(CliCommand::Edit(PathBuf::from(first)))
        }
    }
}

fn ensure_no_extra_args(program: &str, context: &str, extra: Option<String>) -> Result<(), String> {
    if let Some(arg) = extra {
        Err(format!(
            "Unexpected argument `{arg}` after `{context}`.\n\n{}",
            usage_text(program)
        ))
    } else {
        Ok(())
    }
}

fn usage_text(program: &str) -> String {
    format!(
        "Usage:\n  {program} <file>\n  {program} --help\n  {program} --version\n  {program} --keys\n  {program} doctor\n  {program} -- <file-starting-with-dash>"
    )
}

fn help_text() -> String {
    format!(
        "punc {VERSION}\n{}\n\n{}\n\nCommands:\n  doctor          Check terminal, clipboard, and file watcher support\n\nOptions:\n  -h, --help      Show this help\n  -V, --version   Show version\n      --keys      Show keyboard shortcuts\n\nExamples:\n  punc README.md\n  punc --version\n  punc --keys\n  punc doctor\n  punc -- --version\n",
        env!("CARGO_PKG_DESCRIPTION"),
        usage_text("punc"),
    )
}

fn keys_text() -> &'static str {
    "\
Punc Keyboard Shortcuts

Editing
  Alt+S     Save
  Alt+Q     Quit
  Alt+Z     Undo
  Alt+Y     Redo
  Alt+V     Paste
  Tab       Insert 4 spaces

Navigation
  Arrow keys / Home / End / PageUp / PageDown

Overlays
  Alt+P     Preview
  Alt+O     Outline
  Alt+D     Diff external changes
  Esc       Close overlay

Diff View
  A         Accept external changes
  R         Reject external changes
  E         Accept and keep editing
  Up/Down   Scroll
  Esc       Decide later

Quit Confirmation
  Left/Right or Tab     Select action
  Enter                 Confirm selected action
  S                     Save and quit
  D                     Discard and quit
  Esc                   Cancel
"
}

fn run_doctor<W: Write>(mut writer: W) -> io::Result<()> {
    writeln!(writer, "punc doctor")?;
    writeln!(writer, "version: {VERSION}")?;
    writeln!(
        writer,
        "platform: {} {}",
        env::consts::OS,
        env::consts::ARCH
    )?;
    writeln!(writer, "stdin tty: {}", yes_no(io::stdin().is_terminal()))?;
    writeln!(writer, "stdout tty: {}", yes_no(io::stdout().is_terminal()))?;

    match crossterm::terminal::size() {
        Ok((cols, rows)) => writeln!(writer, "terminal size: {cols}x{rows}")?,
        Err(err) => writeln!(writer, "terminal size: unavailable ({err})")?,
    }

    match available_clipboard_command() {
        Some(program) => writeln!(writer, "clipboard helper: {program}")?,
        None => writeln!(
            writer,
            "clipboard helper: unavailable (install xclip/xsel on Linux, or use terminal paste)"
        )?,
    }

    match check_file_watcher() {
        Ok(()) => writeln!(writer, "file watcher: ok")?,
        Err(err) => writeln!(writer, "file watcher: unavailable ({err})")?,
    }

    Ok(())
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn available_clipboard_command() -> Option<&'static str> {
    clipboard_commands()
        .iter()
        .map(|(program, _)| *program)
        .find(|program| Path::new(program).is_file())
}

fn check_file_watcher() -> Result<(), String> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let path = env::temp_dir().join(format!("punc-doctor-{}-{unique}.tmp", process::id()));

    File::create(&path).map_err(|err| err.to_string())?;
    let result = FileWatcher::new(path.clone())
        .map(|_| ())
        .map_err(|err| err.to_string());
    let _ = fs::remove_file(&path);
    result
}

fn run_editor(path: &Path) -> io::Result<()> {
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
        (_, KeyCode::Tab) => app.buffer.insert_text("    "),

        _ => {}
    }

    // Adjust scroll after cursor movement (wrap-aware)
    if let Ok((cols, rows)) = crossterm::terminal::size() {
        let editor_height = rows.saturating_sub(2) as usize; // minus status + hint bars
        let editor_width = cols as usize;
        app.buffer.adjust_scroll_wrapped(editor_height, editor_width);
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
    use super::{
        clipboard_commands, handle_edit_key, handle_quit_confirm_key, help_text, keys_text,
        parse_cli, run_doctor, CliCommand,
    };
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
    fn parse_version_flag() {
        let command = parse_cli(["punc", "--version"]).unwrap();
        assert_eq!(command, CliCommand::Version);
    }

    #[test]
    fn parse_doctor_command() {
        let command = parse_cli(["punc", "doctor"]).unwrap();
        assert_eq!(command, CliCommand::Doctor);
    }

    #[test]
    fn dash_dash_allows_dash_prefixed_file_name() {
        let command = parse_cli(["punc", "--", "--version"]).unwrap();
        assert_eq!(command, CliCommand::Edit("--version".into()));
    }

    #[test]
    fn unknown_flag_returns_error() {
        let err = parse_cli(["punc", "--wat"]).unwrap_err();
        assert!(err.contains("Unknown option `--wat`"));
    }

    #[test]
    fn help_and_keys_text_cover_new_cli_commands() {
        assert!(help_text().contains("punc --version"));
        assert!(help_text().contains("punc doctor"));
        assert!(keys_text().contains("Alt+D"));
    }

    #[test]
    fn doctor_report_includes_expected_sections() {
        let mut output = Vec::new();
        run_doctor(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("punc doctor"));
        assert!(output.contains("clipboard helper:"));
        assert!(output.contains("file watcher:"));
    }

    #[test]
    fn tab_is_undone_and_redone_as_one_logical_edit() {
        let path = temp_file_path("tab-undo");
        fs::write(&path, "").unwrap();

        let mut app = App::new(&path).unwrap();

        handle_edit_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.buffer.rope.to_string(), "    ");
        assert_eq!(app.buffer.cursor.col, 4);

        handle_edit_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT),
        );
        assert_eq!(app.buffer.rope.to_string(), "");
        assert_eq!(app.buffer.cursor.col, 0);

        handle_edit_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::ALT),
        );
        assert_eq!(app.buffer.rope.to_string(), "    ");
        assert_eq!(app.buffer.cursor.col, 4);

        fs::remove_file(&path).unwrap();
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
