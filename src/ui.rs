//! GTK4 user interface.
//!
//! Design rule: the list selection decides what the editor shows. Several
//! notes can be selected (Ctrl/Shift-click, Ctrl+A) so they can be deleted
//! together; the editor shows a note only when exactly one is selected and is
//! disabled otherwise, so text can never be typed into a note that does not
//! exist. Every operation that changes the list (search, create, delete) goes
//! through [`AppState::reload_list`], which picks the new selection and then
//! brings the editor in line with it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4 as gtk;

use gtk::prelude::*;
use gtk::Box as GtkBox;
use gtk::{
    gdk, pango, Align, Application, ApplicationWindow, Button, ButtonsType, CheckButton,
    CssProvider, DialogFlags, Entry, Label, ListItem, ListView, MessageDialog, MessageType,
    MultiSelection, Orientation, Paned, PolicyType, ResponseType, ScrolledWindow, Separator,
    SignalListItemFactory, StringList, StringObject, TextView, WrapMode,
};

use crate::database::Database;
use crate::notes::{self, Note, DEFAULT_TITLE};

/* <--------------------Application State--------------------> */

/// State shared by all UI callbacks, held in an `Rc`.
pub struct AppState {
    pub window: ApplicationWindow,
    pub notes_list: ScrolledWindow,
    pub editor: GtkBox,
    pub title_entry: Entry,
    pub content_text: TextView,
    pub search_entry: Entry,
    pub status_label: Label,
    pub database: Database,
    /// ID of the note currently shown in the editor.
    pub current_note_id: RefCell<Option<String>>,
    /// Model behind the list; holds note IDs and is updated in place.
    pub model: StringList,
    /// Multi-selection over `model`; one selected row is edited, several can be deleted.
    pub selection: MultiSelection,
    /// True while the editor widgets are being filled programmatically, so
    /// their "changed" handlers do not write the text back to the database.
    pub updating_editor: Cell<bool>,
    /// True while the list model is being rewritten programmatically, so the
    /// selection handler ignores the intermediate selection changes.
    pub updating_list: Cell<bool>,
}

impl AppState {
    pub fn new(app: &impl IsA<Application>, database: Database) -> Rc<Self> {
        let model = StringList::new(&[]);
        let selection = MultiSelection::new(Some(model.clone()));

        Rc::new(AppState {
            window: ApplicationWindow::new(app),
            notes_list: ScrolledWindow::new(),
            editor: GtkBox::new(Orientation::Vertical, 12),
            title_entry: Entry::new(),
            content_text: TextView::new(),
            search_entry: Entry::new(),
            status_label: Label::new(Some("Ready")),
            database,
            current_note_id: RefCell::new(None),
            model,
            selection,
            updating_editor: Cell::new(false),
            updating_list: Cell::new(false),
        })
    }

    pub fn update_status(&self, message: &str) {
        self.status_label.set_text(message);
    }

