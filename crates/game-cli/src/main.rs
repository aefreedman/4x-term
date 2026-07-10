use anyhow::{Context, Result};
use game_core::GameSession;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open("4x-term.log")
        .context("failed to open 4x-term.log")?;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_ansi(false)
        .with_writer(Mutex::new(log))
        .try_init()
        .ok();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let content = content_path(&args);
    let definition = game_content::load_directory(&content)
        .with_context(|| format!("failed to load content from {}", content.display()))?;
    if args.iter().any(|arg| arg == "--validate-content") {
        println!(
            "Valid content: {} systems, {} goods, {} recipes, {} traders",
            definition.systems.len(),
            definition.goods.len(),
            definition.recipes.len(),
            definition.traders.len()
        );
        return Ok(());
    }
    let mut session = GameSession::new(definition).context("failed to construct simulation")?;
    if args.iter().any(|arg| arg == "--headless") {
        for _ in 0..50 {
            session.step()?;
        }
        let snapshot = session.snapshot();
        let player = snapshot
            .traders
            .iter()
            .find(|trader| trader.player)
            .context("player missing")?;
        println!(
            "Headless run complete: tick={}, systems={}, traders={}, player_funds=¤{}, player_cargo={}",
            snapshot.tick,
            snapshot.markets.len(),
            snapshot.traders.len(),
            player.currency.0,
            player.cargo.values().sum::<u32>()
        );
        return Ok(());
    }
    game_tui::run(game_app::spawn(session)).await
}

fn content_path(args: &[String]) -> PathBuf {
    args.windows(2)
        .find(|pair| pair[0] == "--content")
        .map_or_else(|| PathBuf::from("content"), |pair| PathBuf::from(&pair[1]))
}
