# Notes Manager - Rust sources

Cargo project for the application described in the [top-level README](../README.md).

```bash
cargo run --release   # build and start the app
cargo test            # run the storage tests
```

## Modules

| File               | Contents                                                        |
| ------------------ | --------------------------------------------------------------- |
| `src/main.rs`      | Entry point; opens the SQLite database and starts GTK           |
| `src/notes.rs`     | `Note` type and the date helper                                 |
| `src/database.rs`  | `Database`: add, find, update, delete, list, search; unit tests |
| `src/ui.rs`        | `AppState`, widgets, list/editor synchronisation, dialogs       |
| `src/css/theme.css`| Small stylesheet on libadwaita named colors, embedded           |

## How the UI keeps state consistent

- The list selection decides what the editor shows. Search, create and delete
  all call `AppState::reload_list`, which chooses the new selection and then
  syncs the editor with it.
- The list allows multiple selection, by Ctrl/Shift-click or by the tick box in
  each row (a `CheckButton` kept in sync with the row's selected state). The
  editor shows a note only when exactly one is selected; with none or several
  it is cleared and disabled.
- Delete removes all selected notes in one database transaction and then
  selects the nearest remaining note; with no notes left the list is empty.
- `updating_editor` and `updating_list` flags stop programmatic changes from
  being treated as user edits or user selections.
- Edits are written to SQLite on every change, and only the affected list row
  is redrawn.

## Theming

The app starts through `adw::Application`, which loads libadwaita's stylesheet.
`theme.css` then uses its named colors (`@card_bg_color`, `@accent_bg_color`,
`@headerbar_bg_color`, `@view_bg_color`, ...) instead of hard-coded values, and
the Rust code uses libadwaita style classes (`suggested-action`,
`destructive-action`, `heading`, `caption`, `dim-label`) for buttons and text.

The stylesheet has no `window`, `headerbar` or bare `button` rules, so the
titlebar and window controls are drawn by GTK/libadwaita unchanged.
