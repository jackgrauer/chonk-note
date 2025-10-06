// Notes mode for Chonker7
use anyhow::Result;
use crate::notes_database::{NotesDatabase, Note};

pub struct NotesMode {
    pub db: NotesDatabase,
    pub current_note: Option<Note>,
    pub search_results: Vec<Note>,
}

impl NotesMode {
    pub fn new() -> Result<Self> {
        Ok(Self {
            db: NotesDatabase::new()?,
            current_note: None,
            search_results: Vec::new(),
        })
    }
}
