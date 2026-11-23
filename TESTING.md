# Testing Guide for Notes Manager

## Automated tests

The storage layer has unit tests that run against an in-memory SQLite
database. They cover add/find, ordering, search (including `%` and `_` in the
query), updates, deleting a list of notes down to one and then to zero, and
deleting several notes at once.

```bash
cd src-rust
cargo test
```

Building the tests needs the same dependencies as the application (a C
compiler and the GTK4 development libraries; see the README).

## Manual test checklist

Use a scratch database so your real notes are untouched:

```bash
cd src-rust
NOTES_MANAGER_DB=/tmp/notes-test.db cargo run
```

1. **Empty state**: the list is empty, the editor is greyed out, and the status
   bar says "No note selected". Delete shows "No note selected to delete".
2. **Create**: click New three times. Each new note is selected, the title is
   highlighted and the cursor is in it. Type a title, then some content.
3. **Live list update**: the list row follows the title and the first line of
   content as you type.
4. **Switch notes**: select another note; the editor shows its text. Switch
   back; your edits are still there.
5. **Persistence**: close the app, start it again; all notes are present.
6. **Search**: type part of a title in any letter case; only matching notes
   remain. Clear the box; all notes return. A search with no matches clears
   and disables the editor.
7. **Delete down to one note**: delete notes until one remains. The remaining
   note must be shown in the editor and be fully editable and deletable.
8. **Delete the last note**: confirm; the list is empty, the editor is
   disabled. Click New; it works again.
9. **New while searching**: type a search that matches nothing, click New;
   the filter is cleared and the new note is selected.
10. **Multi-select**: Ctrl+click and Shift+click select several notes; the
    status bar shows "N notes selected" and the editor is greyed out. Click a
    single note to edit it again. Ctrl+A selects all.
11. **Multi-delete**: select three of five notes, click Delete. One dialog
    asks about "these 3 notes"; after Yes only the other two remain, and the
    nearest remaining note is selected. Repeat with all notes selected: the
    list ends empty and New works.
12. **Search + selection**: select two notes, type a search that matches both;
    they stay selected. Delete with the filter on removes only the selected
    visible notes.
13. **Tick boxes**: every row has a tick box. Ticking two boxes selects both
    notes (status bar: "2 notes selected"). Unticking one leaves a single
    selection and the editor shows that note. Ctrl+click or Shift+click a row
    and its box ticks; click a row without Ctrl to select only it and all
    other boxes clear. Ctrl+A ticks everything. Scroll a long list and
    confirm ticks stay with the right notes.
14. **Theme**: switch the system between light and dark style (and change the
    accent color if your desktop allows it). Note cards, the editor, toolbar
    and status bar follow it with readable text; New is accent-colored and
    Delete is red.
15. **Titlebar**: the window has the normal titlebar with standard-sized
    close/minimize/maximize buttons, unaffected by the app's styling.

## Troubleshooting

**`gtk4` or `libadwaita` not found during build**
- Install the GTK4 and libadwaita development packages (`libgtk-4-dev` and
  `libadwaita-1-dev` on Debian/Ubuntu) and `pkg-config`, then check with
  `pkg-config --modversion gtk4 libadwaita-1`.

**C compiler errors while building `libsqlite3-sys`**
- Install a C compiler (`build-essential` on Debian/Ubuntu).

**"Could not open the database" in the status bar**
- The database path is not writable. Check `NOTES_MANAGER_DB` or the
  permissions of your user data directory. Notes are not saved in this state.

## Reporting issues

Run the application from a terminal to capture error output, note the steps to
reproduce, and include your OS and the output of `pkg-config --modversion gtk4 libadwaita-1`.
