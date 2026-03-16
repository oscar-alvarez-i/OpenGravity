#![allow(unexpected_cfgs)]
use crate::agent::executor::Executor;
use crate::agent::memory_bridge::MemoryBridge;
use crate::agent::planner::Planner;
use crate::agent::r#loop::AgentLoop;
use crate::db::sqlite::Db;
use crate::domain::message::{Message as DomainMessage, Role};
use crate::llm::LlmOrchestrator;
use crate::security::whitelist::Whitelist;
use crate::tools::registry::Registry;

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use teloxide::prelude::*;
use tracing::{error, info, warn};

#[async_trait]
pub trait BotClient: Send + Sync {
    async fn send_message(&self, chat_id: ChatId, text: String) -> Result<()>;
}

pub struct TelegramBot {
    bot: Bot,
}

impl TelegramBot {
    pub fn new(bot: Bot) -> Self {
        Self { bot }
    }
}

#[cfg(not(tarpaulin_include))]
#[async_trait]
impl BotClient for TelegramBot {
    async fn send_message(&self, chat_id: ChatId, text: String) -> Result<()> {
        self.bot
            .send_message(chat_id, text)
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!(e))
    }
}

pub struct BotDependencies {
    pub db: Arc<Db>,
    pub llm: Arc<LlmOrchestrator>,
    pub registry: Arc<Registry>,
    pub whitelist: Arc<Whitelist>,
}

#[cfg(not(tarpaulin_include))]
pub async fn run_bot(token: String, deps: BotDependencies) {
    let bot = Bot::new(token);
    let client = Arc::new(TelegramBot::new(bot.clone()));
    let deps = Arc::new(deps);

    info!("Starting Telegram long polling...");

    teloxide::repl(bot, move |bot_unused: Bot, msg: Message| {
        let deps = Arc::clone(&deps);
        let client = Arc::clone(&client);

        async move {
            let _ = bot_unused; // We use our client instead
            handle_message(client, deps, msg).await
        }
    })
    .await;
}

