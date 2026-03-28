use crate::db::sqlite::Db;
use crate::domain::message::{Message, Role};
use crate::skills::r#trait::MemoryUpdate;
use anyhow::Result;
use tracing::info;

pub struct MemoryBridge<'a> {
    db: &'a Db,
    user_id: String,
}

impl<'a> MemoryBridge<'a> {
    pub fn new(db: &'a Db, user_id: &str) -> Self {
        Self {
            db,
            user_id: user_id.to_string(),
        }
    }

    pub fn fetch_context(&self, limit: usize) -> Result<Vec<Message>> {
        self.db
            .fetch_latest_memories(&self.user_id, limit)
            .map_err(Into::into)
    }

    pub fn fetch_conversation_only(&self, limit: usize) -> Result<Vec<Message>> {
        let all = self.db.fetch_latest_memories(&self.user_id, limit)?;
        let filtered: Vec<Message> = all
            .into_iter()
            .filter(|m| {
                !(m.role == Role::System
                    && (m.content.starts_with("MEMORY_SET:")
                        || m.content.starts_with("MEMORY_UPDATE:")
                        || m.content.starts_with("MEMORY_DELETE:")))
            })
            .collect();
        Ok(filtered)
    }

    pub fn fetch_memories_only(&self, scan_limit: usize, take: usize) -> Result<Vec<Message>> {
        let all = self.db.fetch_latest_memories(&self.user_id, scan_limit)?;
        let memories: Vec<Message> = all
            .into_iter()
            .filter(|m| {
                m.role == Role::System
                    && (m.content.starts_with("MEMORY_SET:")
                        || m.content.starts_with("MEMORY_UPDATE:")
                        || m.content.starts_with("MEMORY_DELETE:"))
            })
            .take(take)
            .collect();
        Ok(memories)
    }

    pub fn save_message(&self, message: &Message) -> Result<()> {
        self.db
            .insert_memory(&self.user_id, message)
            .map_err(Into::into)
    }

