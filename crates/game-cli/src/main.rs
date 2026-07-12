use anyhow::{Context, Result};
use game_core::{GameEvent, GameSession};
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
    if let Some(ticks) = economy_diagnostic_ticks(&args)? {
        run_economy_diagnostics(&mut session, ticks)?;
        return Ok(());
    }
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

fn economy_diagnostic_ticks(args: &[String]) -> Result<Option<u64>> {
    let Some(index) = args.iter().position(|arg| arg == "--economy-diagnostics") else {
        return Ok(None);
    };
    let ticks = args
        .get(index + 1)
        .filter(|value| !value.starts_with("--"))
        .map_or(Ok(500), |value| {
            value
                .parse::<u64>()
                .context("--economy-diagnostics ticks must be a positive integer")
        })?;
    anyhow::ensure!(ticks > 0, "--economy-diagnostics ticks must be positive");
    Ok(Some(ticks))
}

#[derive(Default)]
struct DiagnosticActivity {
    trades: u64,
    departures: u64,
    arrivals: u64,
    produced: u64,
    consumed: u64,
}

fn run_economy_diagnostics(session: &mut GameSession, ticks: u64) -> Result<()> {
    const REPORT_INTERVAL: u64 = 50;
    let mut activity = DiagnosticActivity::default();
    println!("Economy diagnostics: ticks={ticks}, interval={REPORT_INTERVAL}");
    for tick in 1..=ticks {
        session.step()?;
        for event in session.drain_events() {
            match event {
                GameEvent::Bought { .. } | GameEvent::Sold { .. } => activity.trades += 1,
                GameEvent::Departed { .. } => activity.departures += 1,
                GameEvent::Arrived { .. } => activity.arrivals += 1,
                GameEvent::Produced { .. } => activity.produced += 1,
                GameEvent::Consumed { .. } => activity.consumed += 1,
                GameEvent::TickAdvanced(_) | GameEvent::Rejected(_) => {}
            }
        }
        if tick % REPORT_INTERVAL == 0 || tick == ticks {
            let snapshot = session.snapshot();
            let npcs = snapshot.traders.iter().filter(|trader| !trader.player);
            let (mut traveling, mut idle, mut blocked_laden) = (0, 0, 0);
            for trader in npcs {
                if trader.travel.is_some() {
                    traveling += 1;
                } else if trader.cargo.is_empty() {
                    idle += 1;
                } else {
                    blocked_laden += 1;
                }
            }
            println!(
                "tick={tick} trades={} departures={} arrivals={} produced={} consumed={} npc_traveling={traveling} npc_idle={idle} npc_stationary_laden={blocked_laden}",
                activity.trades,
                activity.departures,
                activity.arrivals,
                activity.produced,
                activity.consumed,
            );
            activity = DiagnosticActivity::default();
        }
    }

    let snapshot = session.snapshot();
    let market_currency = snapshot
        .markets
        .iter()
        .map(|market| i128::from(market.currency.0))
        .sum::<i128>();
    let trader_currency = snapshot
        .traders
        .iter()
        .map(|trader| i128::from(trader.currency.0))
        .sum::<i128>();
    println!(
        "currency market=¤{market_currency} trader=¤{trader_currency} total=¤{}",
        market_currency + trader_currency
    );
    println!("markets:");
    for market in &snapshot.markets {
        let ledger = market.ledger;
        println!(
            "  {} balance=¤{} net_trade=¤{} paid=¤{} received=¤{} units_bought={} units_sold={} source_units={} recipe_inputs={} recipe_outputs={} tertiary_inputs={}",
            market.name,
            market.currency.0,
            ledger.net_trade_cash_flow(),
            ledger.currency_paid_to_traders,
            ledger.currency_received_from_traders,
            ledger.units_bought_from_traders,
            ledger.units_sold_to_traders,
            ledger.source_units_generated,
            ledger.recipe_input_units_consumed,
            ledger.recipe_output_units_produced,
            ledger.tertiary_input_units_consumed,
        );
    }
    println!("npc traders:");
    for trader in snapshot.traders.iter().filter(|trader| !trader.player) {
        let state = if trader.travel.is_some() {
            "traveling"
        } else if trader.cargo.is_empty() {
            "idle"
        } else {
            "stationary-laden"
        };
        println!(
            "  {} state={state} system={} balance=¤{} cargo_units={} transactions={} net_trade=¤{}",
            trader.name,
            trader.system,
            trader.currency.0,
            trader.cargo.values().sum::<u32>(),
            trader.ledger.completed_transactions,
            i128::from(trader.ledger.sales_revenue) - i128::from(trader.ledger.purchase_cost),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn economy_diagnostic_tick_argument_is_optional_and_validated() {
        assert_eq!(economy_diagnostic_ticks(&[]).unwrap(), None);
        assert_eq!(
            economy_diagnostic_ticks(&["--economy-diagnostics".into()]).unwrap(),
            Some(500)
        );
        assert_eq!(
            economy_diagnostic_ticks(&["--economy-diagnostics".into(), "350".into()]).unwrap(),
            Some(350)
        );
        assert!(economy_diagnostic_ticks(&["--economy-diagnostics".into(), "0".into()]).is_err());
    }
}