    /// Unwrap a database result, reporting a failure in the status bar.
    fn report<T>(&self, result: rusqlite::Result<T>, action: &str) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(err) => {
                eprintln!("Database error while {action}: {err}");
                self.update_status(&format!("Error while {action}"));
                None
            }
        }
    }

    /// Position of a note ID in the list model.
    fn position_of(&self, note_id: &str) -> Option<u32> {
        (0..self.model.n_items()).find(|&i| self.model.string(i).as_deref() == Some(note_id))
    }

    /// Positions of the selected rows, in list order.
    fn selected_positions(&self) -> Vec<u32> {
        (0..self.model.n_items())
            .filter(|&i| self.selection.is_selected(i))
            .collect()
    }

    /// IDs of the selected notes, in list order.
    pub fn selected_ids(&self) -> Vec<String> {
        self.selected_positions()
            .into_iter()
            .filter_map(|i| self.model.string(i))
            .map(|id| id.to_string())
            .collect()
    }

    /// Rebuild the list from the database (honouring the search box), select
    /// those of `prefer` that are still listed (or the first note if none
    /// are), and sync the editor. Returns the number of notes listed.
    pub fn reload_list(&self, prefer: &[String]) -> usize {
        let query = self.search_entry.text();
        let result = if query.is_empty() {
            self.database.all_notes()
        } else {
            self.database.search(query.as_str())
        };
        let ids: Vec<String> = self
            .report(result, "loading notes")
            .unwrap_or_default()
            .into_iter()
            .map(|note| note.id)
            .collect();
        let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();

        self.updating_list.set(true);
        self.model.splice(0, self.model.n_items(), &id_refs);
        let mut any_selected = false;
        for (position, id) in ids.iter().enumerate() {
            if prefer.contains(id) {
                self.selection.select_item(position as u32, false);
                any_selected = true;
            }
        }
        if !any_selected && !ids.is_empty() {
            self.selection.select_item(0, true);
        }
        self.updating_list.set(false);

        self.sync_editor_with_selection();
        ids.len()
    }

    /// Redraw the list row of one note after its title or content changed.
    fn refresh_row(&self, note_id: &str) {
        let Some(position) = self.position_of(note_id) else {
            return;
        };
        let was_selected = self.selection.is_selected(position);

        self.updating_list.set(true);
        self.model.splice(position, 1, &[note_id]);
        if was_selected {
            self.selection.select_item(position, false);
        }
        self.updating_list.set(false);
    }

    /// The note to select once the selected notes are deleted: the first
    /// unselected note at or after the first selected row, otherwise the last
    /// unselected one before it.
    fn survivor_after_delete(&self) -> Option<String> {
        let count = self.model.n_items();
        let first = self.selected_positions().first().copied()?;
        let survivor = (first..count)
            .find(|&i| !self.selection.is_selected(i))
            .or_else(|| (0..first).rev().find(|&i| !self.selection.is_selected(i)))?;
        self.model.string(survivor).map(|id| id.to_string())
    }

    /// Make the editor show the selected note when exactly one is selected;
    /// otherwise clear and disable it.
    fn sync_editor_with_selection(&self) {
        let selected = self.selected_ids();
        let note_id = match selected.as_slice() {
            [] => {
                self.clear_editor();
                self.update_status("No note selected");
                return;
            }
            [id] => id.clone(),
            many => {
                self.clear_editor();
                self.update_status(&format!("{} notes selected", many.len()));
                return;
            }
        };

        // Already showing this note: leave the cursor and text alone.
        if self.current_note_id.borrow().as_deref() == Some(note_id.as_str()) {
            return;
        }

        match self.report(self.database.find_note(&note_id), "loading note") {
            Some(Some(note)) => self.show_note_in_editor(&note),
            Some(None) => {
                self.clear_editor();
                self.update_status("Note not found");
            }
            None => {}
        }
    }

    fn show_note_in_editor(&self, note: &Note) {
        *self.current_note_id.borrow_mut() = Some(note.id.clone());

        self.updating_editor.set(true);
        self.title_entry.set_text(note.title.as_str());
        self.content_text.buffer().set_text(note.content.as_str());
        self.updating_editor.set(false);

        self.editor.set_sensitive(true);
        self.update_status("Showing note");
    }

    fn clear_editor(&self) {
        *self.current_note_id.borrow_mut() = None;

        self.updating_editor.set(true);
        self.title_entry.set_text("");
        self.content_text.buffer().set_text("");
        self.updating_editor.set(false);

        self.editor.set_sensitive(false);
    }
}

/* <--------------------Notes List--------------------> */

pub fn create_notes_list(state: &Rc<AppState>) -> ScrolledWindow {
    state.selection.connect_selection_changed({
        let state = Rc::clone(state);
        move |_, _, _| {
            if !state.updating_list.get() {
                state.sync_editor_with_selection();
            }
        }
    });

    let factory = SignalListItemFactory::new();

    factory.connect_setup({
        let state = Rc::clone(state);
        move |_, item| setup_list_item(&state, item.upcast_ref())
    });

    factory.connect_bind({
        let state = Rc::clone(state);
        move |_, item| bind_list_item(&state, item.upcast_ref())
    });

    let list_view = ListView::new(Some(state.selection.clone()), Some(factory));
    list_view.set_show_separators(true);

    let scrolled = state.notes_list.clone();
    scrolled.set_policy(PolicyType::Never, PolicyType::Automatic);
    scrolled.set_child(Some(&list_view));
    scrolled
}

