pub mod groq;
pub mod models;
pub mod openrouter;

use crate::domain::message::Message;
use anyhow::Result;
use tracing::{debug, warn};

pub struct LlmOrchestrator {
    groq: Box<dyn models::LlmProvider>,
    openrouter: Box<dyn models::LlmProvider>,
}

impl LlmOrchestrator {
    pub fn new(
        groq: Box<dyn models::LlmProvider>,
        openrouter: Box<dyn models::LlmProvider>,
    ) -> Self {
        Self { groq, openrouter }
    }

    pub async fn generate(&self, system: &str, messages: &[Message]) -> Result<String> {
        match self.groq.generate_response(system, messages).await {
            Ok(response) => {
                debug!("LLM response generated via Groq");
                Ok(response)
            }
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("groq_fallback_required") {
                    warn!(
                        "Groq unavailable ({}). Falling back to OpenRouter.",
                        err_msg
                    );
                    let resp = self.openrouter.generate_response(system, messages).await?;
                    debug!("LLM response generated via OpenRouter (Fallback)");
                    Ok(resp)
                } else {
                    // Non-fallbackable error (e.g. fatal JSON parse error on successful 200)
                    Err(e)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::models::MockLlmProvider;

    #[tokio::test]
    async fn test_orchestrator_fallback() {
        let mut groq_mock = MockLlmProvider::new();
        groq_mock
            .expect_generate_response()
            .times(1)
            .returning(|_, _| {
                Box::pin(async { Err(anyhow::anyhow!("groq_fallback_required: Timeout")) })
            });

        let mut or_mock = MockLlmProvider::new();
        or_mock
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("Fallback response".to_string()) }));

        let orchestrator = LlmOrchestrator::new(Box::new(groq_mock), Box::new(or_mock));
        let res = orchestrator.generate("system", &[]).await.unwrap();

        assert_eq!(res, "Fallback response");
    }

    #[tokio::test]
    async fn test_orchestrator_non_fallback_error() {
        let mut groq_mock = MockLlmProvider::new();
        groq_mock
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(anyhow::anyhow!("Fatal parse error")) }));

        let mut or_mock = MockLlmProvider::new();
        or_mock.expect_generate_response().times(0);

        let orchestrator = LlmOrchestrator::new(Box::new(groq_mock), Box::new(or_mock));
        let res = orchestrator.generate("system", &[]).await;

        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Fatal parse error");
    }
}
