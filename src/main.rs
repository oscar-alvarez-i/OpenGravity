#![allow(unexpected_cfgs)]
use open_gravity::bot::telegram::{run_bot, BotDependencies};
use open_gravity::config::env::load_config;
use open_gravity::db::sqlite::Db;
use open_gravity::llm::{groq::GroqClient, openrouter::OpenRouterClient, LlmOrchestrator};
use open_gravity::security::whitelist::Whitelist;
use open_gravity::tools::registry::Registry;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[cfg(not(tarpaulin_include))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Init tracing
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("open_gravity=info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("Starting OpenGravity...");

    // 2. Load Config
    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            error!("Configuration error: {:?}", e);
            std::process::exit(1);
        }
    };
    info!("Configuration loaded sequentially");

    // 3. Open SQLite DB
    let db = match Db::new(&config.db_path) {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to initialize SQLite: {:?}", e);
            std::process::exit(1);
        }
    };
    let db = Arc::new(db);
    info!("Database initialized");

    // 4. Setup Whitelist
    let whitelist = Arc::new(Whitelist::new(config.telegram_allowed_user_ids));

    // 5. Setup LLMs Orchestrator
    let groq = Box::new(GroqClient::new(config.groq_api_key));
    let openrouter = Box::new(OpenRouterClient::new(
        config.openrouter_api_key,
        config.openrouter_model,
    ));
    let llm_orchestrator = Arc::new(LlmOrchestrator::new(groq, openrouter));

    // 6. Setup Tools Registry
    let registry = Arc::new(Registry::new());

    // 7. Setup & Run Bot
    let dependencies = BotDependencies {
        db,
        llm: llm_orchestrator,
        registry,
        whitelist,
    };

    let token = config.telegram_bot_token.expose_secret().to_string();
    run_bot(token, dependencies).await;

    Ok(())
}
