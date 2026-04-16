# Changelog

## v0.3.3 - 2026-04-16

- Fixed `Tab` insertion so one keypress is treated as one logical undo step.
- Fixed external-change acceptance so using the disk version leaves the buffer clean instead of immediately triggering unsaved-change state.
- Made dirty-state tracking follow the saved/disk-synced state, so undo and redo now correctly restore clean state when returning to saved content.
- Improved multi-character cursor positioning by deriving coordinates from `ropey`, including correct handling for CRLF input.

## v0.3.0 - 2026-04-15

- Added built-in CLI commands for `--help`, `--version`, `--keys`, and `doctor`.
- Fixed argument parsing so flags no longer open accidental files, with `--` support for dash-prefixed file names.
- Added doctor checks for terminal, clipboard helper, and file watcher availability.
- Documented the new CLI command surface and quit-confirmation shortcuts in both English and Chinese READMEs.

## v0.2.0 - 2026-04-14

- Added unsaved-exit confirmation with `Save`, `Discard`, and `Cancel` actions.
- Added keyboard selection in the quit dialog with default focus on `Save`, arrow-key navigation, and `Enter` to confirm.
- Fixed external-change state handling so the alert clears automatically when the disk file matches the buffer again.
- Clarified external diff behavior in the editor and documentation, with `Alt+D` shown persistently in the edit hint bar.
- Hardened file handling and clipboard command execution.
- Reduced editor and preview memory usage by avoiding full-string file IO and caching preview/status computations.
