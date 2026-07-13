use anyhow::{Context, Result};
use game_content::{ContentWarning, LoadedContent};
use game_core::{CoreSnapshot, ENERGY_ID, GameDefinition, GameEvent, GameSession, PricingMode};
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
    let loaded = game_content::load_directory_with_warnings(&content)
        .with_context(|| format!("failed to load content from {}", content.display()))?;
    if args.iter().any(|arg| arg == "--validate-content") {
        print_content_validation(&loaded);
        return Ok(());
    }

    let ticks = economy_diagnostic_ticks(&args)?;
    if args.iter().any(|arg| arg == "--compare-pricing-modes") {
        let ticks = ticks.unwrap_or(500);
        println!("Pricing A/B: identical content and initial state, ticks={ticks}");
        for mode in [PricingMode::Scarcity, PricingMode::CostAware] {
            let mut definition = loaded.definition.clone();
            apply_pricing_mode(&mut definition, mode);
            let mut session =
                GameSession::new(definition).context("failed to construct simulation")?;
            println!("\n=== pricing_mode={} ===", pricing_mode_label(mode));
            run_economy_diagnostics(&mut session, ticks)?;
        }
        return Ok(());
    }

    let mut definition = loaded.definition;
    if let Some(mode) = pricing_mode_argument(&args)? {
        apply_pricing_mode(&mut definition, mode);
    }
    let mut session = GameSession::new(definition).context("failed to construct simulation")?;
    if let Some(ticks) = ticks {
        run_economy_diagnostics(&mut session, ticks)?;
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--headless") {
        let initial_physical = physical_energy(&session.snapshot())?;
        for _ in 0..50 {
            session.step()?;
        }
        let snapshot = session.snapshot();
        let player = snapshot
            .traders
            .iter()
            .find(|trader| trader.player)
            .context("player missing")?;
        let bay_energy = player
            .cargo
            .iter()
            .find(|(good, _)| good.as_str() == ENERGY_ID)
            .map_or(0, |(_, quantity)| *quantity);
        println!(
            "Headless run complete: tick={}, systems={}, traders={}, player_tank={}/{}, player_bay_energy={}, player_cargo={}",
            snapshot.tick,
            snapshot.markets.len(),
            snapshot.traders.len(),
            player.energy_tank.0,
            player.energy_tank_capacity.0,
            bay_energy,
            player.cargo.values().sum::<u64>()
        );
        println!("{}", format_reconciliation(&snapshot, initial_physical));
        return Ok(());
    }
    game_tui::run(game_app::spawn(session)).await
}

fn print_content_validation(loaded: &LoadedContent) {
    println!(
        "Valid content: {} systems, {} goods, {} recipes, {} traders",
        loaded.definition.systems.len(),
        loaded.definition.goods.len(),
        loaded.definition.recipes.len(),
        loaded.definition.traders.len()
    );
    for warning in &loaded.warnings {
        match warning {
            ContentWarning::BootstrapRunwayAcknowledged {
                source,
                system,
                starting_energy,
                required_burn_per_tick,
                runway_ticks,
                required_ticks,
                exporter,
                trader,
            } => println!(
                "warning: {source}:{system}: bootstrap risk acknowledged; starting_energy={starting_energy} burn_per_tick={required_burn_per_tick} runway={runway_ticks} required={required_ticks} exporter={exporter} trader={trader}"
            ),
            ContentWarning::BootstrapDeliveryAcknowledged {
                source,
                system,
                detail,
            } => println!(
                "warning: {source}:{system}: bootstrap delivery risk acknowledged; {detail}"
            ),
        }
    }
}

fn content_path(args: &[String]) -> PathBuf {
    args.windows(2)
        .find(|pair| pair[0] == "--content")
        .map_or_else(|| PathBuf::from("content"), |pair| PathBuf::from(&pair[1]))
}

fn value_after<'a>(args: &'a [String], option: &str) -> Result<Option<&'a str>> {
    let Some(index) = args.iter().position(|arg| arg == option) else {
        return Ok(None);
    };
    let value = args
        .get(index + 1)
        .filter(|value| !value.starts_with("--"))
        .with_context(|| format!("{option} requires a value"))?;
    Ok(Some(value))
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

