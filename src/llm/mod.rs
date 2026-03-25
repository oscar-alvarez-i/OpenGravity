pub mod groq;
pub mod models;
pub mod openrouter;

use crate::domain::message::Message;
use anyhow::Result;
use tracing::{debug, warn};

pub struct LlmOrchestrator {
    providers: Vec<Box<dyn models::LlmProvider>>,
}

impl LlmOrchestrator {
    pub fn new(providers: Vec<Box<dyn models::LlmProvider>>) -> Self {
        Self { providers }
    }

    pub async fn generate(&self, system: &str, messages: &[Message]) -> Result<String> {
        let mut last_error: Option<anyhow::Error> = None;

        for (i, provider) in self.providers.iter().enumerate() {
            match provider.generate_response(system, messages).await {
                Ok(response) => {
                    debug!("LLM response generated via provider[{}]", i);
                    return Ok(response);
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    if err_msg.contains("groq_fallback_required") {
                        warn!(
                            "Provider[{}] unavailable ({}). Trying next provider.",
                            i, err_msg
                        );
                        last_error = Some(e);
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        if let Some(err) = last_error {
            warn!("All providers failed, returning fallback error");
            Err(err)
        } else {
            Err(anyhow::anyhow!("No providers available"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::models::MockLlmProvider;

    #[tokio::test]
    async fn test_orchestrator_first_provider_success() {
        let mut provider_mock = MockLlmProvider::new();
        provider_mock
            .expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("First response".to_string()) }));

        let orchestrator = LlmOrchestrator::new(vec![Box::new(provider_mock)]);
        let res = orchestrator.generate("system", &[]).await.unwrap();

        assert_eq!(res, "First response");
    }

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

        let orchestrator = LlmOrchestrator::new(vec![Box::new(groq_mock), Box::new(or_mock)]);
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

        let orchestrator = LlmOrchestrator::new(vec![Box::new(groq_mock), Box::new(or_mock)]);
        let res = orchestrator.generate("system", &[]).await;

        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Fatal parse error");
    }

    #[tokio::test]
    async fn test_orchestrator_all_fallback_errors() {
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
            .returning(|_, _| {
                Box::pin(async { Err(anyhow::anyhow!("openrouter_fallback_required")) })
            });

        let orchestrator = LlmOrchestrator::new(vec![Box::new(groq_mock), Box::new(or_mock)]);
        let res = orchestrator.generate("system", &[]).await;

        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("openrouter_fallback_required"));
    }

    #[tokio::test]
    async fn test_orchestrator_no_providers() {
        let orchestrator = LlmOrchestrator::new(vec![]);
        let res = orchestrator.generate("system", &[]).await;

        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "No providers available");
    }
}
