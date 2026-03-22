use crate::db::sqlite::Db;
use crate::domain::message::{Message, Role};
use crate::skills::r#trait::MemoryUpdate;
use anyhow::Result;

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
        let msg = Message::new(Role::System, content);
        self.db
            .insert_memory(&self.user_id, &msg)
            .map_err(Into::into)
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
}
