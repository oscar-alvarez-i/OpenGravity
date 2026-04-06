use super::schema::{CREATE_MEMORIES_TABLE, INDEX_MEMORIES_USER_ID};
use crate::domain::message::{Message, Role};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult};
use std::sync::Mutex;

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Opens the SQLite database safely enforcing boundaries.
    pub fn new(path: &str) -> SqliteResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute(CREATE_MEMORIES_TABLE, [])?;
        conn.execute(INDEX_MEMORIES_USER_ID, [])?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Appends a new memory to the user_id's conversational history.
    pub fn insert_memory(&self, user_id: &str, msg: &Message) -> SqliteResult<()> {
        let role_str = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Tool => "tool",
        };
        let created_at = Utc::now().to_rfc3339();

        self.conn.lock().unwrap().execute(
            "INSERT INTO memories (user_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![user_id, role_str, msg.content, created_at],
        )?;

        Ok(())
    }

    /// Fetches the latest N memories for a specific user.
    pub fn fetch_latest_memories(&self, user_id: &str, limit: usize) -> SqliteResult<Vec<Message>> {
        let conn_lock = self.conn.lock().unwrap();
        let mut stmt = conn_lock.prepare(
            "SELECT role, content FROM memories WHERE user_id = ?1 ORDER BY created_at DESC LIMIT ?2"
        )?;

        let memory_iter = stmt.query_map(params![user_id, limit as i64], |row| {
            let role_str: String = row.get(0)?;
            let content: String = row.get(1)?;

            let role = match role_str.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                "tool" => Role::Tool,
                _ => Role::User, // Fallback, though ideally handled strictly.
            };

            Ok(Message::new(role, content))
        })?;

        let mut memories: Vec<Message> = memory_iter.filter_map(Result::ok).collect();
        // Since we ordered by DESC (to get the latest X), we need to reverse them back to chronological order
        memories.reverse();

        Ok(memories)
    }

    pub fn find_memory_by_key(
        &self,
        user_id: &str,
        fact_key: &str,
    ) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let pattern_set = format!("%MEMORY_SET:{}=%", fact_key);
        let pattern_update = format!("%MEMORY_UPDATE:{}=%", fact_key);
        let mut stmt = conn.prepare(
            "SELECT content FROM memories WHERE user_id = ?1 AND (content LIKE ?2 OR content LIKE ?3) LIMIT 1",
        )?;
        let result = stmt
            .query_row(params![user_id, pattern_set, pattern_update], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(result)
    }

    pub fn update_memory_by_key(
        &self,
        user_id: &str,
        fact_key: &str,
        new_content: &str,
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock().unwrap();
        let pattern_set = format!("%MEMORY_SET:{}=%", fact_key);
        let pattern_update = format!("%MEMORY_UPDATE:{}=%", fact_key);
        let created_at = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE memories SET content = ?1, created_at = ?2 WHERE user_id = ?3 AND (content LIKE ?4 OR content LIKE ?5)",
            params![new_content, created_at, user_id, pattern_set, pattern_update],
        )
    }

    #[cfg(test)]
    pub fn execute_raw(&self, sql: &str) -> SqliteResult<usize> {
        self.conn.lock().unwrap().execute(sql, [])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_insert_and_fetch() {
        // Use an in-memory db for testing
        let db = Db::new(":memory:").expect("Failed to create in-memory db");

        let msg1 = Message::new(Role::User, "Hello gravity");
        let msg2 = Message::new(Role::Assistant, "Hi there");

        db.insert_memory("123", &msg1).unwrap();
        db.insert_memory("123", &msg2).unwrap();

        let fetched = db.fetch_latest_memories("123", 10).unwrap();
        assert_eq!(fetched.len(), 2);
        assert_eq!(fetched[0].content, "Hello gravity");
        assert_eq!(fetched[1].content, "Hi there");
    }

    #[test]
    fn test_memory_limit() {
        let db = Db::new(":memory:").expect("Failed to create in-memory db");

        for i in 0..15 {
            let msg = Message::new(Role::User, format!("Msg {}", i));
            db.insert_memory("123", &msg).unwrap();
        }

        let fetched = db.fetch_latest_memories("123", 10).unwrap();
        assert_eq!(fetched.len(), 10);
        // The last one should be Msg 14
        assert_eq!(fetched[9].content, "Msg 14");
        // The first one should be Msg 5
        assert_eq!(fetched[0].content, "Msg 5");
    }

    #[test]
    fn test_memory_insert_system_and_tool_roles() {
        let db = Db::new(":memory:").unwrap();
        db.insert_memory("123", &Message::new(Role::System, "sys"))
            .unwrap();
        db.insert_memory("123", &Message::new(Role::Tool, "tool"))
            .unwrap();
        let fetched = db.fetch_latest_memories("123", 2).unwrap();
        assert_eq!(fetched.len(), 2);
    }

    /// Tests that an unknown role string in the database falls back to Role::User.
    #[test]
    fn test_memory_unknown_role_fallback() {
        let db = Db::new(":memory:").unwrap();
        // Insert a row with an unknown role directly via SQL
        let conn = db.conn.lock().unwrap();
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO memories (user_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
            params!["123", "unknown_role", "fallback test", created_at],
        )
        .unwrap();
        drop(conn);
        let fetched = db.fetch_latest_memories("123", 1).unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].role, Role::User); // Fallback
        assert_eq!(fetched[0].content, "fallback test");
    }

    #[test]
    fn test_db_new_invalid_path() {
        let res = Db::new("/invalid/path/to/db");
        assert!(res.is_err());
    }
}