/// Build the empty row for a list item: a tick box next to the note text.
///
/// The tick box mirrors whether the row is selected. Ticking or unticking it
/// adds or removes just that row, like Ctrl+click, so it can be combined with
/// the keyboard and mouse selection methods.
fn setup_list_item(state: &Rc<AppState>, item: &gtk::glib::Object) {
    let Some(list_item) = item.downcast_ref::<ListItem>() else {
        return;
    };

    let row = GtkBox::new(Orientation::Horizontal, 12);
    row.add_css_class("note");

    let check = CheckButton::new();
    check.set_valign(Align::Center);
    check.set_tooltip_text(Some("Select this note"));
    row.append(&check);

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.set_hexpand(true);
    row.append(&content);

    list_item.set_child(Some(&row));

    // Tick box -> selection (this row only). The comparison with the row's
    // current state also stops the two directions from echoing each other.
    check.connect_toggled({
        let state = Rc::clone(state);
        let list_item = list_item.downgrade();
        move |check| {
            let Some(list_item) = list_item.upgrade() else {
                return;
            };
            let position = list_item.position();
            if position == gtk::INVALID_LIST_POSITION
                || check.is_active() == list_item.is_selected()
            {
                return;
            }
            if check.is_active() {
                state.selection.select_item(position, false);
            } else {
                state.selection.unselect_item(position);
            }
        }
    });

    // Selection -> tick box, for changes made by click, Ctrl/Shift or Ctrl+A.
    list_item.connect_selected_notify({
        let check = check.downgrade();
        move |list_item| {
            if let Some(check) = check.upgrade() {
                check.set_active(list_item.is_selected());
            }
        }
    });
}

/// Fill one list row from the note whose ID the list item holds.
fn bind_list_item(state: &AppState, item: &gtk::glib::Object) {
    let Some(list_item) = item.downcast_ref::<ListItem>() else {
        return;
    };
    let Some(row) = list_item.child().and_downcast::<GtkBox>() else {
        return;
    };
    let Some(check) = row.first_child().and_downcast::<CheckButton>() else {
        return;
    };
    let Some(content) = check.next_sibling().and_downcast::<GtkBox>() else {
        return;
    };

    // Rows are recycled, so show the current selection state for this note.
    check.set_active(list_item.is_selected());

    while let Some(child) = content.first_child() {
        content.remove(&child);
    }

    let Some(note_id) = list_item
        .item()
        .and_downcast::<StringObject>()
        .map(|item| item.string())
    else {
        return;
    };
    let Ok(Some(note)) = state.database.find_note(&note_id) else {
        return;
    };

    let title = if note.title.trim().is_empty() {
        "Untitled"
    } else {
        note.title.as_str()
    };
    let title_label = Label::new(Some(title));
    title_label.set_ellipsize(pango::EllipsizeMode::End);
    title_label.set_max_width_chars(40);
    title_label.set_xalign(0.0);
    title_label.add_css_class("note-title");
    title_label.add_css_class("heading");
    content.append(&title_label);

    // Preview: the first non-blank line of the content.
    if let Some(line) = note.content.lines().find(|l| !l.trim().is_empty()) {
        let content_label = Label::new(Some(line.trim()));
        content_label.set_ellipsize(pango::EllipsizeMode::End);
        content_label.set_max_width_chars(60);
        content_label.set_xalign(0.0);
        content_label.add_css_class("note-content");
        content_label.add_css_class("caption");
        content_label.add_css_class("dim-label");
        content.append(&content_label);
    }

    let date = notes::date_string(note.modified_at);
    let meta_label = Label::new(Some(date.as_str()));
    meta_label.set_xalign(0.0);
    meta_label.add_css_class("note-meta");
    meta_label.add_css_class("caption");
    meta_label.add_css_class("dim-label");
    content.append(&meta_label);
}

/* <--------------------Note Editor--------------------> */

