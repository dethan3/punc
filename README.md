# Punc

A terminal-native Markdown editor built for the age of AI Agents.

**[中文文档](README_zh.md)**

## Why another editor?

### Why terminal, not VS Code?

VS Code is a great IDE. But when you're *writing*, it gets in the way.

Extensions fight for attention. Sidebars steal screen space. Notifications interrupt your flow. The Markdown you write is buried under layers of UI that exist for *coding*, not *writing*.

Writing wants focus. A terminal gives you exactly that — one screen, one file, nothing else. No mouse needed. No distractions possible.

And in the Agent era, terminal is where the action is. You run `codex`, `claude`, `aider` in terminal sessions. Your writing tool should live there too, not in a separate GUI window that breaks the workflow.

### Why not existing terminal editors?

Vim, Helix, and Neovim are excellent code editors. But they treat Markdown as just another filetype. When an Agent modifies your file externally:

- **Vim**: "File changed. Reload? (y/n)" — no diff, no context
- **Helix**: Same reload prompt, same blind choice
- **Nano**: Doesn't even notice

None of them understand the **human-Agent writing loop**:

```
You write → Agent edits the file → You review the diff → You adjust → Agent edits again → Done
```

punc is designed for this loop. It watches the file, shows you exactly what changed, and lets you accept, reject, or edit — without leaving the editor.

### What punc does differently

| Scenario | vim / helix | glow / mdcat | punc |
|---|---|---|---|
| Write a draft | ✅ | ❌ read-only | ✅ |
| Review Agent's changes | Blind reload prompt | ❌ | ✅ inline diff |
| Continue editing after review | Manual reload | ❌ | ✅ seamless |
| Multiple collaboration rounds | Reload prompt every time | ❌ | ✅ continuous watch |

## Install

### From GitHub Releases

Download the pre-built binary from [Releases](https://github.com/dethan3/punc/releases):

> Currently only **Linux x86_64** binaries are provided. macOS and Windows support is planned.

```bash
chmod +x punc-linux-amd64
sudo mv punc-linux-amd64 /usr/local/bin/punc
```

### From source (any platform)

Requires [Rust toolchain](https://rustup.rs/) (1.70+):

```bash
git clone https://github.com/dethan3/punc.git
cd punc
cargo build --release
cp target/release/punc ~/.local/bin/
```

Clipboard paste (`Alt+V`) on Linux needs `xclip` or `xsel`.

## Usage

```bash
punc README.md
punc ~/docs/proposal.md
```

That's it. One file. Focused writing.

## Keyboard Shortcuts

All shortcuts use `Alt` to avoid conflicts with VS Code, tmux, and system hotkeys.

### Editing

| Key | Action |
|---|---|
| `Alt+S` | Save |
| `Alt+Q` | Quit |
| `Alt+Z` | Undo |
| `Alt+Y` | Redo |
| `Alt+V` | Paste |
| `Tab` | Insert indent |

### Overlays

| Key | Action |
|---|---|
| `Alt+P` | Preview (rendered Markdown) |
| `Alt+O` | Outline (heading navigation) |
| `Alt+D` | Diff (review external changes) |
| `Esc` | Close overlay, back to editing |

### In Diff view

| Key | Action |
|---|---|
| `A` | Accept external changes |
| `R` | Reject, keep your version |
| `E` | Accept and continue editing |
| `↑↓` | Scroll |
| `Esc` | Decide later |

### External Changes

When the file on disk changes outside `punc`, the status bar shows `⚡`.

- `⚡` means: the disk file no longer matches your current buffer
- `Alt+D` opens a diff between `punc`'s buffer and the current disk file
- `A` accepts the disk version into the buffer
- `R` ignores that external change and keeps your current buffer
- `E` accepts the disk version, then lets you continue editing immediately

If the external tool changes the file and then changes it back to match your buffer again, the `⚡` indicator disappears automatically.

## The Agent Workflow

```
Terminal 1                      Terminal 2
┌─────────────────────┐        ┌─────────────────────┐
│ punc proposal.md    │        │ codex / claude       │
│                     │        │ > expand section 3   │
│  (you're writing)   │        │                     │
│                     │  ←──── │ (agent edits file)   │
│  ⚡ External change  │        │                     │
│  Alt+D → diff view  │        │                     │
│  A/R/E to decide    │        │                     │
└─────────────────────┘        └─────────────────────┘
```

No API. No protocol. No plugins. The file system is the interface.

## Design Principles

- **Single focus** — one pane, one thing visible at a time
- **On-demand overlays** — preview, outline, diff appear only when needed
- **Keyboard-first** — built for writers, not mouse users
- **Reviewable diff** — external changes are never silently applied
- **Stability over complexity** — do fewer things, do them well
- **Unix philosophy** — punc edits, the OS manages files

## License

MIT
