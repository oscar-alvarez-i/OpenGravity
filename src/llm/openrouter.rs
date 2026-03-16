use super::models::LlmProvider;
use crate::config::env::SecretString;
use crate::domain::message::{Message, Role};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

pub struct OpenRouterClient {
    api_key: SecretString,
    client: Client,
    model: String,
    base_url: String,
}

impl OpenRouterClient {
    pub fn new(api_key: SecretString, model: String) -> Self {
        Self {
            api_key,
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            model,
            base_url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
        }
    }

    #[cfg(test)]
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenRouterClient {
    async fn generate_response(&self, system: &str, messages: &[Message]) -> Result<String> {
        let mut api_messages = vec![json!({
            "role": "system",
            "content": system,
        })];

        for m in messages {
            let (role_str, content) = match m.role {
                Role::User => ("user", m.content.clone()),
                Role::Assistant => ("assistant", m.content.clone()),
                Role::System => ("system", m.content.clone()),
                Role::Tool => ("user", format!("TOOL RESULT: {}", m.content)),
            };

            api_messages.push(json!({
                "role": role_str,
                "content": content,
            }));
        }

        let body = json!({
            "model": self.model,
            "messages": api_messages,
            "temperature": 0.0,
        });

        let res = self
            .client
            .post(&self.base_url)
            .header(
                "Authorization",
                format!("Bearer {}", self.api_key.expose_secret()),
            )
            .header("HTTP-Referer", "https://github.com/Oscar/OpenGravity")
            .header("X-Title", "OpenGravity")
            .json(&body)
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            return Err(anyhow!("OpenRouter HTTP error: {}", status));
        }

        let data: serde_json::Value = res.json().await?;
        let content = data["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid OpenRouter response format"))?;

        Ok(content.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::message::{Message, Role};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_openrouter_roles_mapping() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"content": "Roles tested"}}]
            })))
            .mount(&mock_server)
            .await;

        let client = OpenRouterClient::new(
            SecretString::new("sk-123".to_string()),
            "test-model".to_string(),
        )
        .with_base_url(format!("{}/chat", mock_server.uri()));

        let messages = vec![
            Message::new(Role::System, "sys"),
            Message::new(Role::Assistant, "ast"),
            Message::new(Role::User, "usr"),
            Message::new(Role::Tool, "tool"),
        ];

        let res = client.generate_response("sys", &messages).await.unwrap();
        assert_eq!(res, "Roles tested");
    }

    #[tokio::test]
    async fn test_openrouter_success() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"content": "Fallback response"}}]
            })))
            .mount(&mock_server)
            .await;

        let client = OpenRouterClient::new(
            SecretString::new("sk-123".to_string()),
            "test-model".to_string(),
        )
        .with_base_url(format!("{}/chat", mock_server.uri()));

        let res = client.generate_response("sys", &[]).await.unwrap();
        assert_eq!(res, "Fallback response");
    }

    #[tokio::test]
    async fn test_openrouter_error() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client =
            OpenRouterClient::new(SecretString::new("sk-123".to_string()), "test".to_string())
                .with_base_url(format!("{}/chat", mock_server.uri()));

        let res = client.generate_response("sys", &[]).await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("OpenRouter HTTP error: 500 Internal Server Error"));
    }

    #[tokio::test]
    async fn test_openrouter_invalid_json() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "wrong": "format"
            })))
            .mount(&mock_server)
            .await;

        let client =
            OpenRouterClient::new(SecretString::new("sk-123".to_string()), "test".to_string())
                .with_base_url(format!("{}/chat", mock_server.uri()));

        let res = client.generate_response("sys", &[]).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_openrouter_content_missing() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {}}]
            })))
            .mount(&mock_server)
            .await;

        let client =
            OpenRouterClient::new(SecretString::new("sk-123".to_string()), "test".to_string())
                .with_base_url(format!("{}/chat", mock_server.uri()));

        let res = client.generate_response("sys", &[]).await;
        assert!(res.is_err());
    }
}
