use crate::domain::message::Message;
use anyhow::Result;

#[cfg(test)]
use mockall::automock;

#[async_trait::async_trait]
#[cfg_attr(test, automock)]
pub trait LlmProvider: Send + Sync {
    /// Generates a response from the LLM given a system prompt and conversation history.
    async fn generate_response(&self, system: &str, messages: &[Message]) -> Result<String>;
}
