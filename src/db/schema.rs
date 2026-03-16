pub const CREATE_MEMORIES_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS memories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
);
"#;

pub const INDEX_MEMORIES_USER_ID: &str = r#"
CREATE INDEX IF NOT EXISTS idx_memories_user_id ON memories(user_id);
"#;
