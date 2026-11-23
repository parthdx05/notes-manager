# Notes Manager

A simple notes application written in Rust with GTK4, storing notes in SQLite.

## Features

- Create, read, update and delete notes, including deleting several at once with tick boxes or Ctrl/Shift selection
- Search notes by title (ASCII case-insensitive)
- Every change is saved to the database immediately; there is no save button
- Split-pane GTK4 UI: note list on the left, editor on the right, status bar
- Styled with libadwaita colors, so it follows the system light/dark style and accent color
- Standard window titlebar, handled entirely by GTK/libadwaita
- UUID note identifiers and creation/modification timestamps

## Requirements

- Rust (2021 edition) with `cargo`
- A C compiler (the `rusqlite` crate compiles SQLite from source)
- GTK4 (4.6 or newer) and libadwaita (1.0 or newer) development libraries

Debian/Ubuntu:

```bash
sudo apt-get install build-essential pkg-config libgtk-4-dev libadwaita-1-dev
```

Fedora:

```bash
sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel
```

Arch Linux:

```bash
sudo pacman -S --needed base-devel gtk4 libadwaita
```

## Build and Run

```bash
cd src-rust
cargo run --release
```

## Where notes are stored

The SQLite database is `notes-manager/notes.db` inside your user data
directory (`~/.local/share` on most Linux systems). Set `NOTES_MANAGER_DB` to
use a different file:

```bash
NOTES_MANAGER_DB=/tmp/test-notes.db cargo run
```

## Usage

- **New** creates a note, selects it and puts the cursor in the title field.
- Select a note in the list to edit it; changes are saved as you type.
- Select several notes with Ctrl+click, Shift+click or Ctrl+A, or tick the box
  at the left of each note (ticking adds or removes just that note). The editor
  is disabled while more than one note is selected and the status bar shows how
  many are.
- **Delete** removes every selected note after one confirmation, then
  selects the nearest remaining note.
- The search box filters the list by title; clear it to see all notes.
- The editor is disabled while no note is selected.

## Project Structure

```
src-rust/
├── Cargo.toml
└── src/
    ├── main.rs        # Entry point: opens the database, starts the app
    ├── notes.rs       # Note type and date helper
    ├── database.rs    # SQLite storage, with unit tests
    ├── ui.rs          # Widgets, selection/editor logic, dialogs
    └── css/theme.css  # Small stylesheet using libadwaita named colors (embedded)
```

See [TESTING.md](TESTING.md) for how to run the tests.

## License

MIT License.
