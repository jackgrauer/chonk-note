// Simple script to create two test notes
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut path = dirs::data_local_dir()
        .ok_or("Could not find data directory")?;
    path.push("chonk-note");
    std::fs::create_dir_all(&path)?;
    path.push("notes.db");

    let conn = rusqlite::Connection::open(path)?;

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
        "CREATE TABLE IF NOT EXISTS tags (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            created_at DATETIME NOT NULL
        )",
        [],
    )?;

    // Clear existing notes
    conn.execute("DELETE FROM notes", [])?;

    // Create two test notes
    let note1_id = uuid::Uuid::new_v4().to_string();
    let note2_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO notes (id, title, content, tags, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            &note1_id,
            "First Note",
            "This is the first test note.\nYou can edit this content.\n\nIt has multiple lines!",
            "[]",
            &now,
            &now
        ],
    )?;

    conn.execute(
        "INSERT INTO notes (id, title, content, tags, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            &note2_id,
            "Second Note",
            "This is the second test note.\nIt also has some content.\n\nYou can switch between notes using the menu!",
            "[]",
            &now,
            &now
        ],
    )?;

    println!("✅ Created 2 test notes successfully!");
    Ok(())
}
