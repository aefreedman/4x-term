use anyhow::{Context, Result};
use game_content::{ContentWarning, LoadedContent};
use game_core::{
    BrownoutStage, ContentId, CoreSnapshot, ENERGY_ID, GameDefinition, GameEvent, GameSession,
    PricingMode,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
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
    let execution_mode = execution_mode_argument(&args)?;
    let content = content_path(&args);
    let loaded = game_content::load_directory_with_warnings(&content)
        .with_context(|| format!("failed to load content from {}", content.display()))?;

    match execution_mode {
        ExecutionMode::ValidateContent => print_content_validation(&loaded),
        ExecutionMode::PlayerImpact => {
            let config = player_impact_argument(&args)?;
            let report = run_player_impact(&loaded.definition, &config)?;
            print_player_impact_report(&report);
        }
        ExecutionMode::ComparePricingModes => {
            let ticks = economy_diagnostic_ticks(&args)?.unwrap_or(500);
            println!("Pricing A/B: identical content and initial state, ticks={ticks}");
            for mode in [PricingMode::Scarcity, PricingMode::CostAware] {
                let mut definition = loaded.definition.clone();
                apply_pricing_mode(&mut definition, mode);
                let mut session =
                    GameSession::new(definition).context("failed to construct simulation")?;
                println!("\n=== pricing_mode={} ===", pricing_mode_label(mode));
                let _ = run_economy_diagnostics(&mut session, ticks)?;
            }
        }
        ExecutionMode::EconomyDiagnostics => {
            let ticks = economy_diagnostic_ticks(&args)?.expect("mode guarantees ticks");
            let mut definition = loaded.definition;
            if let Some(mode) = pricing_mode_argument(&args)? {
                apply_pricing_mode(&mut definition, mode);
            }
            let mut session =
                GameSession::new(definition).context("failed to construct simulation")?;
            let baseline = run_economy_diagnostics(&mut session, ticks)?;
            if ticks >= 10_000 {
                validate_metastability(&baseline, "baseline")?;
                println!(
                    "metastability_acceptance ticks={} {} status=ok",
                    ticks,
                    format_soak_summary(&baseline),
                );
            }
        }
        ExecutionMode::Headless => {
            let mut definition = loaded.definition;
            if let Some(mode) = pricing_mode_argument(&args)? {
                apply_pricing_mode(&mut definition, mode);
            }
            let mut session =
                GameSession::new(definition).context("failed to construct simulation")?;
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
            let reconciliation = reconcile_energy(&snapshot, initial_physical)?;
            println!("{}", format_reconciliation(&reconciliation));
        }
        ExecutionMode::Tui => {
            let encyclopedia = encyclopedia_view(loaded.encyclopedia);
            let mut definition = loaded.definition;
            if let Some(mode) = pricing_mode_argument(&args)? {
                apply_pricing_mode(&mut definition, mode);
            }
            let session = GameSession::new(definition).context("failed to construct simulation")?;
            game_tui::run(game_app::spawn_with_encyclopedia(session, encyclopedia)).await?;
        }
    }
    Ok(())
}

