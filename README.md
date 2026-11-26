# readmd

Keyboard-first Markdown reader and editor built with Rust and GPUI (`gpui-kit` `gpui-base` + `gpui-fast`).

## Features

- Fast, GPU-accelerated Markdown rendering and plain-text editing with synchronized scroll positions between Normal and Insert modes
- Follows the active Omarchy desktop theme live (`~/.local/state/omarchy/current/theme/colors.toml`) for app backgrounds and command-bar accents, while keeping Markdown highlights cleanly neutral and adaptive for dark and light modes
- Soft matte charcoal dark fallback (`#161618`) on non-Omarchy systems, plus a built-in `:Theme` command picker (`auto`, `dark`, `light`, `tokyo-night`, `catppuccin`, `gruvbox`, `nord`, `flexoki-light`)
- Vim-inspired modal workflow (`INSERT`, `NORMAL`, `COMMAND`) with a top-center `:` command bar, live hints, and `?` shortcuts overlay
- System font picker (`:Font <name>`), drag-and-drop Markdown file opening, CLI file argument (`readmd path/to/file.md`), and atomic file saves
- Customizable keybindings via `~/.config/readmd/keymaps.toml`

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `i` | Enter Insert mode (from Normal mode) |
| `Esc` | Return to Normal mode / close command bar or help |
| `:` | Open command bar (from Normal mode) |
| `j` / `k` or `↓` / `↑` | Scroll in Normal mode |
| `Ctrl+N` | Create a new blank file |
| `Ctrl+O` | Open a Markdown file |
| `Ctrl+S` | Save current file |
| `Ctrl+Q` | Quit (guards unsaved changes) |
| `?` | Toggle shortcuts help overlay (in Normal mode) |

### Command Bar (`:`)

| Command | Description |
| :--- | :--- |
| `:w [path]` | Save current file (or open save dialog) |
| `:wq [path]` | Save and quit |
| `:q` / `:q!` | Quit / force quit without saving |
| `:n` / `:n!` | New blank file / discard changes and create new file |
| `:e [path]` | Open a Markdown file |
| `:Font <name>` | Change document font (`default` resets) |
| `:Theme <name>` | Switch color theme (`auto`, `dark`, `light`, `tokyo-night`, `catppuccin`, `gruvbox`, `nord`, `flexoki-light`) |

## Requirements

- Linux (Wayland) with Vulkan / WGPU support
- Rust & Cargo (to build from source)

## Build, Run & Install

```bash
# Run in development (optionally pass a Markdown file)
cargo run -- README.md

# Run tests
./bin/test

# Build release binary
./bin/build

# Install as an Arch package (adds binary, icon & launcher entry)
./bin/install

# Launch after install (or open "readmd" from the Omarchy launcher)
readmd README.md

# Uninstall
omarchy pkg drop readmd
# or: sudo pacman -Rns readmd
```

## License

MIT — [1kaye](https://github.com/1kaye)
