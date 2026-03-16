use crate::db::sqlite::Db;
use crate::domain::message::Message;
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
}
