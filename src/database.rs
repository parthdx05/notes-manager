//! SQLite-backed note storage.
//!
//! Every change is written to the database immediately, so there is no
//! separate "save" step and nothing is lost if the application is closed or
//! crashes.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Result, Row};

use crate::notes::{now, Note};

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS notes (
        id          TEXT    PRIMARY KEY NOT NULL,
        title       TEXT    NOT NULL,
        content     TEXT    NOT NULL,
        created_at  INTEGER NOT NULL,
        modified_at INTEGER NOT NULL
    );
";

const SELECT_NOTE: &str = "SELECT id, title, content, created_at, modified_at FROM notes";

pub struct Database {
    conn: Connection,
}

fn note_from_row(row: &Row<'_>) -> Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
        modified_at: row.get(4)?,
    })
}

impl Database {
    /// Open (creating if necessary) the database file at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    /// A throwaway database that lives only in memory.
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(SCHEMA)?;
        Ok(Database { conn })
    }

    /// Insert a note.
    pub fn add_note(&self, note: &Note) -> Result<()> {
        self.conn.execute(
            "INSERT INTO notes (id, title, content, created_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                note.id,
                note.title,
                note.content,
                note.created_at,
                note.modified_at
            ],
        )?;
        Ok(())
    }

    /// Delete a note by ID. Returns whether a note was deleted.
    pub fn remove_note(&self, note_id: &str) -> Result<bool> {
        let changed = self
            .conn
            .execute("DELETE FROM notes WHERE id = ?1", params![note_id])?;
        Ok(changed > 0)
    }

    /// Delete several notes in one transaction (all or none). Returns how
    /// many notes were actually deleted; unknown IDs are ignored.
    pub fn remove_notes(&self, note_ids: &[String]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut removed = 0;
        {
            let mut stmt = tx.prepare("DELETE FROM notes WHERE id = ?1")?;
            for id in note_ids {
                removed += stmt.execute(params![id])?;
            }
        }
        tx.commit()?;
        Ok(removed)
    }

    /// Look up a note by ID.
    pub fn find_note(&self, note_id: &str) -> Result<Option<Note>> {
        self.conn
            .query_row(
                &format!("{SELECT_NOTE} WHERE id = ?1"),
                params![note_id],
                note_from_row,
            )
            .optional()
    }

    /// Change a note's title and refresh its modification time.
    /// Returns whether the note exists.
    pub fn set_title(&self, note_id: &str, title: &str) -> Result<bool> {
        let changed = self.conn.execute(
            "UPDATE notes SET title = ?1, modified_at = ?2 WHERE id = ?3",
            params![title, now(), note_id],
        )?;
        Ok(changed > 0)
    }

    /// Change a note's content and refresh its modification time.
    /// Returns whether the note exists.
    pub fn set_content(&self, note_id: &str, content: &str) -> Result<bool> {
        let changed = self.conn.execute(
            "UPDATE notes SET content = ?1, modified_at = ?2 WHERE id = ?3",
            params![content, now(), note_id],
        )?;
        Ok(changed > 0)
    }

    /// All notes, oldest first.
    pub fn all_notes(&self) -> Result<Vec<Note>> {
        let mut stmt = self
            .conn
            .prepare(&format!("{SELECT_NOTE} ORDER BY created_at, rowid"))?;
        let notes = stmt
            .query_map([], note_from_row)?
            .collect::<Result<Vec<_>>>()?;
        Ok(notes)
    }

    /// Notes whose title contains `query`, ignoring ASCII case, oldest first.
    ///
    /// `instr` is used instead of `LIKE` so `%` and `_` in the query are
    /// matched literally.
    pub fn search(&self, query: &str) -> Result<Vec<Note>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SELECT_NOTE} WHERE instr(lower(title), lower(?1)) > 0
             ORDER BY created_at, rowid"
        ))?;
        let notes = stmt
            .query_map(params![query], note_from_row)?
            .collect::<Result<Vec<_>>>()?;
        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_with(titles: &[&str]) -> Database {
        let db = Database::open_in_memory().unwrap();
        for title in titles {
            db.add_note(&Note::new(title, "body")).unwrap();
        }
        db
    }

    fn titles(notes: &[Note]) -> Vec<&str> {
        notes.iter().map(|n| n.title.as_str()).collect()
    }

    #[test]
    fn add_and_find_round_trip() {
        let db = Database::open_in_memory().unwrap();
        let note = Note::new("Title", "Body");
        db.add_note(&note).unwrap();
        assert_eq!(db.find_note(&note.id).unwrap(), Some(note));
        assert_eq!(db.find_note("missing").unwrap(), None);
    }

    #[test]
    fn all_notes_keep_insertion_order() {
        let db = db_with(&["b", "a", "c"]);
        assert_eq!(titles(&db.all_notes().unwrap()), ["b", "a", "c"]);
    }

    #[test]
    fn remove_note_reports_whether_it_existed() {
        let db = db_with(&["one", "two"]);
        let id = db.all_notes().unwrap()[0].id.clone();
        assert!(db.remove_note(&id).unwrap());
        assert!(!db.remove_note(&id).unwrap());
        assert_eq!(titles(&db.all_notes().unwrap()), ["two"]);
    }

    #[test]
    fn deleting_down_to_one_note_and_then_zero_works() {
        let db = db_with(&["one", "two", "three"]);
        for note in db.all_notes().unwrap().iter().take(2) {
            db.remove_note(&note.id).unwrap();
        }
        let last = db.all_notes().unwrap();
        assert_eq!(titles(&last), ["three"]);
        assert!(db.set_title(&last[0].id, "renamed").unwrap());
        assert!(db.set_content(&last[0].id, "edited").unwrap());
        assert!(db.remove_note(&last[0].id).unwrap());
        assert!(db.all_notes().unwrap().is_empty());
    }

    #[test]
    fn remove_notes_deletes_all_listed_ids_at_once() {
        let db = db_with(&["a", "b", "c", "d"]);
        let all = db.all_notes().unwrap();
        let ids = vec![all[0].id.clone(), all[2].id.clone(), "missing".to_owned()];
        assert_eq!(db.remove_notes(&ids).unwrap(), 2);
        assert_eq!(titles(&db.all_notes().unwrap()), ["b", "d"]);
        assert_eq!(db.remove_notes(&[]).unwrap(), 0);
        assert_eq!(titles(&db.all_notes().unwrap()), ["b", "d"]);
    }

    #[test]
    fn set_title_and_content_update_the_note() {
        let db = db_with(&["old"]);
        let id = db.all_notes().unwrap()[0].id.clone();
        assert!(db.set_title(&id, "new").unwrap());
        assert!(db.set_content(&id, "text").unwrap());
        let note = db.find_note(&id).unwrap().unwrap();
        assert_eq!(note.title, "new");
        assert_eq!(note.content, "text");
        assert!(note.modified_at >= note.created_at);
        assert!(!db.set_title("missing", "x").unwrap());
    }

    #[test]
    fn search_ignores_ascii_case_and_matches_titles_only() {
        let db = Database::open_in_memory().unwrap();
        db.add_note(&Note::new("Shopping List", "milk")).unwrap();
        db.add_note(&Note::new("Work notes", "standup")).unwrap();
        assert_eq!(titles(&db.search("SHOP").unwrap()), ["Shopping List"]);
        assert_eq!(titles(&db.search("notes").unwrap()), ["Work notes"]);
        assert!(db.search("milk").unwrap().is_empty());
    }

    #[test]
    fn search_treats_like_wildcards_literally() {
        let db = db_with(&["100% done", "plain"]);
        assert_eq!(titles(&db.search("%").unwrap()), ["100% done"]);
        assert_eq!(db.search("_").unwrap().len(), 0);
    }
}