pub async fn handle_message(
    bot: Arc<dyn BotClient>,
    deps: Arc<BotDependencies>,
    msg: Message,
) -> Result<(), teloxide::RequestError> {
    let Some(user) = &msg.from else {
        warn!("Ignoring message without from context");
        return Ok(());
    };

    let user_id = user.id.0;

    if !deps.whitelist.is_allowed(user_id) {
        warn!("Unauthorized access attempt from user_id: {}", user_id);
        let _ = bot
            .send_message(msg.chat.id, "Unauthorized. Go away.".to_string())
            .await;
        return Ok(());
    }

    let Some(text) = msg.text() else {
        warn!("Ignoring non-text message");
        return Ok(());
    };

    // Observation 3: Intercept /start before agent loop and persistence
    if text == "/start" {
        info!("Received /start command from user {}", user_id);
        let _ = bot
            .send_message(
                msg.chat.id,
                "Welcome to OpenGravity! I am your AI assistant, functioning within the OpenGravity architecture via Groq.".to_string(),
            )
            .await;
        return Ok(());
    }

    info!("Received message from authorized user {}", user_id);

    let incoming_msg = DomainMessage::new(Role::User, text);

    let memory = MemoryBridge::new(&deps.db, &user_id.to_string());
    let planner = Planner::new();
    let executor = Executor::new(&deps.llm, &deps.registry);

    let agent_loop = AgentLoop::new(memory, planner, executor);

    match agent_loop.run(incoming_msg).await {
        Ok(response) => {
            if let Err(e) = bot.send_message(msg.chat.id, response.content).await {
                error!("Failed to send Telegram response: {}", e);
            }
        }
        Err(e) => {
            error!("Agent loop error: {}", e);
            let _ = bot
                .send_message(msg.chat.id, "Internal agent error.".to_string())
                .await;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::models::MockLlmProvider;
    use mockall::mock;
    use teloxide::types::{ChatId, Message};

    mock! {
        pub BotClient {}
        #[async_trait]
        impl BotClient for BotClient {
            async fn send_message(&self, chat_id: ChatId, text: String) -> Result<()>;
        }
    }

    fn create_test_message(text: Option<&str>, user_id: u64) -> Message {
        let text_json = match text {
            Some(t) => format!("\"text\": \"{}\", \"media_kind\": {{\"Text\": {{\"text\": \"{}\", \"entities\": []}}}}", t, t),
            None => "\"media_kind\": {\"Animation\": {\"animation\": {\"file_id\": \"123\", \"file_unique_id\": \"123\", \"width\": 100, \"height\": 100, \"duration\": 10, \"file_size\": 100}, \"has_media_spoiler\": false}}".to_string(),
        };

        let json = format!(
            r#"{{
            "message_id": 1,
            "date": 1600000000,
            "chat": {{
                "id": 123,
                "type": "private",
                "first_name": "Test"
            }},
            "from": {{
                "id": {},
                "is_bot": false,
                "first_name": "Test"
            }},
            {1}
        }}"#,
            user_id, text_json
        );

        serde_json::from_str(&json).expect("Failed to parse test message JSON")
    }

    #[tokio::test]
    async fn test_handle_message_unauthorized() {
        let mut bot_mock = MockBotClient::new();
        bot_mock
            .expect_send_message()
            .times(1)
            .withf(|_, text| text == "Unauthorized. Go away.")
            .returning(|_, _| Ok(()));

        let db = Arc::new(Db::new(":memory:").unwrap());
        let groq = Box::new(MockLlmProvider::new());
        let or = Box::new(MockLlmProvider::new());
        let llm = Arc::new(LlmOrchestrator::new(groq, or));
        let registry = Arc::new(Registry::new());
        let whitelist = Arc::new(Whitelist::new(vec![1])); // Only user 1 allowed

        let deps = Arc::new(BotDependencies {
            db,
            llm,
            registry,
            whitelist,
        });

        let msg = create_test_message(Some("hello"), 2); // User 2 is unauthorized
        let res = handle_message(Arc::new(bot_mock), deps, msg).await;

        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_handle_message_non_text() {
        let bot_mock = MockBotClient::new(); // No messages expected

        let db = Arc::new(Db::new(":memory:").unwrap());
        let groq = Box::new(MockLlmProvider::new());
        let or = Box::new(MockLlmProvider::new());
        let llm = Arc::new(LlmOrchestrator::new(groq, or));
        let registry = Arc::new(Registry::new());
        let whitelist = Arc::new(Whitelist::new(vec![1]));

        let deps = Arc::new(BotDependencies {
            db,
            llm,
            registry,
            whitelist,
        });

        let msg = create_test_message(None, 1); // User 1, but no text
        let res = handle_message(Arc::new(bot_mock), deps, msg).await;

        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_handle_message_send_failure() {
        let mut bot_mock = MockBotClient::new();
        bot_mock
            .expect_send_message()
            .times(1)
            .returning(|_, _| Err(anyhow::anyhow!("Telegram API Error")));

        let db = Arc::new(Db::new(":memory:").unwrap());
        let mut groq = MockLlmProvider::new();
        groq.expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("Answer".to_string()) }));

        let or = MockLlmProvider::new();
        let llm = Arc::new(LlmOrchestrator::new(Box::new(groq), Box::new(or)));
        let registry = Arc::new(Registry::new());
        let whitelist = Arc::new(Whitelist::new(vec![1]));

        let deps = Arc::new(BotDependencies {
            db,
            llm,
            registry,
            whitelist,
        });

        let msg = create_test_message(Some("test"), 1);
        let res = handle_message(Arc::new(bot_mock), deps, msg).await;

        assert!(res.is_ok()); // The handler swallows the error after logging
    }

    #[tokio::test]
    async fn test_handle_message_no_from() {
        let bot_mock = MockBotClient::new();
        let db = Arc::new(Db::new(":memory:").unwrap());
        let groq = Box::new(MockLlmProvider::new());
        let or = Box::new(MockLlmProvider::new());
        let llm = Arc::new(LlmOrchestrator::new(groq, or));
        let registry = Arc::new(Registry::new());
        let whitelist = Arc::new(Whitelist::new(vec![1]));

        let deps = Arc::new(BotDependencies {
            db,
            llm,
            registry,
            whitelist,
        });

        // Create JSON message without "from"
        let json = r#"{
            "message_id": 1,
            "date": 1600000000,
            "chat": {
                "id": 123,
                "type": "private",
                "first_name": "Test"
            },
            "text": "hello",
            "media_kind": {"Text": {"text": "hello", "entities": []}}
        }"#;
        let msg: Message = serde_json::from_str(json).unwrap();

        let res = handle_message(Arc::new(bot_mock), deps, msg).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_telegram_bot_client() {
        let bot = Bot::new("token");
        let client = TelegramBot::new(bot);
        let _chat_id = ChatId(123);

        // We can't easily test the actual network call without wiremocking teloxide's inner client,
        // but we can at least invoke the struct to cover its methods.
        // Since it's a thin wrapper, this is mostly for coverage of the Delegation Pattern.
        // To avoid actual network calls in tests, we'd need to mock the Bot's inner client.
        // For now, let's just cover the constructor and a simple check.
        let bot_instance = client.bot.clone();
        assert_eq!(bot_instance.token(), "token");
    }

    #[tokio::test]
    async fn test_handle_message_success() {
        let mut bot_mock = MockBotClient::new();
        bot_mock
            .expect_send_message()
            .times(1)
            .withf(|_, text| text == "I am an AI assistant.")
            .returning(|_, _| Ok(()));

        let db = Arc::new(Db::new(":memory:").unwrap());
        let mut groq = MockLlmProvider::new();
        groq.expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("I am an AI assistant.".to_string()) }));

        let or = MockLlmProvider::new();
        let llm = Arc::new(LlmOrchestrator::new(Box::new(groq), Box::new(or)));
        let registry = Arc::new(Registry::new());
        let whitelist = Arc::new(Whitelist::new(vec![1]));

        let deps = Arc::new(BotDependencies {
            db,
            llm,
            registry,
            whitelist,
        });

        let msg = create_test_message(Some("who are you?"), 1);
        let res = handle_message(Arc::new(bot_mock), deps, msg).await;

        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_handle_message_agent_loop_error() {
        let mut bot_mock = MockBotClient::new();
        bot_mock
            .expect_send_message()
            .times(1)
            .withf(|_, text| text == "Internal agent error.")
            .returning(|_, _| Ok(()));

        let db = Arc::new(Db::new(":memory:").unwrap());
        let mut groq = MockLlmProvider::new();
        groq.expect_generate_response()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(anyhow::anyhow!("LLM down")) }));

        let or = MockLlmProvider::new();
        let llm = Arc::new(LlmOrchestrator::new(Box::new(groq), Box::new(or)));
        let registry = Arc::new(Registry::new());
        let whitelist = Arc::new(Whitelist::new(vec![1]));

        let deps = Arc::new(BotDependencies {
            db,
            llm,
            registry,
            whitelist,
        });

        let msg = create_test_message(Some("help"), 1);
        let res = handle_message(Arc::new(bot_mock), deps, msg).await;

        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_handle_message_tool_invocation_boundary() {
        let mut bot_mock = MockBotClient::new();
        // The bot should eventually send the final answer after tool reasoning
        bot_mock
            .expect_send_message()
            .times(1)
            .withf(|_, text| text.contains("The current time is"))
            .returning(|_, _| Ok(()));

        let db = Arc::new(Db::new(":memory:").unwrap());

        let mut groq = MockLlmProvider::new();
        let mut seq = mockall::Sequence::new();

        // Step 1: LLM decides to use a tool
        groq.expect_generate_response()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_, _| Box::pin(async { Ok("TOOL:get_current_time".to_string()) }));

        // Step 2: LLM formulates final answer after tool output (which executor appends)
        groq.expect_generate_response()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_, _| {
                Box::pin(async { Ok("The current time is 2024-03-15T20:00:00Z".to_string()) })
            });

        let or = MockLlmProvider::new();
        let llm = Arc::new(LlmOrchestrator::new(Box::new(groq), Box::new(or)));
        let registry = Arc::new(Registry::new());
        let whitelist = Arc::new(Whitelist::new(vec![1]));

        let deps = Arc::new(BotDependencies {
            db,
            llm,
            registry,
            whitelist,
        });

        let msg = create_test_message(Some("what time is it?"), 1);
        let res = handle_message(Arc::new(bot_mock), deps, msg).await;

        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_handle_message_start_interception() {
        let mut bot_mock = MockBotClient::new();
        // Bot should send welcome message directly
        bot_mock
            .expect_send_message()
            .times(1)
            .withf(|_, text| text.contains("Welcome to OpenGravity"))
            .returning(|_, _| Ok(()));

        let db = Arc::new(Db::new(":memory:").unwrap());
        // No LLM calls expected
        let groq = Box::new(MockLlmProvider::new());
        let or = Box::new(MockLlmProvider::new());
        let llm = Arc::new(LlmOrchestrator::new(groq, or));
        let registry = Arc::new(Registry::new());
        let whitelist = Arc::new(Whitelist::new(vec![1]));

        let deps = Arc::new(BotDependencies {
            db: Arc::clone(&db),
            llm,
            registry,
            whitelist,
        });

        let msg = create_test_message(Some("/start"), 1);
        let res = handle_message(Arc::new(bot_mock), deps, msg).await;

        assert!(res.is_ok());

        // Verify NO messages were persisted in DB for user 1
        let memories = db.fetch_latest_memories("1", 10).unwrap();
        assert_eq!(memories.len(), 0, "/start should not be persisted");
    }
}
