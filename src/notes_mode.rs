// Notes mode for Chonker7
use anyhow::Result;
use helix_core::{Rope, Selection, Transaction, Range};
use crate::notes_database::{NotesDatabase, Note};

pub struct NotesMode {
    db: NotesDatabase,
    current_note: Option<Note>,
    search_results: Vec<Note>,
}

impl NotesMode {
    pub fn new() -> Result<Self> {
        Ok(Self {
            db: NotesDatabase::new()?,
            current_note: None,
            search_results: Vec::new(),
        })
    }

    pub fn handle_command(&mut self, rope: &mut Rope, selection: &mut Selection, cmd: &str) -> Result<Option<String>> {
        match cmd {
            "new" => self.create_new_note(rope, selection),
            "save" => self.save_current_note(rope),
            "search" => self.start_search(),
            "list" => self.list_all_notes(rope, selection),
            _ if cmd.starts_with("open ") => {
                let id = cmd.strip_prefix("open ").unwrap();
                self.open_note(rope, selection, id)
            },
            _ if cmd.starts_with("delete ") => {
                let id = cmd.strip_prefix("delete ").unwrap();
                self.delete_note(id)
            },
            _ => Ok(None), // Pass through to main editor
        }
    }

    fn create_new_note(&mut self, rope: &mut Rope, selection: &mut Selection) -> Result<Option<String>> {
        let title = "# New Note\n\nStart typing here...\n\nTags: ";
        let new_rope = Rope::from_str(title);
        *rope = new_rope;
        *selection = Selection::single(11, 11); // Position after "# New Note\n"
        self.current_note = None;
        Ok(Some("Created new note".to_string()))
    }

    fn save_current_note(&mut self, rope: &Rope) -> Result<Option<String>> {
        let content = rope.to_string();

        // Extract title from first line
        let title = content.lines()
            .next()
            .unwrap_or("Untitled")
            .trim_start_matches('#')
            .trim()
            .to_string();

        // Extract tags if present
        let tags = extract_tags(&content);

        if let Some(ref note) = self.current_note {
            // Update existing note
            self.db.update_note(&note.id, title, content, tags)?;
            Ok(Some("Note updated".to_string()))
        } else {
            // Create new note
            let note = self.db.create_note(title, content, tags)?;
            self.current_note = Some(note.clone());
            Ok(Some(format!("Note saved with ID: {}", note.id)))
        }
    }

    fn start_search(&mut self) -> Result<Option<String>> {
        // In real implementation, this would open a search dialog
        // For now, we'll just return a message
        Ok(Some("Search: Use /search <query> command".to_string()))
    }

    pub fn search(&mut self, query: &str) -> Result<Option<String>> {
        self.search_results = self.db.search_notes(query)?;
        if self.search_results.is_empty() {
            Ok(Some("No notes found".to_string()))
        } else {
            let msg = format!("Found {} notes. Use /open <id> to open", self.search_results.len());
            Ok(Some(msg))
        }
    }

    fn list_all_notes(&mut self, rope: &mut Rope, selection: &mut Selection) -> Result<Option<String>> {
        let notes = self.db.list_notes(50)?;
        let mut content = String::from("# All Notes\n\n");

        for note in notes {
            content.push_str(&format!(
                "## {} [{}]\n{}\nTags: {}\nUpdated: {}\n\n---\n\n",
                note.title,
                note.id,
                &note.content[..note.content.len().min(200)],
                note.tags.join(", "),
                note.updated_at.format("%Y-%m-%d %H:%M")
            ));
        }

        let new_rope = Rope::from_str(&content);
        *rope = new_rope;
        *selection = Selection::single(0, 0);
        Ok(Some("Notes list loaded".to_string()))
    }

    fn open_note(&mut self, rope: &mut Rope, selection: &mut Selection, id: &str) -> Result<Option<String>> {
        if let Some(note) = self.db.get_note(id)? {
            let new_rope = Rope::from_str(&note.content);
            *rope = new_rope;
            *selection = Selection::single(0, 0);
            self.current_note = Some(note.clone());
            Ok(Some(format!("Opened: {}", note.title)))
        } else {
            Ok(Some("Note not found".to_string()))
        }
    }

    fn delete_note(&mut self, id: &str) -> Result<Option<String>> {
        self.db.delete_note(id)?;
        if let Some(ref current) = self.current_note {
            if current.id == id {
                self.current_note = None;
            }
        }
        Ok(Some(format!("Deleted note: {}", id)))
    }

    pub fn get_search_results(&self) -> &[Note] {
        &self.search_results
    }
}

fn extract_tags(content: &str) -> Vec<String> {
    for line in content.lines() {
        if line.starts_with("Tags:") {
            let tags_str = line.trim_start_matches("Tags:").trim();
            return tags_str.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    Vec::new()
}