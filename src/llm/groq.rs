use super::models::LlmProvider;
use crate::config::env::SecretString;
use crate::domain::message::{Message, Role};
use anyhow::{anyhow, Result};
use reqwest::{Client, StatusCode};
use serde_json::json;
use std::time::Duration;

pub struct GroqClient {
    api_key: SecretString,
    client: Client,
    model: String,
    base_url: String,
}

impl GroqClient {
    pub fn new(api_key: SecretString) -> Self {
        Self {
            api_key,
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
            model: "llama-3.3-70b-versatile".to_string(), // Stable fast model as requested implicitly by Groq usage
            base_url: "https://api.groq.com/openai/v1/chat/completions".to_string(),
        }
    }

    #[cfg(test)]
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }
}

#[async_trait::async_trait]
impl LlmProvider for GroqClient {
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
            .json(&body)
            .send()
            .await;

        match res {
            Ok(response) => {
                let status = response.status();
                if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
                    // Fallback on 5xx or 429
                    return Err(anyhow!("groq_fallback_required: {}", status));
                }

                if !status.is_success() {
                    return Err(anyhow!("Groq HTTP error: {}", status));
                }

                let data: serde_json::Value = response.json().await?;
                let content = data["choices"][0]["message"]["content"]
                    .as_str()
                    .ok_or_else(|| anyhow!("Invalid Groq response format: missing content"))?;
                Ok(content.to_string())
            }
            Err(e) => {
                // Timeout or network error
                Err(anyhow!("groq_fallback_required: {}", e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::message::{Message, Role};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_groq_roles_mapping() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"content": "Roles tested"}}]
            })))
            .mount(&mock_server)
            .await;

        let client = GroqClient::new(SecretString::new("sk-123".to_string()))
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
    async fn test_groq_success() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"content": "Hello world"}}]
            })))
            .mount(&mock_server)
            .await;

        let client = GroqClient::new(SecretString::new("sk-123".to_string()))
            .with_base_url(format!("{}/chat", mock_server.uri()));

        let res = client.generate_response("sys", &[]).await.unwrap();
        assert_eq!(res, "Hello world");
    }

    #[tokio::test]
    async fn test_groq_429_fallback() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&mock_server)
            .await;

        let client = GroqClient::new(SecretString::new("sk-123".to_string()))
            .with_base_url(format!("{}/chat", mock_server.uri()));

        let res = client.generate_response("sys", &[]).await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("groq_fallback_required: 429 Too Many Requests"));
    }

    #[tokio::test]
    async fn test_groq_500_fallback() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = GroqClient::new(SecretString::new("sk-123".to_string()))
            .with_base_url(format!("{}/chat", mock_server.uri()));

        let res = client.generate_response("sys", &[]).await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("groq_fallback_required: 500 Internal Server Error"));
    }

    #[tokio::test]
    async fn test_groq_400_error() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(400))
            .mount(&mock_server)
            .await;

        let client = GroqClient::new(SecretString::new("sk-123".to_string()))
            .with_base_url(format!("{}/chat", mock_server.uri()));

        let res = client.generate_response("sys", &[]).await;
        assert!(res.is_err());
        assert!(!res
            .unwrap_err()
            .to_string()
            .contains("groq_fallback_required"));
    }

    #[tokio::test]
    async fn test_groq_invalid_json() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "wrong": "format"
            })))
            .mount(&mock_server)
            .await;

        let client = GroqClient::new(SecretString::new("sk-123".to_string()))
            .with_base_url(format!("{}/chat", mock_server.uri()));

        let res = client.generate_response("sys", &[]).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_groq_content_missing() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {}}]
            })))
            .mount(&mock_server)
            .await;

        let client = GroqClient::new(SecretString::new("sk-123".to_string()))
            .with_base_url(format!("{}/chat", mock_server.uri()));

        let res = client.generate_response("sys", &[]).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_groq_network_error() {
        // Use an invalid scheme to induce a network/request error
        let client = GroqClient::new(SecretString::new("sk-123".to_string()))
            .with_base_url("invalid-url".to_string());

        let res = client.generate_response("sys", &[]).await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("groq_fallback_required"));
    }
}