pub fn create_note_editor(state: &Rc<AppState>) -> GtkBox {
    let editor = state.editor.clone();
    editor.add_css_class("editor");
    editor.set_margin_top(16);
    editor.set_margin_bottom(16);
    editor.set_margin_start(16);
    editor.set_margin_end(16);
    editor.set_sensitive(false); // enabled once a note is selected

    let title_label = Label::new(Some("Title:"));
    title_label.set_xalign(0.0);
    editor.append(&title_label);

    state
        .title_entry
        .set_placeholder_text(Some("Enter note title..."));
    state.title_entry.connect_changed({
        let state = Rc::clone(state);
        move |entry| on_title_changed(&state, entry)
    });
    editor.append(&state.title_entry);

    let content_label = Label::new(Some("Content:"));
    content_label.set_xalign(0.0);
    content_label.set_margin_top(8);
    editor.append(&content_label);

    let text_scrolled = ScrolledWindow::new();
    text_scrolled.set_policy(PolicyType::Never, PolicyType::Automatic);
    text_scrolled.set_hexpand(true);
    text_scrolled.set_vexpand(true);

    state.content_text.set_wrap_mode(WrapMode::Word);
    state.content_text.set_editable(true);
    state.content_text.set_cursor_visible(true);
    state.content_text.add_css_class("text-editor");
    state.content_text.buffer().connect_changed({
        let state = Rc::clone(state);
        move |buffer| on_content_changed(&state, buffer)
    });

    text_scrolled.set_child(Some(&state.content_text));
    editor.append(&text_scrolled);
    editor
}

fn on_title_changed(state: &AppState, entry: &Entry) {
    if state.updating_editor.get() {
        return;
    }
    let Some(note_id) = state.current_note_id.borrow().clone() else {
        return;
    };

    let text = entry.text();
    let saved = state.database.set_title(&note_id, text.as_str());
    if state.report(saved, "saving title").is_some() {
        state.refresh_row(&note_id);
        state.update_status("Note saved");
    }
}

fn on_content_changed(state: &AppState, buffer: &gtk::TextBuffer) {
    if state.updating_editor.get() {
        return;
    }
    let Some(note_id) = state.current_note_id.borrow().clone() else {
        return;
    };

    let (start, end) = buffer.bounds();
    let text = buffer.text(&start, &end, false);
    let saved = state.database.set_content(&note_id, text.as_str());
    if state.report(saved, "saving content").is_some() {
        state.refresh_row(&note_id);
        state.update_status("Note saved");
    }
}

/* <--------------------Toolbar--------------------> */

fn on_new_note_clicked(state: &AppState) {
    let note = Note::new(DEFAULT_TITLE, "");
    if state
        .report(state.database.add_note(&note), "creating note")
        .is_none()
    {
        return;
    }

    // A filter could hide the new note, so clear it first.
    if !state.search_entry.text().is_empty() {
        state.search_entry.set_text("");
    }
    state.reload_list(std::slice::from_ref(&note.id));

    state.title_entry.grab_focus();
    state.title_entry.select_region(0, -1);
    state.update_status("New note created");
}

fn on_delete_note_clicked(state: &Rc<AppState>) {
    let note_ids = state.selected_ids();
    if note_ids.is_empty() {
        show_message_dialog(&state.window, "Error", "No note selected to delete");
        return;
    }

    // Choose the note to select next while the doomed ones are still listed.
    let survivor = state.survivor_after_delete();
    let (title, message) = if note_ids.len() == 1 {
        (
            "Delete Note".to_owned(),
            "Are you sure you want to delete this note?".to_owned(),
        )
    } else {
        (
            "Delete Notes".to_owned(),
            format!(
                "Are you sure you want to delete these {} notes?",
                note_ids.len()
            ),
        )
    };

    let parent = state.window.clone();
    let state = Rc::clone(state);
    confirm_dialog(&parent, &title, &message, move || {
        let removed = state.database.remove_notes(&note_ids);
        if let Some(count) = state.report(removed, "deleting notes") {
            let survivors: Vec<String> = survivor.iter().cloned().collect();
            state.reload_list(&survivors);
            state.update_status(&format!("Deleted {count} note(s)"));
        }
    });
}