fn pricing_mode_argument(args: &[String]) -> Result<Option<PricingMode>> {
    value_after(args, "--pricing-mode")?
        .map(parse_pricing_mode)
        .transpose()
}

fn parse_pricing_mode(value: &str) -> Result<PricingMode> {
    match value {
        "scarcity" => Ok(PricingMode::Scarcity),
        "cost-aware" | "cost_aware" => Ok(PricingMode::CostAware),
        _ => anyhow::bail!("--pricing-mode must be 'scarcity' or 'cost-aware'"),
    }
}

fn pricing_mode_label(mode: PricingMode) -> &'static str {
    match mode {
        PricingMode::Scarcity => "scarcity",
        PricingMode::CostAware => "cost-aware",
    }
}

fn apply_pricing_mode(definition: &mut GameDefinition, mode: PricingMode) {
    for system in &mut definition.systems {
        system.policy.pricing_mode = mode;
    }
}

#[derive(Default)]
struct DiagnosticActivity {
    trades: u64,
    partial_sales: u64,
    reservations: u64,
    departures: u64,
    arrivals: u64,
    produced: u64,
    generated: i128,
    life_support_unsupplied: i128,
}

fn run_economy_diagnostics(session: &mut GameSession, ticks: u64) -> Result<()> {
    const REPORT_INTERVAL: u64 = 50;
    let initial = session.snapshot();
    let initial_physical = physical_energy(&initial)?;
    let mode = initial.markets.first().map_or("unknown", |market| {
        pricing_mode_label(market.policy.pricing_mode)
    });
    let requires_processor_solvency = initial
        .markets
        .iter()
        .any(|market| market.policy.pricing_mode == PricingMode::CostAware);
    let mut activity = DiagnosticActivity::default();
    println!("Economy diagnostics: ticks={ticks}, interval={REPORT_INTERVAL}, pricing_mode={mode}");
    for tick in 1..=ticks {
        session.step()?;
        for event in session.drain_events() {
            match event {
                GameEvent::Bought { .. } => activity.trades += 1,
                GameEvent::Sold { partial, .. } => {
                    activity.trades += 1;
                    activity.partial_sales += u64::from(partial);
                }
                GameEvent::ReservationCreated { .. } => activity.reservations += 1,
                GameEvent::Departed { .. } => activity.departures += 1,
                GameEvent::Arrived { .. } => activity.arrivals += 1,
                GameEvent::Produced { .. } => activity.produced += 1,
                GameEvent::EnergyGenerated { amount, .. } => {
                    activity.generated += i128::from(amount.0);
                }
                GameEvent::LifeSupport { unsupplied, .. } => {
                    activity.life_support_unsupplied += i128::from(unsupplied.0);
                }
                GameEvent::BrownoutTransition { .. }
                | GameEvent::PopulationChanged { .. }
                | GameEvent::TraderSpawned { .. }
                | GameEvent::TraderRetired { .. }
                | GameEvent::InvestmentCompleted { .. }
                | GameEvent::InvestmentDeferred { .. }
                | GameEvent::GovernorPolicyRejected { .. }
                | GameEvent::ReservationReleased { .. }
                | GameEvent::SaleDeferred { .. }
                | GameEvent::PolicyChanged { .. }
                | GameEvent::TickAdvanced(_)
                | GameEvent::Rejected(_) => {}
            }
        }
        if tick % REPORT_INTERVAL == 0 || tick == ticks {
            let snapshot = session.snapshot();
            let (mut traveling, mut idle, mut blocked_laden) = (0, 0, 0);
            for trader in snapshot.traders.iter().filter(|trader| !trader.player) {
                if trader.travel.is_some() {
                    traveling += 1;
                } else if trader.cargo.is_empty() {
                    idle += 1;
                } else {
                    blocked_laden += 1;
                }
            }
            println!(
                "tick={tick} trades={} partial_sales={} reservations={} departures={} arrivals={} produced={} generated={} life_support_unsupplied={} npc_traveling={traveling} npc_idle={idle} npc_stationary_laden={blocked_laden}",
                activity.trades,
                activity.partial_sales,
                activity.reservations,
                activity.departures,
                activity.arrivals,
                activity.produced,
                activity.generated,
                activity.life_support_unsupplied,
            );
            activity = DiagnosticActivity::default();
        }
    }

    let snapshot = session.snapshot();
    println!("{}", format_reconciliation(&snapshot, initial_physical));
    println!("markets:");
    for market in &snapshot.markets {
        let flow = market.energy_flow;
        let burned = i128::from(flow.life_support_burned.0)
            + i128::from(flow.source_burned.0)
            + i128::from(flow.production_burned.0)
            + i128::from(flow.travel_burned.0);
        let target_count = market.targets.len();
        let stocked_targets = market
            .targets
            .iter()
            .filter(|(good, target)| {
                market.inventory.get(*good).copied().unwrap_or(0) >= u64::from(**target)
            })
            .count();
        let funded_demand_units = market
            .demand
            .values()
            .map(|demand| u64::from(demand.funded))
            .sum::<u64>();
        let average_unit_cost = if market.cost_basis.is_empty() {
            0
        } else {
            market
                .cost_basis
                .values()
                .filter_map(|basis| basis.unit_cost_ceil().ok())
                .map(|cost| i128::from(cost.0))
                .sum::<i128>()
                / i128::try_from(market.cost_basis.len()).unwrap_or(1)
        };
        let realized_processor_cost = i128::from(market.ledger.processor_input_cost.0)
            + i128::from(market.ledger.processor_operating_energy.0);
        let realized_processor_revenue = i128::from(market.ledger.processor_output_revenue.0);
        let realized_processor_margin = realized_processor_revenue - realized_processor_cost;
        println!(
            "  {} energy={}/{} health={} reserved_claims={} operating_reserve={} protected_budget={} unreserved_purchasing={} funded_demand_units={} generated={} burned={} curtailed={} unsupplied_life_support={} avg_unit_cost={} policy_margin={}% realized_processor_input_cost={} realized_processor_operating_energy={} realized_processor_output_revenue={} realized_processor_margin={} targets_met={}/{} bootstrap_risk_acknowledged={} net_trade_energy={} paid={} received={} units_bought={} units_sold={} source_units={} recipe_inputs={} recipe_outputs={}",
            market.name,
            market.energy_stock.0,
            market.energy_storage_cap.0,
            market_health(market),
            market.reserved_energy.0,
            market.operating_reserve.0,
            market.protected_liquidation_budget.0,
            market.unreserved_energy_for_purchases.0,
            funded_demand_units,
            flow.generated.0,
            burned,
            flow.curtailed.0,
            flow.life_support_unsupplied.0,
            average_unit_cost,
            market.policy.producer_margin_percent,
            market.ledger.processor_input_cost.0,
            market.ledger.processor_operating_energy.0,
            realized_processor_revenue,
            realized_processor_margin,
            stocked_targets,
            target_count,
            market.bootstrap_risk_acknowledged,
            market.ledger.energy_received_from_traders.0 - market.ledger.energy_paid_to_traders.0,
            market.ledger.energy_paid_to_traders.0,
            market.ledger.energy_received_from_traders.0,
            market.ledger.units_bought_from_traders,
            market.ledger.units_sold_to_traders,
            market.ledger.source_units_generated,
            market.ledger.recipe_input_units_consumed,
            market.ledger.recipe_output_units_produced,
        );
    }
    let solvency = session.processor_solvency()?;
    let insolvent = solvency.iter().filter(|row| !row.solvent).count();
    println!(
        "processor_structural_solvency total={} insolvent={} status={}",
        solvency.len(),
        insolvent,
        if insolvent == 0 { "ok" } else { "INSOLVENT" }
    );
    for row in solvency.iter().filter(|row| !row.solvent) {
        println!(
            "  insolvent system={} recipe={} expected_input_bids={} operating_energy={} expected_output_asks={} required_margin={}%",
            row.system,
            row.recipe,
            row.expected_input_bids.0,
            row.operating_energy.0,
            row.expected_output_asks.0,
            row.required_margin_percent
        );
    }
    if requires_processor_solvency {
        anyhow::ensure!(insolvent == 0, "processor structural insolvency detected");
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
        let bay_energy = trader
            .cargo
            .iter()
            .find(|(good, _)| good.as_str() == ENERGY_ID)
            .map_or(0, |(_, quantity)| *quantity);
        println!(
            "  {} state={state} system={} tank={}/{} bay_energy={} cargo_units={}/{} reservation={:?} transactions={} net_trade_energy={}",
            trader.name,
            trader.system,
            trader.energy_tank.0,
            trader.energy_tank_capacity.0,
            bay_energy,
            trader.cargo.values().sum::<u64>(),
            trader.cargo_capacity,
            trader.reservation,
            trader.ledger.completed_transactions,
            i128::from(trader.ledger.sales_revenue.0) - i128::from(trader.ledger.purchase_cost.0),
        );
    }
    Ok(())
}

