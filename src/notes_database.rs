// Notes database for Chonker7
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct NotesDatabase {
    conn: Connection,
}

impl NotesDatabase {
    pub fn new() -> Result<Self> {
        let mut path = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find data directory"))?;
        path.push("chonker7");
        path.push("notes");
        std::fs::create_dir_all(&path)?;
        path.push("notes.db");

        let conn = Connection::open(path)?;

        // Create tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                tags TEXT NOT NULL,
                created_at DATETIME NOT NULL,
                updated_at DATETIME NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_notes_updated
             ON notes(updated_at DESC)",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn create_note(&self, title: String, content: String, tags: Vec<String>) -> Result<Note> {
        let now = Utc::now();
        let id = self.generate_id(&title, &now);
        let tags_json = serde_json::to_string(&tags)?;

        self.conn.execute(
            "INSERT INTO notes (id, title, content, tags, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, title, content, tags_json, now.to_rfc3339(), now.to_rfc3339()],
        )?;

        Ok(Note {
            id,
            title,
            content,
            tags,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn update_note(&self, id: &str, title: String, content: String, tags: Vec<String>) -> Result<()> {
        let now = Utc::now();
        let tags_json = serde_json::to_string(&tags)?;

        self.conn.execute(
            "UPDATE notes SET title = ?1, content = ?2, tags = ?3, updated_at = ?4
             WHERE id = ?5",
            params![title, content, tags_json, now.to_rfc3339(), id],
        )?;

        Ok(())
    }

    pub fn update_note_title(&self, id: &str, title: &str) -> Result<()> {
        let now = Utc::now();

        self.conn.execute(
            "UPDATE notes SET title = ?1, updated_at = ?2
             WHERE id = ?3",
            params![title, now.to_rfc3339(), id],
        )?;

        Ok(())
    }

    pub fn get_note(&self, id: &str) -> Result<Option<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, content, tags, created_at, updated_at
             FROM notes WHERE id = ?1"
        )?;

        let note = stmt.query_row([id], |row| {
            let tags_json: String = row.get(3)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                tags,
                created_at: row.get::<_, String>(4)?.parse().unwrap_or_else(|_| Utc::now()),
                updated_at: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| Utc::now()),
            })
        }).optional()?;

        Ok(note)
    }

    pub fn list_notes(&self, limit: usize) -> Result<Vec<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, content, tags, created_at, updated_at
             FROM notes
             ORDER BY updated_at DESC
             LIMIT ?1"
        )?;

        let notes = stmt.query_map([limit], |row| {
            let tags_json: String = row.get(3)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                tags,
                created_at: row.get::<_, String>(4)?.parse().unwrap_or_else(|_| Utc::now()),
                updated_at: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| Utc::now()),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(notes)
    }

    pub fn delete_note(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM notes WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn search_notes(&self, query: &str) -> Result<Vec<Note>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT id, title, content, tags, created_at, updated_at
             FROM notes
             WHERE title LIKE ?1 OR content LIKE ?1 OR tags LIKE ?1
             ORDER BY updated_at DESC
             LIMIT 50"
        )?;

        let notes = stmt.query_map([pattern], |row| {
            let tags_json: String = row.get(3)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                tags,
                created_at: row.get::<_, String>(4)?.parse().unwrap_or_else(|_| Utc::now()),
                updated_at: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| Utc::now()),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(notes)
    }

    fn generate_id(&self, title: &str, timestamp: &DateTime<Utc>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(title.as_bytes());
        hasher.update(timestamp.to_rfc3339().as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)[..8].to_string()
    }
}