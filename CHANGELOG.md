# Changelog

## v0.2.0 - 2026-04-14

- Added unsaved-exit confirmation with `Save`, `Discard`, and `Cancel` actions.
- Added keyboard selection in the quit dialog with default focus on `Save`, arrow-key navigation, and `Enter` to confirm.
- Fixed external-change state handling so the alert clears automatically when the disk file matches the buffer again.
- Clarified external diff behavior in the editor and documentation, with `Alt+D` shown persistently in the edit hint bar.
- Hardened file handling and clipboard command execution.
- Reduced editor and preview memory usage by avoiding full-string file IO and caching preview/status computations.