fn market_health(market: &game_core::MarketSnapshot) -> &'static str {
    if market.energy_flow.life_support_unsupplied.0 > 0 || market.energy_stock.0 == 0 {
        "deficit"
    } else if market.unreserved_energy_for_purchases.0 == 0 {
        "low"
    } else {
        "healthy"
    }
}

fn physical_energy(snapshot: &CoreSnapshot) -> Result<i128> {
    let markets = snapshot
        .markets
        .iter()
        .map(|market| i128::from(market.energy_stock.0))
        .sum::<i128>();
    let tanks = snapshot
        .traders
        .iter()
        .map(|trader| i128::from(trader.energy_tank.0))
        .sum::<i128>();
    let mut cargo = 0_i128;
    for trader in &snapshot.traders {
        if let Some((_, quantity)) = trader
            .cargo
            .iter()
            .find(|(good, _)| good.as_str() == ENERGY_ID)
        {
            cargo = cargo
                .checked_add(i128::from(*quantity))
                .context("energy cargo total overflow")?;
        }
    }
    markets
        .checked_add(tanks)
        .and_then(|value| value.checked_add(cargo))
        .context("physical energy total overflow")
}

fn format_reconciliation(snapshot: &CoreSnapshot, initial_physical: i128) -> String {
    let flow = snapshot.energy_flow;
    let generated = i128::from(flow.generated.0);
    let burned = i128::from(flow.life_support_burned.0)
        + i128::from(flow.source_burned.0)
        + i128::from(flow.production_burned.0)
        + i128::from(flow.travel_burned.0);
    let curtailed = i128::from(flow.curtailed.0);
    let expected = initial_physical + generated - burned - curtailed;
    let actual = physical_energy(snapshot).unwrap_or(i128::MAX);
    let difference = actual - expected;
    format!(
        "energy_reconciliation initial={initial_physical} generated={generated} burned={burned} curtailed={curtailed} expected={expected} actual={actual} difference={difference} status={} physical_transfers market_to_tank={} tank_to_market={} market_to_energy_cargo={} energy_cargo_to_market={}",
        if difference == 0 { "ok" } else { "MISMATCH" },
        i128::from(flow.market_to_tank.0),
        i128::from(flow.tank_to_market.0),
        i128::from(flow.market_to_energy_cargo.0),
        i128::from(flow.energy_cargo_to_market.0),
    )
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

    #[test]
    fn pricing_mode_argument_accepts_documented_spellings() {
        assert_eq!(pricing_mode_argument(&[]).unwrap(), None);
        assert_eq!(
            pricing_mode_argument(&["--pricing-mode".into(), "scarcity".into()]).unwrap(),
            Some(PricingMode::Scarcity)
        );
        assert_eq!(
            pricing_mode_argument(&["--pricing-mode".into(), "cost-aware".into()]).unwrap(),
            Some(PricingMode::CostAware)
        );
        assert!(pricing_mode_argument(&["--pricing-mode".into(), "money".into()]).is_err());
        assert!(pricing_mode_argument(&["--pricing-mode".into()]).is_err());
    }

    #[test]
    fn reconciliation_formatter_reports_exact_flow() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let definition = game_content::load_directory(root).unwrap();
        let mut session = GameSession::new(definition).unwrap();
        let initial = physical_energy(&session.snapshot()).unwrap();
        session.step().unwrap();
        let output = format_reconciliation(&session.snapshot(), initial);
        assert!(output.contains("energy_reconciliation"));
        assert!(output.contains("generated="));
        assert!(output.contains("burned="));
        assert!(output.contains("curtailed="));
        assert!(output.contains("difference=0 status=ok"), "{output}");
    }
}