    pub fn save_memory_update(&self, update: &MemoryUpdate) -> Result<()> {
        let content = match update.operation {
            crate::skills::r#trait::MemoryOperation::Set => {
                format!("MEMORY_SET:{}={}", update.fact_key, update.fact_value)
            }
            crate::skills::r#trait::MemoryOperation::Update => {
                format!("MEMORY_UPDATE:{}={}", update.fact_key, update.fact_value)
            }
            crate::skills::r#trait::MemoryOperation::Delete => {
                format!("MEMORY_DELETE:{}", update.fact_key)
            }
        };

        let existing = self
            .db
            .find_memory_by_key(&self.user_id, &update.fact_key)?;

        if existing.is_some() {
            info!(
                "Memory overwrite: key='{}', operation={:?}",
                update.fact_key, update.operation
            );
            self.db
                .update_memory_by_key(&self.user_id, &update.fact_key, &content)?;
        } else {
            info!(
                "Memory persist: key='{}', operation={:?}",
                update.fact_key, update.operation
            );
            let msg = Message::new(Role::System, content);
            self.db.insert_memory(&self.user_id, &msg)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_memory_update_set() {
        let db = Db::new(":memory:").unwrap();
        let bridge = MemoryBridge::new(&db, "u");
        let update = MemoryUpdate {
            fact_key: "test".to_string(),
            fact_value: "value".to_string(),
            operation: crate::skills::r#trait::MemoryOperation::Set,
        };
        assert!(bridge.save_memory_update(&update).is_ok());
        let memories = db.fetch_latest_memories("u", 10).unwrap();
        assert!(!memories.is_empty());
        assert!(memories[0].content.contains("MEMORY_SET:test=value"));
    }

    #[test]
    fn test_save_memory_update_delete() {
        let db = Db::new(":memory:").unwrap();
        let bridge = MemoryBridge::new(&db, "u");
        let update = MemoryUpdate {
            fact_key: "test".to_string(),
            fact_value: "".to_string(),
            operation: crate::skills::r#trait::MemoryOperation::Delete,
        };
        assert!(bridge.save_memory_update(&update).is_ok());
        let memories = db.fetch_latest_memories("u", 10).unwrap();
        assert!(!memories.is_empty());
        assert!(memories[0].content.contains("MEMORY_DELETE:test"));
    }

    #[test]
    fn test_memory_overwrite_same_key() {
        let db = Db::new(":memory:").unwrap();
        let bridge = MemoryBridge::new(&db, "u");

        let update1 = MemoryUpdate {
            fact_key: "favorite_color".to_string(),
            fact_value: "azul".to_string(),
            operation: crate::skills::r#trait::MemoryOperation::Set,
        };
        assert!(bridge.save_memory_update(&update1).is_ok());

        let update2 = MemoryUpdate {
            fact_key: "favorite_color".to_string(),
            fact_value: "verde".to_string(),
            operation: crate::skills::r#trait::MemoryOperation::Set,
        };
        assert!(bridge.save_memory_update(&update2).is_ok());

        let memories = db.fetch_latest_memories("u", 10).unwrap();
        let memory_messages: Vec<_> = memories
            .iter()
            .filter(|m| m.content.contains("MEMORY_"))
            .collect();
        assert_eq!(
            memory_messages.len(),
            1,
            "Should have only one memory persisted"
        );
        assert!(
            memory_messages[0].content.contains("verde"),
            "Final value should be verde"
        );
    }

    #[test]
    fn test_memory_no_duplicate_in_storage() {
        let db = Db::new(":memory:").unwrap();
        let bridge = MemoryBridge::new(&db, "u");

        for i in 0..5 {
            let update = MemoryUpdate {
                fact_key: "test_key".to_string(),
                fact_value: format!("value_{}", i),
                operation: crate::skills::r#trait::MemoryOperation::Set,
            };
            assert!(bridge.save_memory_update(&update).is_ok());
        }

        let memories = db.fetch_latest_memories("u", 10).unwrap();
        let memory_messages: Vec<_> = memories
            .iter()
            .filter(|m| m.content.contains("test_key"))
            .collect();
        assert_eq!(
            memory_messages.len(),
            1,
            "Same semantic key should result in single DB row"
        );
    }

    #[test]
    fn test_memory_context_window_fallback() {
        let db = Db::new(":memory:").unwrap();
        let bridge = MemoryBridge::new(&db, "u");

        for i in 0..12 {
            let msg = Message::new(Role::User, format!("User message {}", i));
            db.insert_memory("u", &msg).unwrap();
        }

        let update1 = MemoryUpdate {
            fact_key: "favorite_color".to_string(),
            fact_value: "azul".to_string(),
            operation: crate::skills::r#trait::MemoryOperation::Set,
        };
        assert!(bridge.save_memory_update(&update1).is_ok());

        let update2 = MemoryUpdate {
            fact_key: "favorite_color".to_string(),
            fact_value: "verde".to_string(),
            operation: crate::skills::r#trait::MemoryOperation::Set,
        };
        assert!(bridge.save_memory_update(&update2).is_ok());

        let memories = db.fetch_latest_memories("u", 10).unwrap();
        let memory_messages: Vec<_> = memories
            .iter()
            .filter(|m| m.content.contains("favorite_color"))
            .collect();
        assert_eq!(
            memory_messages.len(),
            1,
            "Should overwrite even outside context window"
        );
        assert!(
            memory_messages[0].content.contains("verde"),
            "Final value should be verde"
        );
    }

    #[test]
    fn test_memory_similar_keys_no_collision() {
        let db = Db::new(":memory:").unwrap();
        let bridge = MemoryBridge::new(&db, "u");

        let update_primary = MemoryUpdate {
            fact_key: "favorite_color".to_string(),
            fact_value: "azul".to_string(),
            operation: crate::skills::r#trait::MemoryOperation::Set,
        };
        assert!(bridge.save_memory_update(&update_primary).is_ok());

        let update_secondary = MemoryUpdate {
            fact_key: "favorite_color_secondary".to_string(),
            fact_value: "rojo".to_string(),
            operation: crate::skills::r#trait::MemoryOperation::Set,
        };
        assert!(bridge.save_memory_update(&update_secondary).is_ok());

        let memories = db.fetch_latest_memories("u", 10).unwrap();
        let memory_messages: Vec<_> = memories
            .iter()
            .filter(|m| m.content.contains("MEMORY_"))
            .collect();
        assert_eq!(
            memory_messages.len(),
            2,
            "favorite_color and favorite_color_secondary should be separate keys"
        );
        let primary = memory_messages
            .iter()
            .find(|m| m.content.contains("favorite_color=") && !m.content.contains("secondary"))
            .expect("Should find primary key");
        let secondary = memory_messages
            .iter()
            .find(|m| m.content.contains("favorite_color_secondary"))
            .expect("Should find secondary key");
        assert!(primary.content.contains("azul"), "Primary should be azul");
        assert!(
            secondary.content.contains("rojo"),
            "Secondary should be rojo"
        );
    }
}
