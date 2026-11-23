// Notes Manager - GTK4 notes application backed by SQLite

mod database;
mod notes;
mod ui;

use std::fs;
use std::path::PathBuf;

use gtk4 as gtk;

use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;

use database::Database;

/// Where the notes database lives: `$NOTES_MANAGER_DB` if set, otherwise
/// `notes-manager/notes.db` in the user's data directory.
fn db_path() -> PathBuf {
    match std::env::var_os("NOTES_MANAGER_DB") {
        Some(path) => PathBuf::from(path),
        None => glib::user_data_dir().join("notes-manager").join("notes.db"),
    }
}

fn open_database() -> (Database, Option<&'static str>) {
    let path = db_path();
    if let Some(dir) = path.parent() {
        if let Err(err) = fs::create_dir_all(dir) {
            eprintln!("Could not create {}: {err}", dir.display());
        }
    }

    match Database::open(&path) {
        Ok(database) => (database, None),
        Err(err) => {
            eprintln!("Could not open database {}: {err}", path.display());
            let database =
                Database::open_in_memory().expect("failed to create in-memory SQLite database");
            (
                database,
                Some("Could not open the database: notes will not be saved"),
            )
        }
    }
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("org.parth.notes")
        .build();
    app.connect_activate(activate);
    app.run()
}

fn activate(app: &adw::Application) {
    let (database, warning) = open_database();

    let state = ui::AppState::new(app, database);
    let window = ui::create_main_window(&state);

    state.reload_list(&[]);
    window.present();

    if let Some(warning) = warning {
        state.update_status(warning);
    }
}