fn encyclopedia_view(
    sections: Vec<game_content::EncyclopediaSection>,
) -> game_app::EncyclopediaView {
    game_app::EncyclopediaView {
        sections: sections
            .into_iter()
            .map(|section| game_app::EncyclopediaSectionView {
                title: section.title,
                articles: section
                    .articles
                    .into_iter()
                    .map(|article| game_app::EncyclopediaArticleView {
                        title: article.title,
                        paragraphs: article.paragraphs,
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecutionMode {
    ValidateContent,
    PlayerImpact,
    ComparePricingModes,
    EconomyDiagnostics,
    Headless,
    Tui,
}

fn execution_mode_argument(args: &[String]) -> Result<ExecutionMode> {
    const MODES: [(&str, ExecutionMode); 5] = [
        ("--validate-content", ExecutionMode::ValidateContent),
        ("--player-impact", ExecutionMode::PlayerImpact),
        (
            "--compare-pricing-modes",
            ExecutionMode::ComparePricingModes,
        ),
        ("--economy-diagnostics", ExecutionMode::EconomyDiagnostics),
        ("--headless", ExecutionMode::Headless),
    ];
    let selected = MODES
        .into_iter()
        .filter(|(option, _)| args.iter().any(|arg| arg == option))
        .collect::<Vec<_>>();
    anyhow::ensure!(
        selected.len() <= 1,
        "conflicting execution modes: {}",
        selected
            .iter()
            .map(|(option, _)| *option)
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(selected
        .first()
        .map_or(ExecutionMode::Tui, |(_, mode)| *mode))
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct PlayerImpactConfig {
    target: ContentId,
    delivery_tick: u64,
    good: ContentId,
    quantity: u64,
    horizon: u64,
}

fn required_value_after<'a>(args: &'a [String], option: &str) -> Result<&'a str> {
    value_after(args, option)?.with_context(|| format!("{option} is required with --player-impact"))
}

fn player_impact_argument(args: &[String]) -> Result<PlayerImpactConfig> {
    let target = ContentId::new(required_value_after(args, "--impact-target")?)
        .context("--impact-target must be a namespace-qualified content ID")?;
    let delivery_tick = required_value_after(args, "--impact-tick")?
        .parse::<u64>()
        .context("--impact-tick must be a non-negative integer")?;
    let good = ContentId::new(required_value_after(args, "--impact-good")?)
        .context("--impact-good must be a namespace-qualified content ID")?;
    let quantity = required_value_after(args, "--impact-quantity")?
        .parse::<u64>()
        .context("--impact-quantity must be a positive integer")?;
    let horizon = required_value_after(args, "--impact-horizon")?
        .parse::<u64>()
        .context("--impact-horizon must be a positive integer")?;
    anyhow::ensure!(quantity > 0, "--impact-quantity must be positive");
    anyhow::ensure!(horizon > 0, "--impact-horizon must be positive");
    anyhow::ensure!(
        delivery_tick < horizon,
        "--impact-tick must be before --impact-horizon"
    );
    Ok(PlayerImpactConfig {
        target,
        delivery_tick,
        good,
        quantity,
        horizon,
    })
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImpactDelivery {
    system: ContentId,
    good: ContentId,
    quantity: u64,
    energy_inflow: i64,
    tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImpactDivergence {
    tick: u64,
    target: ContentId,
    baseline_stage: BrownoutStage,
    intervention_stage: BrownoutStage,
    baseline_population: u64,
    intervention_population: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PlayerImpactReport {
    initial_snapshots_identical: bool,
    pre_delivery_snapshots_identical: bool,
    horizon: u64,
    delivery: ImpactDelivery,
    divergence: ImpactDivergence,
    baseline_reconciliation: ReconciliationReport,
    intervention_reconciliation: ReconciliationReport,
}

fn format_impact_divergence(divergence: &ImpactDivergence) -> String {
    format!(
        "player_impact first_divergence_tick={} target={} baseline_stage={} intervention_stage={} baseline_population={} intervention_population={}",
        divergence.tick,
        divergence.target,
        divergence.baseline_stage.label(),
        divergence.intervention_stage.label(),
        divergence.baseline_population,
        divergence.intervention_population,
    )
}

fn delivery_from_event(event: &GameEvent) -> Option<ImpactDelivery> {
    let GameEvent::ExternalDeliveryRecorded {
        system,
        good,
        quantity,
        energy_inflow,
        tick,
    } = event
    else {
        return None;
    };
    Some(ImpactDelivery {
        system: system.clone(),
        good: good.clone(),
        quantity: *quantity,
        energy_inflow: energy_inflow.0,
        tick: *tick,
    })
}

fn run_player_impact(
    definition: &GameDefinition,
    config: &PlayerImpactConfig,
) -> Result<PlayerImpactReport> {
    let mut baseline =
        GameSession::new(definition.clone()).context("failed to construct baseline simulation")?;
    let mut intervention = GameSession::new(definition.clone())
        .context("failed to construct intervention simulation")?;
    let baseline_initial = baseline.snapshot();
    let intervention_initial = intervention.snapshot();
    anyhow::ensure!(
        baseline_initial == intervention_initial,
        "identical definitions did not produce identical initial snapshots"
    );
    let baseline_initial_physical = physical_energy(&baseline_initial)?;
    let intervention_initial_physical = physical_energy(&intervention_initial)?;

    let mut deliveries = Vec::new();
    let mut first_divergence = None;
    for tick in 0..config.horizon {
        if tick < config.delivery_tick {
            anyhow::ensure!(
                baseline.snapshot() == intervention.snapshot(),
                "baseline and intervention diverged before delivery at tick {tick}"
            );
        }
        if tick == config.delivery_tick {
            anyhow::ensure!(
                baseline.snapshot() == intervention.snapshot(),
                "baseline and intervention diverged before delivery at tick {tick}"
            );
            intervention
                .submit(game_core::GameCommand::RecordExternalDelivery {
                    system: config.target.clone(),
                    good: config.good.clone(),
                    quantity: config.quantity,
                })
                .with_context(|| {
                    format!(
                        "external delivery rejected at tick {} for {}",
                        config.delivery_tick, config.target
                    )
                })?;
        }
        baseline.step()?;
        intervention.step()?;
        let baseline_events = baseline.drain_events();
        let intervention_events = intervention.drain_events();
        if tick < config.delivery_tick {
            anyhow::ensure!(
                baseline_events == intervention_events
                    && baseline.snapshot() == intervention.snapshot(),
                "baseline and intervention diverged before delivery after tick {}",
                tick + 1
            );
        }
        anyhow::ensure!(
            !baseline_events
                .iter()
                .any(|event| delivery_from_event(event).is_some()),
            "baseline unexpectedly recorded an external delivery"
        );
        deliveries.extend(intervention_events.iter().filter_map(delivery_from_event));

        if first_divergence.is_none() && tick >= config.delivery_tick {
            let baseline_snapshot = baseline.snapshot();
            let intervention_snapshot = intervention.snapshot();
            let baseline_target = baseline_snapshot
                .markets
                .iter()
                .find(|market| market.system_id == config.target)
                .context("impact target missing from baseline")?;
            let intervention_target = intervention_snapshot
                .markets
                .iter()
                .find(|market| market.system_id == config.target)
                .context("impact target missing from intervention")?;
            if baseline_target.brownout.stage != intervention_target.brownout.stage
                || baseline_target.population != intervention_target.population
            {
                first_divergence = Some(ImpactDivergence {
                    tick: tick + 1,
                    target: config.target.clone(),
                    baseline_stage: baseline_target.brownout.stage,
                    intervention_stage: intervention_target.brownout.stage,
                    baseline_population: baseline_target.population,
                    intervention_population: intervention_target.population,
                });
            }
        }
    }
    anyhow::ensure!(
        deliveries.len() == 1,
        "intervention must record exactly one typed external delivery, recorded {}",
        deliveries.len()
    );
    let delivery = deliveries.pop().expect("length checked");
    anyhow::ensure!(
        delivery.system == config.target
            && delivery.good == config.good
            && delivery.quantity == config.quantity
            && delivery.tick == config.delivery_tick,
        "recorded external delivery did not match the requested intervention"
    );
    let divergence = first_divergence.context(
        "the recorded intervention produced no stage or population divergence within the configured horizon",
    )?;
    anyhow::ensure!(
        divergence.tick > config.delivery_tick && divergence.tick <= config.horizon,
        "player-impact divergence was outside the configured post-delivery horizon"
    );
    let baseline_reconciliation = reconcile_energy(&baseline.snapshot(), baseline_initial_physical)
        .context("baseline energy reconciliation failed")?;
    let intervention_reconciliation =
        reconcile_energy(&intervention.snapshot(), intervention_initial_physical)
            .context("intervention energy reconciliation failed")?;
    Ok(PlayerImpactReport {
        initial_snapshots_identical: true,
        pre_delivery_snapshots_identical: true,
        horizon: config.horizon,
        delivery,
        divergence,
        baseline_reconciliation,
        intervention_reconciliation,
    })
}

fn print_player_impact_report(report: &PlayerImpactReport) {
    println!(
        "Player-impact differential: identical_seed=true deterministic_rng=unused target={} delivery_tick={} good={} quantity={} horizon={}",
        report.delivery.system,
        report.delivery.tick,
        report.delivery.good,
        report.delivery.quantity,
        report.horizon,
    );
    println!(
        "baseline {}",
        format_reconciliation(&report.baseline_reconciliation)
    );
    println!(
        "intervention {}",
        format_reconciliation(&report.intervention_reconciliation)
    );
    println!(
        "{} status=bounded",
        format_impact_divergence(&report.divergence)
    );
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
    fleet_spawns: u64,
    fleet_retirements: u64,
}

const FINAL_ACTIVITY_WINDOW_TICKS: u64 = 1_000;
const FINAL_POPULATION_STABILITY_WINDOW_TICKS: u64 = 100;

#[derive(Clone, Debug, Eq, PartialEq)]
struct SoakSummary {
    initial_populations: BTreeMap<ContentId, u64>,
    minimum_populations: BTreeMap<ContentId, u64>,
    final_populations: BTreeMap<ContentId, u64>,
    final_stability_minimums: BTreeMap<ContentId, u64>,
    final_stability_maximums: BTreeMap<ContentId, u64>,
    minimum_aggregate_population: u64,
    final_window_trades: u64,
    final_window_stage_transitions: u64,
    post_midpoint_stage_transitions: u64,
    population_changes: u64,
    maximum_stationary_laden_streak: u64,
    final_window_ticks: u64,
    final_stages: BTreeMap<ContentId, BrownoutStage>,
    market_ledgers: BTreeMap<ContentId, game_core::MarketLedger>,
    market_energy_flows: BTreeMap<ContentId, game_core::EnergyFlowLedger>,
    global_energy_flow: game_core::GlobalEnergyFlowLedger,
    dynamics_history: game_core::AggregateDynamicsHistory,
    reconciliation: ReconciliationReport,
}

struct SoakTracker {
    ticks: u64,
    final_window_start: u64,
    final_stability_start: u64,
    initial_populations: BTreeMap<ContentId, u64>,
    minimum_populations: BTreeMap<ContentId, u64>,
    final_stability_minimums: BTreeMap<ContentId, u64>,
    final_stability_maximums: BTreeMap<ContentId, u64>,
    minimum_aggregate_population: u64,
    final_window_trades: u64,
    final_window_stage_transitions: u64,
    post_midpoint_stage_transitions: u64,
    population_changes: u64,
    stationary_laden_streaks: BTreeMap<ContentId, u64>,
    maximum_stationary_laden_streak: u64,
}

impl SoakTracker {
    fn new(initial: &CoreSnapshot, ticks: u64) -> Self {
        let initial_populations = initial
            .markets
            .iter()
            .map(|market| (market.system_id.clone(), market.population))
            .collect::<BTreeMap<_, _>>();
        Self {
            ticks,
            final_window_start: ticks.saturating_sub(ticks.min(FINAL_ACTIVITY_WINDOW_TICKS)),
            final_stability_start: ticks
                .saturating_sub(ticks.min(FINAL_POPULATION_STABILITY_WINDOW_TICKS)),
            minimum_aggregate_population: initial_populations.values().sum(),
            minimum_populations: initial_populations.clone(),
            initial_populations,
            final_stability_minimums: BTreeMap::new(),
            final_stability_maximums: BTreeMap::new(),
            final_window_trades: 0,
            final_window_stage_transitions: 0,
            post_midpoint_stage_transitions: 0,
            population_changes: 0,
            stationary_laden_streaks: BTreeMap::new(),
            maximum_stationary_laden_streak: 0,
        }
    }

    fn observe_event(&mut self, tick: u64, event: &GameEvent) {
        let in_final_window = tick > self.final_window_start;
        match event {
            GameEvent::Bought { .. } | GameEvent::Sold { .. } if in_final_window => {
                self.final_window_trades += 1;
            }
            GameEvent::BrownoutTransition { .. } => {
                if in_final_window {
                    self.final_window_stage_transitions += 1;
                }
                if tick > self.ticks / 2 {
                    self.post_midpoint_stage_transitions += 1;
                }
            }
            GameEvent::PopulationChanged { .. } => {
                self.population_changes += 1;
            }
            _ => {}
        }
    }

    fn observe_snapshot(&mut self, tick: u64, snapshot: &CoreSnapshot) {
        let aggregate = snapshot
            .markets
            .iter()
            .map(|market| market.population)
            .sum::<u64>();
        self.minimum_aggregate_population = self.minimum_aggregate_population.min(aggregate);
        for market in &snapshot.markets {
            self.minimum_populations
                .entry(market.system_id.clone())
                .and_modify(|minimum| *minimum = (*minimum).min(market.population))
                .or_insert(market.population);
            if tick > self.final_stability_start {
                self.final_stability_minimums
                    .entry(market.system_id.clone())
                    .and_modify(|minimum| *minimum = (*minimum).min(market.population))
                    .or_insert(market.population);
                self.final_stability_maximums
                    .entry(market.system_id.clone())
                    .and_modify(|maximum| *maximum = (*maximum).max(market.population))
                    .or_insert(market.population);
            }
        }
        if tick <= self.final_window_start {
            return;
        }
        let stationary = snapshot
            .traders
            .iter()
            .filter(|trader| !trader.player && trader.travel.is_none() && !trader.cargo.is_empty())
            .map(|trader| trader.id.clone())
            .collect::<BTreeSet<_>>();
        self.stationary_laden_streaks
            .retain(|trader, _| stationary.contains(trader));
        for trader in stationary {
            let streak = self.stationary_laden_streaks.entry(trader).or_default();
            *streak += 1;
            self.maximum_stationary_laden_streak =
                self.maximum_stationary_laden_streak.max(*streak);
        }
    }

    fn finish(self, snapshot: &CoreSnapshot, reconciliation: ReconciliationReport) -> SoakSummary {
        SoakSummary {
            initial_populations: self.initial_populations,
            minimum_populations: self.minimum_populations,
            final_populations: snapshot
                .markets
                .iter()
                .map(|market| (market.system_id.clone(), market.population))
                .collect(),
            final_stability_minimums: self.final_stability_minimums,
            final_stability_maximums: self.final_stability_maximums,
            minimum_aggregate_population: self.minimum_aggregate_population,
            final_window_trades: self.final_window_trades,
            final_window_stage_transitions: self.final_window_stage_transitions,
            post_midpoint_stage_transitions: self.post_midpoint_stage_transitions,
            population_changes: self.population_changes,
            maximum_stationary_laden_streak: self.maximum_stationary_laden_streak,
            final_window_ticks: self.ticks.min(FINAL_ACTIVITY_WINDOW_TICKS),
            final_stages: snapshot
                .markets
                .iter()
                .map(|market| (market.system_id.clone(), market.brownout.stage))
                .collect(),
            market_ledgers: snapshot
                .markets
                .iter()
                .map(|market| (market.system_id.clone(), market.ledger))
                .collect(),
            market_energy_flows: snapshot
                .markets
                .iter()
                .map(|market| (market.system_id.clone(), market.energy_flow))
                .collect(),
            global_energy_flow: snapshot.energy_flow,
            dynamics_history: snapshot.dynamics_history.clone(),
            reconciliation,
        }
    }
}

fn validate_metastability(summary: &SoakSummary, label: &str) -> Result<()> {
    let initial_population = summary.initial_populations.values().sum::<u64>();
    let final_population = summary.final_populations.values().sum::<u64>();
    anyhow::ensure!(
        summary
            .initial_populations
            .keys()
            .eq(summary.minimum_populations.keys())
            && summary
                .initial_populations
                .keys()
                .eq(summary.final_populations.keys()),
        "{label} soak population maps do not describe the same systems"
    );
    anyhow::ensure!(
        summary.initial_populations.iter().all(|(system, initial)| {
            *initial > 0
                && summary
                    .minimum_populations
                    .get(system)
                    .is_some_and(|value| *value > 0)
                && summary
                    .final_populations
                    .get(system)
                    .is_some_and(|value| *value > 0)
        }),
        "{label} soak contains an extinct system"
    );
    anyhow::ensure!(
        u128::from(summary.minimum_aggregate_population) * 2 >= u128::from(initial_population),
        "{label} soak suffered a global population collapse: initial={initial_population} minimum={}",
        summary.minimum_aggregate_population
    );
    anyhow::ensure!(
        u128::from(final_population) * 100 >= u128::from(initial_population) * 90,
        "{label} soak has a severe population ratchet: {initial_population} -> {final_population}"
    );
    anyhow::ensure!(
        summary.final_window_trades > 0 && summary.final_window_stage_transitions > 0,
        "{label} soak lost final-window trade/stage activity"
    );
    anyhow::ensure!(
        summary.post_midpoint_stage_transitions > 0,
        "{label} soak had no post-midpoint stage transition"
    );
    let stable_changed_system = summary.initial_populations.iter().any(|(system, initial)| {
        let changed = summary
            .minimum_populations
            .get(system)
            .is_some_and(|minimum| minimum != initial)
            || summary
                .final_populations
                .get(system)
                .is_some_and(|final_population| final_population != initial);
        changed
            && summary.final_stability_minimums.get(system)
                == summary.final_stability_maximums.get(system)
            && summary.final_stability_minimums.get(system) == summary.final_populations.get(system)
    });
    anyhow::ensure!(
        summary.population_changes > 0 && stable_changed_system,
        "{label} soak had no changed population stable over the final {} ticks",
        FINAL_POPULATION_STABILITY_WINDOW_TICKS
    );
    anyhow::ensure!(
        summary.maximum_stationary_laden_streak < summary.final_window_ticks,
        "{label} soak has a stationary laden deadlock"
    );
    Ok(())
}

#[cfg(test)]
fn compare_deterministic_outcomes(
    baseline: &SoakSummary,
    comparison: &SoakSummary,
    label: &str,
) -> Result<()> {
    anyhow::ensure!(
        baseline.final_populations == comparison.final_populations
            && baseline.final_stages == comparison.final_stages
            && baseline.market_ledgers == comparison.market_ledgers
            && baseline.market_energy_flows == comparison.market_energy_flows
            && baseline.global_energy_flow == comparison.global_energy_flow
            && baseline.dynamics_history == comparison.dynamics_history
            && baseline.reconciliation == comparison.reconciliation,
        "{label} insertion-order outcome diverged"
    );
    Ok(())
}

fn format_soak_summary(summary: &SoakSummary) -> String {
    format!(
        "population={}->{} minimum_population={} population_changes={} stage_transitions={} post_midpoint_transitions={} final_window_trades={} final_window_stage_transitions={} max_stationary_laden_streak={}",
        summary.initial_populations.values().sum::<u64>(),
        summary.final_populations.values().sum::<u64>(),
        summary.minimum_aggregate_population,
        summary.population_changes,
        summary.dynamics_history.stage_transitions,
        summary.post_midpoint_stage_transitions,
        summary.final_window_trades,
        summary.final_window_stage_transitions,
        summary.maximum_stationary_laden_streak,
    )
}

#[cfg(test)]
fn run_compact_soak(definition: GameDefinition, ticks: u64) -> Result<SoakSummary> {
    let mut session = GameSession::new(definition).context("failed to construct compact soak")?;
    let initial = session.snapshot();
    let initial_physical = physical_energy(&initial)?;
    let mut tracker = SoakTracker::new(&initial, ticks);
    for tick in 1..=ticks {
        session.step()?;
        for event in session.drain_events() {
            tracker.observe_event(tick, &event);
        }
        tracker.observe_snapshot(tick, &session.snapshot());
    }
    let snapshot = session.snapshot();
    let reconciliation = reconcile_energy(&snapshot, initial_physical)
        .context("compact soak energy reconciliation failed")?;
    Ok(tracker.finish(&snapshot, reconciliation))
}

#[derive(Clone, Copy, Debug)]
struct CycleAmplitude {
    minimum_effective_output: i64,
    maximum_effective_output: i64,
    minimum_storage_basis_points: u64,
    maximum_storage_basis_points: u64,
    minimum_stage: BrownoutStage,
    maximum_stage: BrownoutStage,
}

fn storage_basis_points(stock: i64, capacity: i64) -> u64 {
    if stock <= 0 || capacity <= 0 {
        return 0;
    }
    u64::try_from(
        i128::from(stock)
            .saturating_mul(10_000)
            .checked_div(i128::from(capacity))
            .unwrap_or(0),
    )
    .unwrap_or(u64::MAX)
}

fn format_basis_points(value: u64) -> String {
    format!("{}.{:02}%", value / 100, value % 100)
}

fn update_cycle_amplitudes(
    amplitudes: &mut BTreeMap<ContentId, CycleAmplitude>,
    snapshot: &CoreSnapshot,
) {
    for market in &snapshot.markets {
        let output = market.seasonal_generation.current_effective_output.0;
        let storage = storage_basis_points(market.energy_stock.0, market.energy_storage_cap.0);
        amplitudes
            .entry(market.system_id.clone())
            .and_modify(|amplitude| {
                amplitude.minimum_effective_output = amplitude.minimum_effective_output.min(output);
                amplitude.maximum_effective_output = amplitude.maximum_effective_output.max(output);
                amplitude.minimum_storage_basis_points =
                    amplitude.minimum_storage_basis_points.min(storage);
                amplitude.maximum_storage_basis_points =
                    amplitude.maximum_storage_basis_points.max(storage);
                amplitude.minimum_stage = amplitude.minimum_stage.min(market.brownout.stage);
                amplitude.maximum_stage = amplitude.maximum_stage.max(market.brownout.stage);
            })
            .or_insert(CycleAmplitude {
                minimum_effective_output: output,
                maximum_effective_output: output,
                minimum_storage_basis_points: storage,
                maximum_storage_basis_points: storage,
                minimum_stage: market.brownout.stage,
                maximum_stage: market.brownout.stage,
            });
    }
}

fn format_stage_percentages(counts: [u64; 4]) -> String {
    let total = counts.iter().sum::<u64>();
    let percent = |stage: BrownoutStage| {
        let basis_points = counts[stage.index()]
            .saturating_mul(10_000)
            .checked_div(total)
            .unwrap_or(0);
        format_basis_points(basis_points)
    };
    format!(
        "normal={} throttled={} emergency={} starvation={}",
        percent(BrownoutStage::Normal),
        percent(BrownoutStage::Throttled),
        percent(BrownoutStage::Emergency),
        percent(BrownoutStage::Starvation),
    )
}

fn format_network_dynamics(snapshot: &CoreSnapshot) -> String {
    let mut current = [0_u64; 4];
    for market in &snapshot.markets {
        current[market.brownout.stage.index()] += 1;
    }
    format!(
        "network_stages current[{}] occupancy[{}] transitions={} population_changes={} population_milestones={} npc_fleet_size={} normalized_unserved_opportunity_per_system={} opportunity_persistence={} fleet_spawns={} fleet_retirements={}",
        format_stage_percentages(current),
        format_stage_percentages(snapshot.dynamics_history.stage_occupancy_ticks),
        snapshot.dynamics_history.stage_transitions,
        snapshot.dynamics_history.population_changes,
        snapshot.dynamics_history.population_milestones,
        snapshot
            .traders
            .iter()
            .filter(|trader| !trader.player)
            .count(),
        snapshot.fleet.normalized_unserved_opportunity,
        snapshot.fleet.opportunity_persistence,
        snapshot.dynamics_history.fleet_spawns,
        snapshot.dynamics_history.fleet_retirements,
    )
}

fn format_profitability_window(values: &[i64]) -> String {
    let sum = values.iter().map(|value| i128::from(*value)).sum::<i128>();
    let average = if values.is_empty() {
        0.0
    } else {
        sum as f64 / values.len() as f64
    };
    format!("samples={} sum={sum} average={average:.1}", values.len())
}

fn sampled_population_trajectory(values: &VecDeque<u32>) -> Vec<u32> {
    values.iter().rev().take(16).copied().rev().collect()
}

fn format_system_dynamics(market: &game_core::MarketSnapshot, previous_stock: i64) -> String {
    let phase = market.seasonal_phase;
    format!(
        "system={} net_flow={} storage={} stage={} occupancy=[{}] transitions={} population={} population_trend={} population_cap={} population_tier={} population_sufficiency_average={} population_trajectory={:?} generation_base={} generation_effective={} seasonal_phase={}/{}:{} next_turning_point={} ticks_to_turn={}",
        market.system_id,
        i128::from(market.energy_stock.0) - i128::from(previous_stock),
        format_basis_points(storage_basis_points(
            market.energy_stock.0,
            market.energy_storage_cap.0
        )),
        market.brownout.stage.label(),
        format_stage_percentages(market.brownout.occupancy_ticks),
        market.brownout.transition_count,
        market.population,
        market.population_state.trend.label(),
        market.population_state.carrying_capacity,
        market.population_state.tier,
        market.population_state.sufficiency_average_percent,
        sampled_population_trajectory(&market.population_state.sufficiency_samples),
        market.seasonal_generation.base_output.0,
        market.seasonal_generation.current_effective_output.0,
        phase.position_ticks,
        phase.period_ticks,
        phase.trend.label(),
        phase
            .next_turning_point_tick
            .map_or_else(|| "beyond-clock".into(), |tick| tick.to_string()),
        phase.ticks_until_turning_point,
    )
}

fn format_cycle_amplitude(market: &game_core::MarketSnapshot, amplitude: CycleAmplitude) -> String {
    format!(
        "system={} generation_min={} generation_max={} generation_amplitude={} storage_min={} storage_max={} storage_amplitude={} stage_span={}..{} transitions={}",
        market.system_id,
        amplitude.minimum_effective_output,
        amplitude.maximum_effective_output,
        i128::from(amplitude.maximum_effective_output)
            - i128::from(amplitude.minimum_effective_output),
        format_basis_points(amplitude.minimum_storage_basis_points),
        format_basis_points(amplitude.maximum_storage_basis_points),
        format_basis_points(
            amplitude
                .maximum_storage_basis_points
                .saturating_sub(amplitude.minimum_storage_basis_points)
        ),
        amplitude.minimum_stage.label(),
        amplitude.maximum_stage.label(),
        market.brownout.transition_count,
    )
}

fn run_economy_diagnostics(session: &mut GameSession, ticks: u64) -> Result<SoakSummary> {
    const REPORT_INTERVAL: u64 = 50;
    let initial = session.snapshot();
    let mut soak_tracker = SoakTracker::new(&initial, ticks);
    let initial_physical = physical_energy(&initial)?;
    let initial_stocks = initial
        .markets
        .iter()
        .map(|market| (market.system_id.clone(), market.energy_stock.0))
        .collect::<BTreeMap<_, _>>();
    let mut previous_report_stocks = initial_stocks.clone();
    let mut cycle_amplitudes = BTreeMap::new();
    update_cycle_amplitudes(&mut cycle_amplitudes, &initial);
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
            soak_tracker.observe_event(tick, &event);
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
                GameEvent::TraderSpawned { .. } => activity.fleet_spawns += 1,
                GameEvent::TraderRetired { .. } => activity.fleet_retirements += 1,
                GameEvent::EnergyLogistics(_)
                | GameEvent::ExternalDeliveryRecorded { .. }
                | GameEvent::BrownoutTransition { .. }
                | GameEvent::PopulationChanged { .. }
                | GameEvent::PopulationTierChanged { .. }
                | GameEvent::InvestmentCompleted { .. }
                | GameEvent::InvestmentDeferred { .. }
                | GameEvent::GovernorPolicyRejected { .. }
                | GameEvent::ReservationReleased { .. }
                | GameEvent::SaleDeferred { .. }
                | GameEvent::PolicyChanged { .. }
                | GameEvent::MarketTargetChanged { .. }
                | GameEvent::TickAdvanced(_)
                | GameEvent::Rejected(_) => {}
            }
        }
        let tick_snapshot = session.snapshot();
        soak_tracker.observe_snapshot(tick, &tick_snapshot);
        update_cycle_amplitudes(&mut cycle_amplitudes, &tick_snapshot);
        if tick % REPORT_INTERVAL == 0 || tick == ticks {
            let snapshot = tick_snapshot;
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
                "tick={tick} trades={} partial_sales={} reservations={} departures={} arrivals={} produced={} generated={} life_support_unsupplied={} npc_fleet_size={} npc_traveling={traveling} npc_idle={idle} npc_stationary_laden={blocked_laden} fleet_spawns={} fleet_retirements={} normalized_unserved_opportunity_per_system={} opportunity_persistence={}",
                activity.trades,
                activity.partial_sales,
                activity.reservations,
                activity.departures,
                activity.arrivals,
                activity.produced,
                activity.generated,
                activity.life_support_unsupplied,
                traveling + idle + blocked_laden,
                activity.fleet_spawns,
                activity.fleet_retirements,
                snapshot.fleet.normalized_unserved_opportunity,
                snapshot.fleet.opportunity_persistence,
            );
            println!("{}", format_network_dynamics(&snapshot));
            for market in &snapshot.markets {
                let previous = previous_report_stocks
                    .get(&market.system_id)
                    .copied()
                    .unwrap_or(market.energy_stock.0);
                println!("  {}", format_system_dynamics(market, previous));
                previous_report_stocks.insert(market.system_id.clone(), market.energy_stock.0);
            }
            activity = DiagnosticActivity::default();
        }
    }

    let snapshot = session.snapshot();
    let reconciliation = reconcile_energy(&snapshot, initial_physical)
        .context("economy diagnostic energy reconciliation failed")?;
    println!("{}", format_reconciliation(&reconciliation));
    println!("markets:");
    for market in &snapshot.markets {
        let flow = market.energy_flow;
        let burned = i128::from(flow.life_support_burned.0)
            + i128::from(flow.source_burned.0)
            + i128::from(flow.production_burned.0)
            + i128::from(flow.investment_burned.0)
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
            "  {} energy={}/{} storage={} net_flow={} health={} stage={} occupancy=[{}] transitions={} population={} population_trend={} population_cap={} population_tier={} population_sufficiency_average={} population_changes={} generation_base={} generation_effective={} seasonal_phase={}/{}:{} next_turning_point={} reserved_claims={} operating_reserve={} protected_budget={} unreserved_purchasing={} funded_demand_units={} generated={} external_inflow={} burned={} curtailed={} unsupplied_life_support={} avg_unit_cost={} policy_margin={}% realized_processor_input_cost={} realized_processor_operating_energy={} realized_processor_output_revenue={} realized_processor_margin={} targets_met={}/{} bootstrap_risk_acknowledged={} net_trade_energy={} paid={} received={} units_bought={} units_sold={} source_units={} recipe_inputs={} recipe_outputs={}",
            market.name,
            market.energy_stock.0,
            market.energy_storage_cap.0,
            format_basis_points(storage_basis_points(
                market.energy_stock.0,
                market.energy_storage_cap.0
            )),
            i128::from(market.energy_stock.0)
                - i128::from(
                    initial_stocks
                        .get(&market.system_id)
                        .copied()
                        .unwrap_or(market.energy_stock.0)
                ),
            market_health(market),
            market.brownout.stage.label(),
            format_stage_percentages(market.brownout.occupancy_ticks),
            market.brownout.transition_count,
            market.population,
            market.population_state.trend.label(),
            market.population_state.carrying_capacity,
            market.population_state.tier,
            market.population_state.sufficiency_average_percent,
            market.population_state.settled_changes,
            market.seasonal_generation.base_output.0,
            market.seasonal_generation.current_effective_output.0,
            market.seasonal_phase.position_ticks,
            market.seasonal_phase.period_ticks,
            market.seasonal_phase.trend.label(),
            market
                .seasonal_phase
                .next_turning_point_tick
                .map_or_else(|| "beyond-clock".into(), |tick| tick.to_string()),
            market.reserved_energy.0,
            market.operating_reserve.0,
            market.protected_liquidation_budget.0,
            market.unreserved_energy_for_purchases.0,
            funded_demand_units,
            flow.generated.0,
            flow.external_inflow.0,
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
    println!("{}", format_network_dynamics(&snapshot));
    println!("cycle_amplitudes:");
    for market in &snapshot.markets {
        if let Some(amplitude) = cycle_amplitudes.get(&market.system_id) {
            println!("  {}", format_cycle_amplitude(market, *amplitude));
        }
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
            "  {} state={state} system={} tank={}/{} bay_energy={} cargo_units={}/{} reservation={:?} retirement={:?} failed_liquidation_ticks={} profitability_window=[{}] transactions={} net_trade_energy={}",
            trader.name,
            trader.system,
            trader.energy_tank.0,
            trader.energy_tank_capacity.0,
            bay_energy,
            trader.cargo.values().sum::<u64>(),
            trader.cargo_capacity,
            trader.reservation,
            trader.retirement,
            trader.failed_liquidation_ticks,
            format_profitability_window(&trader.profitability_window),
            trader.ledger.completed_transactions,
            i128::from(trader.ledger.sales_revenue.0)
                - i128::from(trader.ledger.purchase_cost.0)
                - i128::from(trader.ledger.travel_cost.0),
        );
    }
    Ok(soak_tracker.finish(&snapshot, reconciliation))
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

fn checked_total(values: impl IntoIterator<Item = i128>, label: &str) -> Result<i128> {
    values.into_iter().try_fold(0_i128, |total, value| {
        total
            .checked_add(value)
            .with_context(|| format!("{label} overflow"))
    })
}

fn physical_energy_from_parts(
    markets: impl IntoIterator<Item = i128>,
    tanks: impl IntoIterator<Item = i128>,
    cargo: impl IntoIterator<Item = i128>,
) -> Result<i128> {
    let markets = checked_total(markets, "market energy total")?;
    let tanks = checked_total(tanks, "tank energy total")?;
    let cargo = checked_total(cargo, "energy cargo total")?;
    markets
        .checked_add(tanks)
        .and_then(|value| value.checked_add(cargo))
        .context("physical energy total overflow")
}

fn physical_energy(snapshot: &CoreSnapshot) -> Result<i128> {
    physical_energy_from_parts(
        snapshot
            .markets
            .iter()
            .map(|market| i128::from(market.energy_stock.0)),
        snapshot
            .traders
            .iter()
            .map(|trader| i128::from(trader.energy_tank.0)),
        snapshot.traders.iter().filter_map(|trader| {
            trader
                .cargo
                .iter()
                .find(|(good, _)| good.as_str() == ENERGY_ID)
                .map(|(_, quantity)| i128::from(*quantity))
        }),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReconciliationReport {
    initial: i128,
    external_inflow: i128,
    generated: i128,
    burned: i128,
    curtailed: i128,
    expected: i128,
    actual: i128,
    market_to_tank: i128,
    tank_to_market: i128,
    market_to_energy_cargo: i128,
    energy_cargo_to_market: i128,
}

fn reconcile_energy(
    snapshot: &CoreSnapshot,
    initial_physical: i128,
) -> Result<ReconciliationReport> {
    let flow = snapshot.energy_flow;
    let external_inflow = i128::from(flow.external_inflow.0);
    let generated = i128::from(flow.generated.0);
    let burned = checked_total(
        [
            i128::from(flow.life_support_burned.0),
            i128::from(flow.source_burned.0),
            i128::from(flow.production_burned.0),
            i128::from(flow.investment_burned.0),
            i128::from(flow.travel_burned.0),
        ],
        "burned energy total",
    )?;
    let curtailed = i128::from(flow.curtailed.0);
    let expected = initial_physical
        .checked_add(external_inflow)
        .and_then(|value| value.checked_add(generated))
        .and_then(|value| value.checked_sub(burned))
        .and_then(|value| value.checked_sub(curtailed))
        .context("expected physical energy calculation overflow")?;
    let actual = physical_energy(snapshot)?;
    let difference = actual
        .checked_sub(expected)
        .context("energy reconciliation difference overflow")?;
    anyhow::ensure!(
        difference == 0,
        "energy reconciliation mismatch: expected {expected}, actual {actual}, difference {difference}"
    );
    Ok(ReconciliationReport {
        initial: initial_physical,
        external_inflow,
        generated,
        burned,
        curtailed,
        expected,
        actual,
        market_to_tank: i128::from(flow.market_to_tank.0),
        tank_to_market: i128::from(flow.tank_to_market.0),
        market_to_energy_cargo: i128::from(flow.market_to_energy_cargo.0),
        energy_cargo_to_market: i128::from(flow.energy_cargo_to_market.0),
    })
}

fn format_reconciliation(report: &ReconciliationReport) -> String {
    format!(
        "energy_reconciliation initial={} external_inflow={} generated={} burned={} curtailed={} expected={} actual={} difference=0 status=ok physical_transfers market_to_tank={} tank_to_market={} market_to_energy_cargo={} energy_cargo_to_market={}",
        report.initial,
        report.external_inflow,
        report.generated,
        report.burned,
        report.curtailed,
        report.expected,
        report.actual,
        report.market_to_tank,
        report.tank_to_market,
        report.market_to_energy_cargo,
        report.energy_cargo_to_market,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_soak_summary(
        initial: &[(&str, u64)],
        minimum: &[(&str, u64)],
        final_values: &[(&str, u64)],
    ) -> SoakSummary {
        let populations = |values: &[(&str, u64)]| {
            values
                .iter()
                .map(|(system, population)| (ContentId::new(*system).unwrap(), *population))
                .collect::<BTreeMap<_, _>>()
        };
        let final_populations = populations(final_values);
        let systems = final_populations.keys().cloned().collect::<Vec<_>>();
        SoakSummary {
            initial_populations: populations(initial),
            minimum_populations: populations(minimum),
            final_stability_minimums: final_populations.clone(),
            final_stability_maximums: final_populations.clone(),
            minimum_aggregate_population: minimum.iter().map(|(_, value)| *value).sum(),
            final_populations,
            final_window_trades: 1,
            final_window_stage_transitions: 1,
            post_midpoint_stage_transitions: 1,
            population_changes: 1,
            maximum_stationary_laden_streak: 0,
            final_window_ticks: FINAL_ACTIVITY_WINDOW_TICKS,
            final_stages: systems
                .iter()
                .cloned()
                .map(|system| (system, BrownoutStage::Normal))
                .collect(),
            market_ledgers: systems
                .iter()
                .cloned()
                .map(|system| (system, game_core::MarketLedger::default()))
                .collect(),
            market_energy_flows: systems
                .iter()
                .cloned()
                .map(|system| (system, game_core::EnergyFlowLedger::default()))
                .collect(),
            global_energy_flow: game_core::GlobalEnergyFlowLedger::default(),
            dynamics_history: game_core::AggregateDynamicsHistory {
                stage_transitions: 1,
                population_changes: 1,
                ..game_core::AggregateDynamicsHistory::default()
            },
            reconciliation: ReconciliationReport {
                initial: 0,
                external_inflow: 0,
                generated: 0,
                burned: 0,
                curtailed: 0,
                expected: 0,
                actual: 0,
                market_to_tank: 0,
                tank_to_market: 0,
                market_to_energy_cargo: 0,
                energy_cargo_to_market: 0,
            },
        }
    }

    #[test]
    fn metastability_rejects_a_fifty_percent_monotonic_decline() {
        let summary =
            synthetic_soak_summary(&[("test:a", 100)], &[("test:a", 50)], &[("test:a", 50)]);
        assert!(validate_metastability(&summary, "synthetic").is_err());
    }

    #[test]
    fn metastability_rejects_any_system_extinction() {
        let summary = synthetic_soak_summary(
            &[("test:a", 100), ("test:b", 1)],
            &[("test:a", 100), ("test:b", 0)],
            &[("test:a", 100), ("test:b", 0)],
        );
        assert!(validate_metastability(&summary, "synthetic").is_err());
    }

    #[test]
    fn deterministic_comparison_rejects_equal_aggregate_different_population_maps() {
        let baseline = synthetic_soak_summary(
            &[("test:a", 100), ("test:b", 100)],
            &[("test:a", 95), ("test:b", 100)],
            &[("test:a", 95), ("test:b", 105)],
        );
        let comparison = synthetic_soak_summary(
            &[("test:a", 100), ("test:b", 100)],
            &[("test:a", 90), ("test:b", 100)],
            &[("test:a", 90), ("test:b", 110)],
        );
        assert_eq!(
            baseline.final_populations.values().sum::<u64>(),
            comparison.final_populations.values().sum::<u64>()
        );
        assert!(compare_deterministic_outcomes(&baseline, &comparison, "synthetic").is_err());
    }

    #[test]
    fn metastability_accepts_decline_then_final_stabilization_or_recovery() {
        let summary =
            synthetic_soak_summary(&[("test:a", 100)], &[("test:a", 80)], &[("test:a", 95)]);
        validate_metastability(&summary, "synthetic").unwrap();
    }

    #[test]
    fn short_system_only_and_trader_only_permutations_match_key_outcomes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let definition = game_content::load_directory(root).unwrap();
        let baseline = run_compact_soak(definition.clone(), 10).unwrap();

        let mut systems_reversed = definition.clone();
        systems_reversed.systems.reverse();
        let system_outcome = run_compact_soak(systems_reversed, 10).unwrap();
        compare_deterministic_outcomes(&baseline, &system_outcome, "system-only").unwrap();

        let mut traders_reversed = definition;
        traders_reversed.traders.reverse();
        let trader_outcome = run_compact_soak(traders_reversed, 10).unwrap();
        compare_deterministic_outcomes(&baseline, &trader_outcome, "trader-only").unwrap();
    }

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
    fn execution_modes_reject_every_conflicting_pair() {
        let modes = [
            "--validate-content",
            "--player-impact",
            "--compare-pricing-modes",
            "--economy-diagnostics",
            "--headless",
        ];
        for (index, left) in modes.iter().enumerate() {
            assert_ne!(
                execution_mode_argument(&[(*left).into()]).unwrap(),
                ExecutionMode::Tui
            );
            for right in &modes[index + 1..] {
                let error = execution_mode_argument(&[(*left).into(), (*right).into()])
                    .unwrap_err()
                    .to_string();
                assert!(error.contains(left), "missing {left} in {error}");
                assert!(error.contains(right), "missing {right} in {error}");
            }
        }
        assert_eq!(execution_mode_argument(&[]).unwrap(), ExecutionMode::Tui);
    }

    #[test]
    fn player_impact_argument_requires_a_bounded_explicit_delivery() {
        let args = [
            "--player-impact",
            "--impact-target",
            "frontier:system_19",
            "--impact-tick",
            "25",
            "--impact-good",
            "core:energy",
            "--impact-quantity",
            "500",
            "--impact-horizon",
            "200",
        ]
        .map(String::from);
        let parsed = player_impact_argument(&args).unwrap();
        assert_eq!(parsed.target.as_str(), "frontier:system_19");
        assert_eq!(parsed.delivery_tick, 25);
        assert_eq!(parsed.good.as_str(), ENERGY_ID);
        assert_eq!(parsed.quantity, 500);
        assert_eq!(parsed.horizon, 200);

        let mut invalid = args;
        invalid[4] = "200".into();
        assert!(player_impact_argument(&invalid).is_err());
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
    fn profitability_diagnostics_are_summarized() {
        assert_eq!(
            format_profitability_window(&[-10, 5, 20]),
            "samples=3 sum=15 average=5.0"
        );
        assert_eq!(
            format_profitability_window(&[]),
            "samples=0 sum=0 average=0.0"
        );
    }

    #[test]
    fn reconciliation_formatter_reports_exact_flow() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let definition = game_content::load_directory(root).unwrap();
        let mut session = GameSession::new(definition).unwrap();
        let initial = physical_energy(&session.snapshot()).unwrap();
        session
            .submit(game_core::GameCommand::RecordExternalDelivery {
                system: ContentId::new("frontier:system_01").unwrap(),
                good: ContentId::new(ENERGY_ID).unwrap(),
                quantity: 1,
            })
            .unwrap();
        session.step().unwrap();
        let snapshot = session.snapshot();
        let reconciliation = reconcile_energy(&snapshot, initial).unwrap();
        let output = format_reconciliation(&reconciliation);
        assert!(output.contains("energy_reconciliation"));
        assert!(output.contains("external_inflow=1"));
        assert!(output.contains("generated="));
        assert!(output.contains("burned="));
        assert!(output.contains("curtailed="));
        assert!(output.contains("difference=0 status=ok"), "{output}");
        let system = format_system_dynamics(&snapshot.markets[0], 2_000);
        assert!(system.contains("net_flow="));
        assert!(system.contains("storage="));
        assert!(system.contains("stage="));
        assert!(system.contains("occupancy=["));
        assert!(system.contains("generation_base="));
        assert!(system.contains("seasonal_phase="));
        let network = format_network_dynamics(&snapshot);
        assert!(network.contains("network_stages current["));
        assert!(network.contains("npc_fleet_size=9"));
        assert!(network.contains("normalized_unserved_opportunity_per_system="));
        assert!(network.contains("opportunity_persistence="));
        let amplitude = CycleAmplitude {
            minimum_effective_output: 5,
            maximum_effective_output: 15,
            minimum_storage_basis_points: 2_500,
            maximum_storage_basis_points: 7_500,
            minimum_stage: BrownoutStage::Normal,
            maximum_stage: BrownoutStage::Emergency,
        };
        let amplitude_output = format_cycle_amplitude(&snapshot.markets[0], amplitude);
        assert!(amplitude_output.contains("generation_amplitude=10"));
        assert!(amplitude_output.contains("storage_amplitude=50.00%"));
        assert!(amplitude_output.contains("stage_span=Normal..Emergency"));
        let divergence = format_impact_divergence(&ImpactDivergence {
            tick: 7,
            target: ContentId::new("frontier:system_19").unwrap(),
            baseline_stage: BrownoutStage::Emergency,
            intervention_stage: BrownoutStage::Throttled,
            baseline_population: 8,
            intervention_population: 8,
        });
        assert!(divergence.contains("first_divergence_tick=7"));
        assert!(divergence.contains("baseline_stage=Emergency"));
        assert!(divergence.contains("intervention_stage=Throttled"));
    }

    #[test]
    fn reconciliation_rejects_mismatch_and_physical_total_calculation_error() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let definition = game_content::load_directory(root).unwrap();
        let mut session = GameSession::new(definition).unwrap();
        let mut snapshot = session.snapshot();
        let initial = physical_energy(&snapshot).unwrap();
        snapshot.markets[0].energy_stock.0 += 1;
        let mismatch = reconcile_energy(&snapshot, initial)
            .unwrap_err()
            .to_string();
        assert!(mismatch.contains("reconciliation mismatch"), "{mismatch}");

        let calculation_error = physical_energy_from_parts([i128::MAX, 1], [], [])
            .unwrap_err()
            .to_string();
        assert!(
            calculation_error.contains("market energy total overflow"),
            "{calculation_error}"
        );
    }

    #[test]
    fn player_impact_report_proves_controlled_end_to_end_difference_and_both_reconciliations() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let definition = game_content::load_directory(root).unwrap();
        let config = PlayerImpactConfig {
            target: ContentId::new("frontier:system_04").unwrap(),
            delivery_tick: 300,
            good: ContentId::new(ENERGY_ID).unwrap(),
            quantity: 500,
            horizon: 500,
        };
        let report = run_player_impact(&definition, &config).unwrap();
        assert!(report.initial_snapshots_identical);
        assert!(report.pre_delivery_snapshots_identical);
        assert_eq!(
            report.delivery,
            ImpactDelivery {
                system: config.target.clone(),
                good: config.good.clone(),
                quantity: config.quantity,
                energy_inflow: 500,
                tick: config.delivery_tick,
            }
        );
        assert!(report.divergence.tick > config.delivery_tick);
        assert!(report.divergence.tick <= config.horizon);
        assert!(
            report.divergence.baseline_stage != report.divergence.intervention_stage
                || report.divergence.baseline_population
                    != report.divergence.intervention_population
        );
        assert_eq!(
            report.baseline_reconciliation.expected,
            report.baseline_reconciliation.actual
        );
        assert_eq!(
            report.intervention_reconciliation.expected,
            report.intervention_reconciliation.actual
        );
        assert_eq!(report.baseline_reconciliation.external_inflow, 0);
        assert_eq!(report.intervention_reconciliation.external_inflow, 500);
    }
}
