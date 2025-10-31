// Simple file-based notes system
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub path: PathBuf,
}

pub struct NotesManager {
    notes_dir: PathBuf,
}

impl NotesManager {
    pub fn new() -> Result<Self> {
        let mut notes_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find data directory"))?;
        notes_dir.push("chonk-note");
        notes_dir.push("notes");
        fs::create_dir_all(&notes_dir)?;

        Ok(Self { notes_dir })
    }

    /// List all notes sorted by modification time (newest first)
    pub fn list_notes(&self) -> Result<Vec<Note>> {
        let mut notes = Vec::new();

        for entry in fs::read_dir(&self.notes_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("txt") {
                if let Ok(content) = fs::read_to_string(&path) {
                    let lines: Vec<&str> = content.lines().collect();
                    let title = lines.first().unwrap_or(&"Untitled").to_string();
                    let id = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    notes.push(Note {
                        id,
                        title,
                        content,
                        path: path.clone(),
                    });
                }
            }
        }

        // Sort by modification time (newest first)
        notes.sort_by(|a, b| {
            let a_time = fs::metadata(&a.path).and_then(|m| m.modified()).ok();
            let b_time = fs::metadata(&b.path).and_then(|m| m.modified()).ok();
            b_time.cmp(&a_time)
        });

        Ok(notes)
    }

    /// Create a new note with a unique ID
    pub fn create_note(&self, title: &str, content: &str) -> Result<Note> {
        // Generate unique ID based on timestamp
        let id = format!("note_{}", chrono::Utc::now().timestamp());
        let filename = format!("{}.txt", id);
        let path = self.notes_dir.join(filename);

        // Write note with title on first line, then content
        let full_content = if content.is_empty() {
            title.to_string()
        } else {
            format!("{}\n{}", title, content)
        };

        fs::write(&path, &full_content)?;

        Ok(Note {
            id,
            title: title.to_string(),
            content: full_content,
            path,
        })
    }

    /// Save note content (title is first line)
    pub fn save_note(&self, note: &Note) -> Result<()> {
        fs::write(&note.path, &note.content)?;
        Ok(())
    }

    /// Delete a note
    pub fn delete_note(&self, note: &Note) -> Result<()> {
        fs::remove_file(&note.path)?;
        Ok(())
    }

    /// Get note by ID
    pub fn get_note(&self, id: &str) -> Result<Option<Note>> {
        let filename = format!("{}.txt", id);
        let path = self.notes_dir.join(filename);

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)?;
        let lines: Vec<&str> = content.lines().collect();
        let title = lines.first().unwrap_or(&"Untitled").to_string();

        Ok(Some(Note {
            id: id.to_string(),
            title,
            content,
            path,
        }))
    }
}