fn on_search_changed(state: &AppState) {
    let selected = state.selected_ids();
    let count = state.reload_list(&selected);

    if state.search_entry.text().is_empty() {
        state.update_status(&format!("Showing {count} note(s)"));
    } else {
        state.update_status(&format!("{count} search result(s)"));
    }
}

fn create_button<F>(label: &str, callback: F) -> Button
where
    F: Fn(&Button) + 'static,
{
    let button = Button::with_label(label);
    button.connect_clicked(callback);
    button
}

pub fn create_toolbar(state: &Rc<AppState>) -> GtkBox {
    let toolbar = GtkBox::new(Orientation::Horizontal, 8);
    toolbar.add_css_class("toolbar");
    toolbar.set_margin_top(8);
    toolbar.set_margin_bottom(8);
    toolbar.set_margin_start(8);
    toolbar.set_margin_end(8);

    toolbar.append(&Label::new(Some("Search:")));

    state
        .search_entry
        .set_placeholder_text(Some("Search notes..."));
    state.search_entry.set_hexpand(true);
    state.search_entry.connect_changed({
        let state = Rc::clone(state);
        move |_| on_search_changed(&state)
    });
    toolbar.append(&state.search_entry);

    let separator = Separator::new(Orientation::Vertical);
    separator.set_margin_start(8);
    separator.set_margin_end(8);
    toolbar.append(&separator);

    let new_button = create_button("New", {
        let state = Rc::clone(state);
        move |_| on_new_note_clicked(&state)
    });
    new_button.add_css_class("suggested-action");
    toolbar.append(&new_button);

    let delete_button = create_button("Delete", {
        let state = Rc::clone(state);
        move |_| on_delete_note_clicked(&state)
    });
    delete_button.add_css_class("destructive-action");
    toolbar.append(&delete_button);

    toolbar
}

/* <--------------------Dialogs--------------------> */

pub fn show_message_dialog(parent: &impl IsA<gtk::Window>, title: &str, message: &str) {
    let dialog = MessageDialog::new(
        Some(parent),
        DialogFlags::MODAL,
        MessageType::Info,
        ButtonsType::Ok,
        message,
    );
    dialog.set_title(Some(title));
    dialog.connect_response(|dialog, _| dialog.destroy());
    dialog.present();
}

/// Ask a yes/no question and run `on_confirm` if the answer is "Yes".
/// GTK4 dialogs are asynchronous, so the answer arrives via the callback.
pub fn confirm_dialog<F>(parent: &impl IsA<gtk::Window>, title: &str, message: &str, on_confirm: F)
where
    F: Fn() + 'static,
{
    let dialog = MessageDialog::new(
        Some(parent),
        DialogFlags::MODAL,
        MessageType::Question,
        ButtonsType::YesNo,
        message,
    );
    dialog.set_title(Some(title));
    dialog.connect_response(move |dialog, response| {
        dialog.destroy();
        if response == ResponseType::Yes {
            on_confirm();
        }
    });
    dialog.present();
}

/* <--------------------Main Window--------------------> */

pub fn create_main_window(state: &Rc<AppState>) -> ApplicationWindow {
    let window = state.window.clone();
    window.set_title(Some("Notes Manager"));
    window.set_default_size(800, 600);

    // CSS is embedded at compile time, so the working directory does not matter.
    let css_provider = CssProvider::new();
    css_provider.load_from_data(include_str!("css/theme.css"));
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let main_box = GtkBox::new(Orientation::Vertical, 0);
    window.set_child(Some(&main_box));
    main_box.append(&create_toolbar(state));

    let paned = Paned::new(Orientation::Horizontal);
    paned.set_hexpand(true);
    paned.set_vexpand(true);
    main_box.append(&paned);

    let notes_list = create_notes_list(state);
    paned.set_start_child(Some(&notes_list));
    notes_list.set_size_request(250, -1);

    let editor = create_note_editor(state);
    paned.set_end_child(Some(&editor));

    let statusbar = GtkBox::new(Orientation::Horizontal, 0);
    statusbar.add_css_class("statusbar");
    state.status_label.set_xalign(0.0);
    state.status_label.add_css_class("caption");
    statusbar.append(&state.status_label);
    main_box.append(&statusbar);

    window
}
