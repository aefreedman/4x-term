//! Async owner and immutable view boundary for the headless simulation.

use game_core::{
    BrownoutStage, CoreError, ENERGY_ID, GameCommand, GameEvent, GameSession, MarketAuthority,
    ReservationStatus, SeasonalTrend, investment_cost, route_travel_energy, ticks_for_distance,
    travel_energy,
};
pub use game_core::{
    ContentId, Energy, GovernorInvestmentPolicy, GovernorMarketPolicy, InvestmentKind,
    InvestmentStatus, PopulationTrend, TradeNetworkAccess,
};
use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

const REQUEST_CAPACITY: usize = 64;
const EVENT_HISTORY: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunState {
    Paused,
    Running,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TickRate {
    Slow,
    Normal,
    Fast,
}

impl TickRate {
    #[must_use]
    pub fn duration(self) -> Duration {
        match self {
            Self::Slow => Duration::from_secs(1),
            Self::Normal => Duration::from_millis(250),
            Self::Fast => Duration::from_millis(100),
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Slow => "1/s",
            Self::Normal => "4/s",
            Self::Fast => "10/s",
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Slow => Self::Normal,
            Self::Normal => Self::Fast,
            Self::Fast => Self::Slow,
        }
    }
}

#[derive(Debug)]
pub enum AppRequest {
    SetRunState(RunState),
    Step,
    SetTickRate(TickRate),
    SelectSystem(ContentId),
    Buy {
        good: ContentId,
        quantity: u32,
    },
    Sell {
        good: ContentId,
        quantity: u32,
    },
    BeginTravel {
        destination: ContentId,
    },
    CommitTrade {
        origin: ContentId,
        destination: ContentId,
        good: ContentId,
        quantity: u32,
    },
    DepositTank {
        amount: Energy,
    },
    WithdrawTank {
        amount: Energy,
    },
    SetMarketPolicy {
        system: ContentId,
        policy: GovernorMarketPolicy,
    },
    SetInvestmentPolicy {
        system: ContentId,
        policy: GovernorInvestmentPolicy,
    },
    CancelReservation,
    Shutdown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemIdentityView {
    pub id: ContentId,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct SystemListItem {
    pub id: ContentId,
    pub name: String,
    pub coordinates: (f64, f64, f64),
    pub player_location: bool,
    pub player_governed: bool,
    pub route_distance_from_player: Option<f64>,
    pub route_ticks_from_player: Option<u32>,
    pub population: PopulationView,
    pub energy_stock: Energy,
    pub energy_capacity: Energy,
    pub health: EnergyHealth,
    pub brownout_stage: BrownoutStage,
    pub runway_ticks: u32,
    pub seasonal_generation: SeasonalGenerationView,
    pub connections: Vec<ConnectionView>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopulationView {
    pub current: u64,
    pub reference: u64,
    pub carrying_capacity: u64,
    pub trend: PopulationTrend,
    pub tier: usize,
    pub sufficiency_average_percent: u32,
    pub sufficiency_trajectory: Vec<u32>,
    pub settled_changes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AggregateDynamicsView {
    pub stage_occupancy_ticks: [u64; 4],
    pub stage_transitions: u64,
    pub population_changes: u64,
    pub population_milestones: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeasonalGenerationView {
    pub base_output: Energy,
    pub effective_output: Energy,
    pub phase_ticks: u32,
    pub period_ticks: u32,
    pub trend: SeasonalTrend,
    pub ticks_until_turning_point: u32,
    pub next_turning_point_tick: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnergyHealth {
    Healthy,
    Full,
    Low,
    Deficit,
}

impl EnergyHealth {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Full => "full",
            Self::Low => "low",
            Self::Deficit => "deficit",
        }
    }
}

#[derive(Clone, Debug)]
pub struct MarketEnergyView {
    pub stock: Energy,
    pub capacity: Energy,
    pub reserved_claims: Energy,
    pub operating_reserve: Energy,
    pub protected_liquidation_budget: Energy,
    pub unreserved_purchasing_energy: Energy,
    pub generated: Energy,
    pub burned: Energy,
    pub curtailed: Energy,
    pub unsupplied_life_support: Energy,
    pub bootstrap_risk_acknowledged: bool,
    pub health: EnergyHealth,
    pub brownout_stage: BrownoutStage,
    pub runway_ticks: u32,
    pub seasonal_generation: SeasonalGenerationView,
}

#[derive(Clone, Debug)]
pub struct ConnectionView {
    pub system_id: ContentId,
    pub system_name: String,
    pub distance: f64,
    pub travel_ticks: u32,
}

#[derive(Clone, Debug)]
pub struct RouteLegView {
    pub from_id: ContentId,
    pub from_name: String,
    pub to_id: ContentId,
    pub to_name: String,
    pub distance: f64,
    pub travel_ticks: u32,
}

#[derive(Clone, Debug)]
pub struct RouteView {
    pub destination_id: ContentId,
    pub destination_name: String,
    pub legs: Vec<RouteLegView>,
    pub current_leg: Option<usize>,
    pub total_distance: f64,
    pub total_ticks: u32,
    pub remaining_ticks: Option<u32>,
    /// Exact tank energy required by this route and destination.
    pub required_energy: Energy,
}

#[derive(Clone, Debug)]
pub struct MarketRow {
    pub good_id: ContentId,
    pub name: String,
    pub inventory: u64,
    pub target: u32,
    pub buy_quote: Energy,
    pub sell_quote: Energy,
    pub unit_cost: Energy,
    pub funded_demand: u64,
}

#[derive(Clone, Debug)]
pub struct CargoItemView {
    pub good_id: ContentId,
    pub good_name: String,
    pub quantity: u64,
}

#[derive(Clone, Debug)]
pub struct PlayerStatusView {
    pub trade_network_access: TradeNetworkAccess,
    pub location: ContentId,
    pub location_name: String,
    pub tank_energy: Energy,
    pub tank_capacity: Energy,
    pub bay_energy: u64,
    pub cargo: Vec<CargoItemView>,
    pub cargo_used: u64,
    pub cargo_capacity: u32,
    pub cargo_energy_value: Energy,
    pub total_energy_value: Energy,
    pub purchase_energy: Energy,
    pub sales_energy: Energy,
    pub realized_energy_gain: Energy,
    pub units_moved: u64,
    pub transactions: u64,
    pub energy_value_rank: usize,
    pub energy_value_share_percent: f64,
    pub sales_share_percent: f64,
    pub runway_jumps: Option<u64>,
    pub traveling: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvestmentView {
    pub kind: InvestmentKind,
    pub allocation_percent: u32,
    pub level: u32,
    pub maximum_level: u32,
    pub next_cost: Option<Energy>,
    pub cooldown_until: u64,
    pub status: InvestmentStatus,
    pub effect_per_level: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernorView {
    pub governed: bool,
    pub policy: GovernorMarketPolicy,
    pub investment_policy: GovernorInvestmentPolicy,
    pub investments: Vec<InvestmentView>,
    pub route_subsidy_percent: u32,
    pub route_subsidy_active: bool,
    pub ladder_occupancy_ticks: [u64; 4],
    pub ladder_transitions: u64,
    pub population_tier: usize,
}

#[derive(Clone, Debug)]
pub struct SystemInspectionView {
    pub system: SystemIdentityView,
    pub read_only_market: bool,
    pub market_energy: MarketEnergyView,
    pub population: PopulationView,
    pub market: Vec<MarketRow>,
    pub governor: GovernorView,
}

#[derive(Clone, Debug)]
pub struct LocalTradeView {
    pub system: SystemIdentityView,
    pub available: bool,
    pub unavailable_reason: Option<String>,
    pub market: Vec<MarketRow>,
}

/// Read-only availability of a market in the player-relative trade comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TradeDestinationAvailability {
    CurrentLocation,
    Available,
    Unreachable,
    Traveling,
}

/// Immutable market and route facts for one comparison destination.
///
/// This projection is always read-only. Actionable dockside trading remains
/// isolated in [`LocalTradeView`].
#[derive(Clone, Debug)]
pub struct TradeMarketComparisonView {
    pub system: SystemIdentityView,
    pub local: bool,
    pub read_only: bool,
    pub availability: TradeDestinationAvailability,
    pub unavailable_reason: Option<String>,
    pub route: Option<RouteView>,
    pub market: Vec<MarketRow>,
}

#[derive(Clone, Debug)]
pub struct EncyclopediaArticleView {
    pub title: String,
    pub paragraphs: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct EncyclopediaSectionView {
    pub title: String,
    pub articles: Vec<EncyclopediaArticleView>,
}

/// Frontend-independent factual manual content.
#[derive(Clone, Debug)]
pub struct EncyclopediaView {
    pub sections: Vec<EncyclopediaSectionView>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationEvent {
    pub sequence: u64,
    pub text: String,
}

impl PresentationEvent {
    #[must_use]
    pub fn contains(&self, pattern: &str) -> bool {
        self.text.contains(pattern)
    }

    #[must_use]
    pub fn starts_with(&self, pattern: &str) -> bool {
        self.text.starts_with(pattern)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FleetView {
    pub active_npcs: usize,
    pub normalized_unserved_opportunity: u64,
    pub opportunity_persistence: u32,
    pub total_spawns: u64,
    pub total_retirements: u64,
}

#[derive(Clone, Debug)]
pub struct ApplicationView {
    pub tick: u64,
    pub run_state: RunState,
    pub tick_rate: TickRate,
    pub systems: Vec<SystemListItem>,
    pub selected_system: ContentId,
    pub selected_route: Option<RouteView>,
    pub governed_system: Option<SystemIdentityView>,
    pub inspection: SystemInspectionView,
    pub local_trade: LocalTradeView,
    pub trade_markets: Vec<TradeMarketComparisonView>,
    pub encyclopedia: EncyclopediaView,
    pub dynamics: AggregateDynamicsView,
    pub player: PlayerStatusView,
    pub fleet: FleetView,
    pub events: Vec<PresentationEvent>,
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("application task has stopped")]
    Closed,
    #[error(transparent)]
    Core(#[from] CoreError),
}

struct Envelope {
    request: AppRequest,
    reply: oneshot::Sender<Result<(), CoreError>>,
}

pub struct AppHandle {
    requests: mpsc::Sender<Envelope>,
    pub views: watch::Receiver<ApplicationView>,
    task: JoinHandle<Result<(), CoreError>>,
}

impl AppHandle {
    pub async fn request(&self, request: AppRequest) -> Result<(), AppError> {
        let (reply, response) = oneshot::channel();
        self.requests
            .send(Envelope { request, reply })
            .await
            .map_err(|_| AppError::Closed)?;
        response.await.map_err(|_| AppError::Closed)??;
        Ok(())
    }

    pub async fn shutdown(self) -> Result<(), AppError> {
        let _ = self.request(AppRequest::Shutdown).await;
        self.task.await.map_err(|_| AppError::Closed)??;
        Ok(())
    }
}

pub fn spawn(mut session: GameSession) -> AppHandle {
    let snapshot = session.snapshot();
    let selected = snapshot
        .markets
        .first()
        .expect("definition has systems")
        .system_id
        .clone();
    let initial = build_view(
        &mut session,
        selected.clone(),
        RunState::Paused,
        TickRate::Normal,
        &VecDeque::new(),
    );
    let (request_tx, request_rx) = mpsc::channel(REQUEST_CAPACITY);
    let (view_tx, view_rx) = watch::channel(initial);
    let task = tokio::spawn(run_owner(session, selected, request_rx, view_tx));
    AppHandle {
        requests: request_tx,
        views: view_rx,
        task,
    }
}

async fn run_owner(
    mut session: GameSession,
    mut selected: ContentId,
    mut requests: mpsc::Receiver<Envelope>,
    views: watch::Sender<ApplicationView>,
) -> Result<(), CoreError> {
    let mut state = RunState::Paused;
    let mut rate = TickRate::Normal;
    let mut interval = tick_interval(rate);
    let mut history = VecDeque::new();
    let mut next_event_sequence = 1_u64;
    loop {
        tokio::select! {
            envelope = requests.recv() => {
                let Some(envelope) = envelope else { break; };
                let mut publish = true;
                let result = match envelope.request {
                    AppRequest::SetRunState(next) => { state = next; Ok(()) },
                    AppRequest::Step if state == RunState::Paused => session.step(),
                    AppRequest::Step => Ok(()),
                    AppRequest::SetTickRate(next) => { rate = next; interval = tick_interval(rate); Ok(()) },
                    AppRequest::SelectSystem(id) => { selected = id; Ok(()) },
                    AppRequest::Buy { good, quantity } => session.submit(GameCommand::Buy { good, quantity }),
                    AppRequest::Sell { good, quantity } => session.submit(GameCommand::Sell { good, quantity }),
                    AppRequest::BeginTravel { destination } => session.submit(GameCommand::BeginTravel { destination }),
                    AppRequest::CommitTrade { origin, destination, good, quantity } => session.submit(GameCommand::CommitTrade { origin, destination, good, quantity }),
                    AppRequest::DepositTank { amount } => session.submit(GameCommand::DepositTank { amount }),
                    AppRequest::WithdrawTank { amount } => session.submit(GameCommand::WithdrawTank { amount }),
                    AppRequest::SetMarketPolicy { system, policy } => session.submit(GameCommand::SetGovernorMarketPolicy { system, policy }),
                    AppRequest::SetInvestmentPolicy { system, policy } => session.submit(GameCommand::SetGovernorInvestmentPolicy { system, policy }),
                    AppRequest::CancelReservation => session.submit(GameCommand::CancelReservation),
                    AppRequest::Shutdown => { let _ = envelope.reply.send(Ok(())); break; }
                };
                if result.is_err() { publish = true; }
                collect_events(&mut session, &mut history, &mut next_event_sequence);
                if publish { views.send_replace(build_view(&mut session, selected.clone(), state, rate, &history)); }
                let _ = envelope.reply.send(result);
            }
            _ = interval.tick(), if state == RunState::Running => {
                session.step()?;
                collect_events(&mut session, &mut history, &mut next_event_sequence);
                views.send_replace(build_view(&mut session, selected.clone(), state, rate, &history));
            }
        }
    }
    Ok(())
}

fn tick_interval(rate: TickRate) -> tokio::time::Interval {
    let duration = rate.duration();
    let mut interval = tokio::time::interval_at(tokio::time::Instant::now() + duration, duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval
}

fn collect_events(
    session: &mut GameSession,
    history: &mut VecDeque<PresentationEvent>,
    next_sequence: &mut u64,
) {
    let events = session.drain_events();
    if events.is_empty() {
        return;
    }
    let snapshot = session.snapshot();
    let labels = EventLabels {
        systems: snapshot
            .markets
            .into_iter()
            .map(|market| (market.system_id, market.name))
            .collect(),
        traders: snapshot
            .traders
            .into_iter()
            .map(|trader| (trader.id, trader.name))
            .collect(),
        goods: session
            .catalog()
            .goods
            .iter()
            .map(|(id, good)| (id.clone(), good.name.clone()))
            .collect(),
        recipes: session
            .catalog()
            .recipes
            .iter()
            .map(|(id, recipe)| (id.clone(), recipe.name.clone()))
            .collect(),
    };
    for event in events {
        if history.len() == EVENT_HISTORY {
            history.pop_front();
        }
        let sequence = *next_sequence;
        *next_sequence = next_sequence
            .checked_add(1)
            .expect("presentation event sequence exhausted");
        history.push_back(PresentationEvent {
            sequence,
            text: format_event(&event, &labels),
        });
    }
}

fn build_view(
    session: &mut GameSession,
    selected: ContentId,
    run_state: RunState,
    tick_rate: TickRate,
    events: &VecDeque<PresentationEvent>,
) -> ApplicationView {
    let snapshot = session.snapshot();
    let player = snapshot
        .traders
        .iter()
        .find(|trader| trader.player)
        .expect("one player")
        .clone();
    let player_market = snapshot
        .markets
        .iter()
        .find(|market| market.system_id == player.system);
    let cargo_energy_value = player.cargo.iter().fold(0_i64, |total, (good, quantity)| {
        let value = player_market
            .and_then(|market| session.quotes(&market.system_id, good).ok())
            .map(|(buy, _)| {
                let quantity = i64::try_from(*quantity).unwrap_or(i64::MAX);
                buy.0.saturating_mul(quantity)
            })
            .unwrap_or(0);
        total.saturating_add(value)
    });
    let energy_values = snapshot
        .traders
        .iter()
        .map(|trader| {
            let cargo = trader.cargo.iter().fold(0_i64, |total, (good, quantity)| {
                let value = player_market
                    .and_then(|market| session.quotes(&market.system_id, good).ok())
                    .map(|(buy, _)| {
                        let quantity = i64::try_from(*quantity).unwrap_or(i64::MAX);
                        buy.0.saturating_mul(quantity)
                    })
                    .unwrap_or(0);
                total.saturating_add(value)
            });
            (
                trader.id.clone(),
                trader.energy_tank.0.saturating_add(cargo),
            )
        })
        .collect::<Vec<_>>();
    let total_energy_value = energy_values
        .iter()
        .fold(0_i64, |total, (_, worth)| total.saturating_add(*worth));
    let player_energy_value = player.energy_tank.0.saturating_add(cargo_energy_value);
    let rank = 1 + energy_values
        .iter()
        .filter(|(id, worth)| {
            *worth > player_energy_value || (*worth == player_energy_value && id < &player.id)
        })
        .count();
    let total_sales = snapshot.traders.iter().fold(0_i64, |total, trader| {
        total.saturating_add(trader.ledger.sales_revenue.0)
    });
    let system_names = snapshot
        .markets
        .iter()
        .map(|market| (market.system_id.clone(), market.name.clone()))
        .collect::<BTreeMap<_, _>>();
    let governed_system = snapshot.markets.iter().find_map(|market| {
        matches!(
            &market.governance.authority,
            MarketAuthority::Player(governor) if governor == &player.id
        )
        .then(|| SystemIdentityView {
            id: market.system_id.clone(),
            name: market.name.clone(),
        })
    });
    let systems = snapshot
        .markets
        .iter()
        .map(|market| {
            let route_from_player = session
                .shortest_path(&player.system, &market.system_id)
                .map(|(route, distance)| {
                    let ticks = route
                        .windows(2)
                        .map(|pair| {
                            session
                                .graph()
                                .neighbors(&pair[0])
                                .iter()
                                .find(|(id, _)| id == &pair[1])
                                .map_or(0, |(_, leg_distance)| {
                                    ticks_for_distance(*leg_distance, player.speed)
                                })
                        })
                        .sum();
                    (distance, ticks)
                });
            SystemListItem {
                id: market.system_id.clone(),
                name: market.name.clone(),
                coordinates: (market.position.x, market.position.y, market.position.z),
                player_location: market.system_id == player.system,
                player_governed: matches!(
                    &market.governance.authority,
                    MarketAuthority::Player(governor) if governor == &player.id
                ),
                route_distance_from_player: route_from_player.map(|(distance, _)| distance),
                route_ticks_from_player: route_from_player.map(|(_, ticks)| ticks),
                population: PopulationView {
                    current: market.population,
                    reference: market.population_state.reference,
                    carrying_capacity: market.population_state.carrying_capacity,
                    trend: market.population_state.trend,
                    tier: market.population_state.tier,
                    sufficiency_average_percent: market
                        .population_state
                        .sufficiency_average_percent,
                    sufficiency_trajectory: market
                        .population_state
                        .sufficiency_samples
                        .iter()
                        .copied()
                        .collect(),
                    settled_changes: market.population_state.settled_changes,
                },
                energy_stock: market.energy_stock,
                energy_capacity: market.energy_storage_cap,
                health: energy_health(market),
                brownout_stage: market.brownout.stage,
                runway_ticks: market.brownout.ticks_of_burn,
                seasonal_generation: SeasonalGenerationView {
                    base_output: market.seasonal_generation.base_output,
                    effective_output: market.seasonal_generation.current_effective_output,
                    phase_ticks: market.seasonal_phase.position_ticks,
                    period_ticks: market.seasonal_phase.period_ticks,
                    trend: market.seasonal_phase.trend,
                    ticks_until_turning_point: market.seasonal_phase.ticks_until_turning_point,
                    next_turning_point_tick: market.seasonal_phase.next_turning_point_tick,
                },
                connections: session
                    .graph()
                    .neighbors(&market.system_id)
                    .iter()
                    .map(|(system_id, distance)| ConnectionView {
                        system_id: system_id.clone(),
                        system_name: system_names
                            .get(system_id)
                            .cloned()
                            .unwrap_or_else(|| "Unknown system".into()),
                        distance: *distance,
                        travel_ticks: ticks_for_distance(*distance, player.speed),
                    })
                    .collect(),
            }
        })
        .collect();
    let selected_market = snapshot
        .markets
        .iter()
        .find(|market| market.system_id == selected)
        .unwrap_or(&snapshot.markets[0]);
    let inspection_market = build_market_rows(session, selected_market);
    let local_market = player_market
        .map(|market| build_market_rows(session, market))
        .unwrap_or_default();
    let trade_markets = snapshot
        .markets
        .iter()
        .map(|market| {
            let local = market.system_id == player.system;
            let route = (!local)
                .then(|| session.shortest_path(&player.system, &market.system_id))
                .flatten()
                .map(|(route, _)| {
                    build_route_view(
                        session,
                        &system_names,
                        &route,
                        player.speed,
                        player.travel_burn_per_distance,
                        None,
                        None,
                    )
                });
            let availability = if player.travel.is_some() {
                TradeDestinationAvailability::Traveling
            } else if local {
                TradeDestinationAvailability::CurrentLocation
            } else if route.is_some() {
                TradeDestinationAvailability::Available
            } else {
                TradeDestinationAvailability::Unreachable
            };
            let unavailable_reason = match availability {
                TradeDestinationAvailability::CurrentLocation => {
                    Some("Player is already at this system".into())
                }
                TradeDestinationAvailability::Available => None,
                TradeDestinationAvailability::Unreachable => {
                    Some("No route is available from the player location".into())
                }
                TradeDestinationAvailability::Traveling => {
                    Some("New travel is unavailable while the player is in transit".into())
                }
            };
            TradeMarketComparisonView {
                system: SystemIdentityView {
                    id: market.system_id.clone(),
                    name: market.name.clone(),
                },
                local,
                read_only: true,
                availability,
                unavailable_reason,
                route,
                market: build_market_rows(session, market),
            }
        })
        .collect::<Vec<_>>();
    let route = if let Some(travel) = &player.travel {
        Some(build_route_view(
            session,
            &system_names,
            &travel.route,
            player.speed,
            player.travel_burn_per_distance,
            Some(travel.next_leg.saturating_sub(1)),
            Some(travel.remaining_ticks),
        ))
    } else if selected != player.system {
        session
            .shortest_path(&player.system, &selected)
            .map(|(route, _)| {
                build_route_view(
                    session,
                    &system_names,
                    &route,
                    player.speed,
                    player.travel_burn_per_distance,
                    None,
                    None,
                )
            })
    } else {
        None
    };
    let runway_jumps = player_market.and_then(|market| {
        session
            .graph()
            .neighbors(&market.system_id)
            .iter()
            .map(|(_, distance)| travel_energy(*distance, player.travel_burn_per_distance))
            .collect::<Result<Vec<_>, _>>()
            .ok()
            .and_then(|costs| {
                costs
                    .into_iter()
                    .map(|cost| cost.0)
                    .filter(|cost| *cost > 0)
                    .min()
            })
            .and_then(|cost| u64::try_from(player.energy_tank.0 / cost).ok())
    });
    let burned = selected_market
        .energy_flow
        .life_support_burned
        .0
        .checked_add(selected_market.energy_flow.source_burned.0)
        .and_then(|value| value.checked_add(selected_market.energy_flow.production_burned.0))
        .and_then(|value| value.checked_add(selected_market.energy_flow.investment_burned.0))
        .and_then(|value| value.checked_add(selected_market.energy_flow.travel_burned.0))
        .expect("checked per-market flow ledger must remain reportable");
    let investments = [
        InvestmentKind::Collector,
        InvestmentKind::Storage,
        InvestmentKind::PopulationSupport,
        InvestmentKind::RouteSubsidy,
    ]
    .into_iter()
    .map(|kind| {
        let shape = &snapshot.investment_shapes[&kind];
        let level = selected_market
            .investment_state
            .levels
            .get(&kind)
            .copied()
            .unwrap_or(0);
        let next_cost = investment_cost(shape, level).ok();
        InvestmentView {
            kind,
            allocation_percent: selected_market
                .investment_policy
                .allocation_percent
                .get(&kind)
                .copied()
                .unwrap_or(0),
            level,
            maximum_level: shape.maximum_level,
            next_cost,
            cooldown_until: selected_market
                .investment_state
                .cooldown_until
                .get(&kind)
                .copied()
                .unwrap_or(0),
            status: selected_market
                .investment_state
                .status
                .get(&kind)
                .cloned()
                .unwrap_or_else(|| {
                    if shape.enabled {
                        next_cost.map_or(InvestmentStatus::MaximumLevel, |cost| {
                            InvestmentStatus::Ready { cost }
                        })
                    } else {
                        InvestmentStatus::Disabled
                    }
                }),
            effect_per_level: shape.effect_per_level,
        }
    })
    .collect::<Vec<_>>();
    let subsidy_level = selected_market
        .investment_state
        .levels
        .get(&InvestmentKind::RouteSubsidy)
        .copied()
        .unwrap_or(0);
    let subsidy_effect = snapshot.investment_shapes[&InvestmentKind::RouteSubsidy].effect_per_level;
    let route_subsidy_percent = subsidy_level
        .checked_mul(subsidy_effect)
        .expect("validated investment levels remain reportable");
    let inspection_governor = GovernorView {
        governed: matches!(
            &selected_market.governance.authority,
            MarketAuthority::Player(governor) if governor == &player.id
        ),
        policy: GovernorMarketPolicy {
            producer_margin_percent: selected_market.policy.producer_margin_percent,
            operating_reserve_ticks: selected_market.policy.operating_reserve_ticks,
            import_priorities: selected_market.policy.import_priorities.clone(),
        },
        investment_policy: GovernorInvestmentPolicy {
            allocation_percent: selected_market.investment_policy.allocation_percent.clone(),
        },
        investments,
        route_subsidy_percent,
        route_subsidy_active: route_subsidy_percent > 0
            && selected_market.brownout.stage < BrownoutStage::Emergency,
        ladder_occupancy_ticks: selected_market.brownout.occupancy_ticks,
        ladder_transitions: selected_market.brownout.transition_count,
        population_tier: selected_market.population_state.tier,
    };
    let local_trade_unavailable_reason = if player.travel.is_some() {
        Some("Trading is unavailable while traveling".to_owned())
    } else if player_market.is_none() {
        Some("No market is available at the player's location".to_owned())
    } else {
        None
    };
    let local_trade_system = player_market.map_or_else(
        || SystemIdentityView {
            id: player.system.clone(),
            name: "Unknown system".into(),
        },
        |market| SystemIdentityView {
            id: market.system_id.clone(),
            name: market.name.clone(),
        },
    );
    ApplicationView {
        tick: snapshot.tick,
        run_state,
        tick_rate,
        systems,
        selected_system: selected_market.system_id.clone(),
        selected_route: route,
        governed_system,
        inspection: SystemInspectionView {
            system: SystemIdentityView {
                id: selected_market.system_id.clone(),
                name: selected_market.name.clone(),
            },
            read_only_market: selected_market.system_id != player.system || player.travel.is_some(),
            market_energy: MarketEnergyView {
                stock: selected_market.energy_stock,
                capacity: selected_market.energy_storage_cap,
                reserved_claims: selected_market.reserved_energy,
                operating_reserve: selected_market.operating_reserve,
                protected_liquidation_budget: selected_market.protected_liquidation_budget,
                unreserved_purchasing_energy: selected_market.unreserved_energy_for_purchases,
                generated: selected_market.energy_flow.generated,
                burned: Energy(burned),
                curtailed: selected_market.energy_flow.curtailed,
                unsupplied_life_support: selected_market.energy_flow.life_support_unsupplied,
                bootstrap_risk_acknowledged: selected_market.bootstrap_risk_acknowledged,
                health: energy_health(selected_market),
                brownout_stage: selected_market.brownout.stage,
                runway_ticks: selected_market.brownout.ticks_of_burn,
                seasonal_generation: SeasonalGenerationView {
                    base_output: selected_market.seasonal_generation.base_output,
                    effective_output: selected_market.seasonal_generation.current_effective_output,
                    phase_ticks: selected_market.seasonal_phase.position_ticks,
                    period_ticks: selected_market.seasonal_phase.period_ticks,
                    trend: selected_market.seasonal_phase.trend,
                    ticks_until_turning_point: selected_market
                        .seasonal_phase
                        .ticks_until_turning_point,
                    next_turning_point_tick: selected_market.seasonal_phase.next_turning_point_tick,
                },
            },
            population: PopulationView {
                current: selected_market.population,
                reference: selected_market.population_state.reference,
                carrying_capacity: selected_market.population_state.carrying_capacity,
                trend: selected_market.population_state.trend,
                tier: selected_market.population_state.tier,
                sufficiency_average_percent: selected_market
                    .population_state
                    .sufficiency_average_percent,
                sufficiency_trajectory: selected_market
                    .population_state
                    .sufficiency_samples
                    .iter()
                    .copied()
                    .collect(),
                settled_changes: selected_market.population_state.settled_changes,
            },
            market: inspection_market,
            governor: inspection_governor,
        },
        local_trade: LocalTradeView {
            system: local_trade_system,
            available: local_trade_unavailable_reason.is_none(),
            unavailable_reason: local_trade_unavailable_reason,
            market: local_market,
        },
        trade_markets,
        encyclopedia: build_encyclopedia(
            &snapshot,
            session.catalog(),
            snapshot.player_trade_network_access,
        ),
        dynamics: AggregateDynamicsView {
            stage_occupancy_ticks: snapshot.dynamics_history.stage_occupancy_ticks,
            stage_transitions: snapshot.dynamics_history.stage_transitions,
            population_changes: snapshot.dynamics_history.population_changes,
            population_milestones: snapshot.dynamics_history.population_milestones,
        },
        player: PlayerStatusView {
            trade_network_access: snapshot.player_trade_network_access,
            location_name: system_names
                .get(&player.system)
                .cloned()
                .unwrap_or_else(|| "Unknown system".into()),
            location: player.system,
            tank_energy: player.energy_tank,
            tank_capacity: player.energy_tank_capacity,
            bay_energy: player
                .cargo
                .get(&ContentId::new(ENERGY_ID).expect("constant energy id"))
                .copied()
                .unwrap_or(0),
            cargo: player
                .cargo
                .iter()
                .map(|(good_id, quantity)| CargoItemView {
                    good_id: good_id.clone(),
                    good_name: session
                        .catalog()
                        .goods
                        .get(good_id)
                        .map_or_else(|| "Unknown good".into(), |good| good.name.clone()),
                    quantity: *quantity,
                })
                .collect(),
            cargo_used: player
                .cargo
                .values()
                .fold(0_u64, |total, quantity| total.saturating_add(*quantity)),
            cargo_capacity: player.cargo_capacity,
            cargo_energy_value: Energy(cargo_energy_value),
            total_energy_value: Energy(player_energy_value),
            purchase_energy: player.ledger.purchase_cost,
            sales_energy: player.ledger.sales_revenue,
            realized_energy_gain: Energy(
                player
                    .ledger
                    .sales_revenue
                    .0
                    .saturating_sub(player.ledger.purchase_cost.0),
            ),
            units_moved: player.ledger.cargo_units_moved,
            transactions: player.ledger.completed_transactions,
            energy_value_rank: rank,
            energy_value_share_percent: if total_energy_value == 0 {
                0.0
            } else {
                player_energy_value as f64 * 100.0 / total_energy_value as f64
            },
            sales_share_percent: if total_sales == 0 {
                0.0
            } else {
                player.ledger.sales_revenue.0 as f64 * 100.0 / total_sales as f64
            },
            runway_jumps,
            traveling: player.travel.is_some(),
        },
        fleet: FleetView {
            active_npcs: snapshot
                .traders
                .iter()
                .filter(|trader| !trader.player)
                .count(),
            normalized_unserved_opportunity: snapshot.fleet.normalized_unserved_opportunity,
            opportunity_persistence: snapshot.fleet.opportunity_persistence,
            total_spawns: snapshot.dynamics_history.fleet_spawns,
            total_retirements: snapshot.dynamics_history.fleet_retirements,
        },
        events: events.iter().cloned().collect(),
    }
}

fn build_market_rows(
    session: &mut GameSession,
    market: &game_core::MarketSnapshot,
) -> Vec<MarketRow> {
    let goods = session
        .catalog()
        .goods
        .values()
        .map(|good| (good.id.clone(), good.name.clone()))
        .collect::<Vec<_>>();
    goods
        .into_iter()
        .map(|(id, name)| {
            let (buy_quote, sell_quote) = session
                .quotes(&market.system_id, &id)
                .unwrap_or((Energy::ZERO, Energy::ZERO));
            MarketRow {
                inventory: market.inventory.get(&id).copied().unwrap_or(0),
                target: market.targets.get(&id).copied().unwrap_or(0),
                unit_cost: market
                    .cost_basis
                    .get(&id)
                    .and_then(|basis| basis.unit_cost_ceil().ok())
                    .unwrap_or(Energy::ZERO),
                funded_demand: u64::from(
                    market.demand.get(&id).copied().unwrap_or_default().funded,
                ),
                good_id: id,
                name,
                buy_quote,
                sell_quote,
            }
        })
        .collect()
}

fn build_encyclopedia(
    snapshot: &game_core::CoreSnapshot,
    catalog: &game_core::Catalog,
    trade_network_access: TradeNetworkAccess,
) -> EncyclopediaView {
    let goods = catalog
        .goods
        .values()
        .map(|good| {
            let category = match good.category {
                game_core::GoodCategory::Energy => "Energy",
                game_core::GoodCategory::Raw => "Raw",
                game_core::GoodCategory::Primary => "Primary",
                game_core::GoodCategory::Secondary => "Secondary",
            };
            format!(
                "{} — {category} good; bootstrap embodied-energy cost {} E per unit. Bootstrap cost seeds cost basis and is not a current market quote.",
                good.name, good.bootstrap_cost.0
            )
        })
        .collect::<Vec<_>>();
    let recipe_articles = catalog
        .recipes
        .values()
        .map(|recipe| {
            let amounts = recipe
                .inputs
                .iter()
                .map(|input| format!("{} {}", input.quantity, good_name(catalog, &input.good)))
                .collect::<Vec<_>>();
            let outputs = recipe
                .outputs
                .iter()
                .map(|output| format!("{} {}", output.quantity, good_name(catalog, &output.good)))
                .collect::<Vec<_>>();
            let layer = match recipe.layer {
                game_core::RecipeLayer::Primary => "Primary",
                game_core::RecipeLayer::Secondary => "Secondary",
                game_core::RecipeLayer::Tertiary => "Tertiary",
            };
            EncyclopediaArticleView {
                title: recipe.name.clone(),
                paragraphs: vec![
                    format!(
                        "Layer: {layer}. Operating energy: {} E.",
                        recipe.operating_energy.0
                    ),
                    format!(
                        "Inputs: {}.",
                        if amounts.is_empty() {
                            "none".into()
                        } else {
                            amounts.join(", ")
                        }
                    ),
                    format!(
                        "Outputs: {}.",
                        if outputs.is_empty() {
                            "none; the process consumes its inputs".into()
                        } else {
                            outputs.join(", ")
                        }
                    ),
                ],
            }
        })
        .collect::<Vec<_>>();
    let investment_facts = snapshot
        .investment_shapes
        .iter()
        .map(|(kind, shape)| {
            let (name, effect) = match kind {
                InvestmentKind::Collector => (
                    "Collector",
                    format!("adds {} E to base generation per level", shape.effect_per_level),
                ),
                InvestmentKind::Storage => (
                    "Storage",
                    format!("adds {} E of market storage per level", shape.effect_per_level),
                ),
                InvestmentKind::PopulationSupport => (
                    "Population Support",
                    format!(
                        "adds {} to support capacity and {} percentage points to the gated growth-rate bonus per level",
                        shape.effect_per_level, shape.effect_per_level
                    ),
                ),
                InvestmentKind::RouteSubsidy => (
                    "Route Subsidy",
                    format!("adds a {}% funded import premium per level", shape.effect_per_level),
                ),
            };
            format!(
                "{name} — {}; base cost {} E; cost growth {}%; maximum level {}; cooldown {} ticks; {effect}.",
                if shape.enabled { "enabled" } else { "disabled" },
                shape.base_cost.0,
                shape.cost_growth_percent,
                shape.maximum_level,
                shape.cooldown_ticks
            )
        })
        .collect::<Vec<_>>();
    let (access_name, access_fact) = match trade_network_access {
        TradeNetworkAccess::Offline => (
            "Offline",
            "The player cannot create reservation-backed CommitTrade commitments. Read-only remote market facts, dockside transactions, and direct travel are separate capabilities.",
        ),
        TradeNetworkAccess::ReservationContracts => (
            "Reservation Contracts",
            "An accepted CommitTrade commitment atomically buys, reserves destination funding, and departs. The current TUI does not expose CommitTrade yet.",
        ),
    };

    EncyclopediaView {
        sections: vec![
            EncyclopediaSectionView {
                title: "Worlds & Population".into(),
                articles: vec![
                    EncyclopediaArticleView {
                        title: "Systems and Energy".into(),
                        paragraphs: vec![
                            format!(
                                "The current frontier contains {} systems. Systems have fixed positions and a connected route graph; route distance determines travel ticks and tank-energy burn.",
                                snapshot.markets.len()
                            ),
                            "Each market has one physical Energy stock and a storage capacity. Generation enters that stock; overflow is curtailed. Life support, production, investments, and market settlements use that same stock under their respective reserve rules.".into(),
                            "Market Energy, trader-tank Energy, and cargo-bay Energy are separate physical stores. Tank Energy pays for local purchases and travel. Cargo-bay Energy occupies cargo capacity and is neither spendable nor burned during travel.".into(),
                        ],
                    },
                    EncyclopediaArticleView {
                        title: "Brownouts".into(),
                        paragraphs: vec![
                            "Brownout state is an ordered ladder: Normal, Throttled, Emergency, and Starvation. Entry and recovery use different runway thresholds and a minimum stage duration, so a stage does not mirror a single stock sample instantly.".into(),
                            "Throttled reduces industrial throughput. Emergency and Starvation suppress non-survival demand and disable investment spending; configured policy remains stored. Starvation also permits population decline after sufficiency is sampled.".into(),
                            "Runway is market Energy stock divided by the mandatory life-support Energy obligation for one tick: population multiplied by per-capita life-support burn. It is not a measure of general or current burn.".into(),
                        ],
                    },
                    EncyclopediaArticleView {
                        title: "Population".into(),
                        paragraphs: vec![
                            "Population is integer market state. Current population determines life-support demand, labor throughput, and authored tertiary demand on the following tick.".into(),
                            "A bounded rolling sufficiency history combines Energy and authored essential-goods supply. Starvation can cause comparatively fast decline. Growth requires sustained sufficient supply under Normal conditions and follows gated logistic growth toward a history-supported carrying capacity.".into(),
                            "Population has a reference level, carrying capacity, trend, and tier. Fractional growth and decline are retained as deterministic remainders rather than discarded.".into(),
                        ],
                    },
                ],
            },
            EncyclopediaSectionView {
                title: "Goods & Markets".into(),
                articles: vec![
                    EncyclopediaArticleView {
                        title: "Goods".into(),
                        paragraphs: goods,
                    },
                    EncyclopediaArticleView {
                        title: "Markets and Quotes".into(),
                        paragraphs: vec![
                            "Every system market records stock, target, funded demand, embodied unit cost, a market-buy quote, and a market-sell quote for each catalog good.".into(),
                            "The market-buy quote is the Energy paid by the market for one unit sold into it. The market-sell quote is the Energy charged by the market for one unit bought from it. Quotes can differ by system because stock, target, cost basis, policy, funding, and brownout state differ.".into(),
                            "Market purchasing power is physical Energy stock after reservation claims, operating reserve, and protected liquidation budget. A reservation claim is not a second physical stockpile.".into(),
                        ],
                    },
                ],
            },
            EncyclopediaSectionView {
                title: "Recipes".into(),
                articles: if recipe_articles.is_empty() {
                    vec![EncyclopediaArticleView {
                        title: "Recipes".into(),
                        paragraphs: vec!["The current catalog defines no production recipes.".into()],
                    }]
                } else {
                    recipe_articles
                },
            },
            EncyclopediaSectionView {
                title: "Governance & Trade".into(),
                articles: vec![
                    EncyclopediaArticleView {
                        title: "Governance Policies".into(),
                        paragraphs: vec![
                            "A market is governed either by a player authority or autonomously. Player authority permits replacement of the exposed producer margin, operating-reserve horizon, import priorities, and investment allocation. Other pricing, liquidation, and target rules remain core-owned.".into(),
                            "Producer margin contributes to sustainable asks. Operating-reserve ticks protect Energy needed for operation. Import priorities weight normal import bids relative to asks, subject to processor solvency and available destination funding; they do not scale target deficit or configured demand quantity. Investment allocations divide at most 100 percent among the four investment kinds.".into(),
                        ],
                    },
                    EncyclopediaArticleView {
                        title: "Investments".into(),
                        paragraphs: investment_facts,
                    },
                    EncyclopediaArticleView {
                        title: "Traders, Reservations, and Travel".into(),
                        paragraphs: vec![
                            "Traders have a location, Energy tank and capacity, cargo bay and capacity, speed, travel-burn rate, refuel policy, ledger, and optional travel plan and reservation.".into(),
                            "An accepted reservation-backed commitment buys at its origin, claims funded Energy at its destination without creating a second stockpile, and departs as one atomic mutation. Claims refresh in transit and are released on cancellation, expiry, or settlement.".into(),
                            "Travel follows a route of connected systems. Its displayed required Energy is calculated for the full route from distance and the trader burn rate. Departure burns tank Energy; arrival and reservation settlement occur in later simulation phases.".into(),
                        ],
                    },
                    EncyclopediaArticleView {
                        title: "Trade Network Access".into(),
                        paragraphs: vec![
                            format!("Current player access: {access_name}."),
                            access_fact.into(),
                            "NPC commitment planning is internal simulation behavior and does not use the player's access capability.".into(),
                        ],
                    },
                ],
            },
        ],
    }
}

fn good_name(catalog: &game_core::Catalog, id: &ContentId) -> String {
    catalog
        .goods
        .get(id)
        .map_or_else(|| "Unknown good".into(), |good| good.name.clone())
}

fn energy_health(market: &game_core::MarketSnapshot) -> EnergyHealth {
    if market.energy_flow.life_support_unsupplied.0 > 0 || market.energy_stock.0 == 0 {
        EnergyHealth::Deficit
    } else if market.energy_stock >= market.energy_storage_cap {
        EnergyHealth::Full
    } else if market.energy_stock.0
        <= market
            .operating_reserve
            .0
            .saturating_add(market.protected_liquidation_budget.0)
            .saturating_add(market.reserved_energy.0)
    {
        EnergyHealth::Low
    } else {
        EnergyHealth::Healthy
    }
}

fn build_route_view(
    session: &GameSession,
    system_names: &BTreeMap<ContentId, String>,
    route: &[ContentId],
    speed: f64,
    travel_burn_per_distance: Energy,
    current_leg: Option<usize>,
    current_leg_remaining: Option<u32>,
) -> RouteView {
    let legs = route
        .windows(2)
        .map(|pair| {
            let distance = session
                .graph()
                .neighbors(&pair[0])
                .iter()
                .find(|(id, _)| id == &pair[1])
                .map_or(0.0, |(_, distance)| *distance);
            RouteLegView {
                from_id: pair[0].clone(),
                from_name: system_names
                    .get(&pair[0])
                    .cloned()
                    .unwrap_or_else(|| "Unknown system".into()),
                to_id: pair[1].clone(),
                to_name: system_names
                    .get(&pair[1])
                    .cloned()
                    .unwrap_or_else(|| "Unknown system".into()),
                distance,
                travel_ticks: ticks_for_distance(distance, speed),
            }
        })
        .collect::<Vec<_>>();
    let total_ticks = legs
        .iter()
        .fold(0_u32, |total, leg| total.saturating_add(leg.travel_ticks));
    let remaining_ticks = current_leg.map(|index| {
        legs.iter()
            .skip(index + 1)
            .fold(current_leg_remaining.unwrap_or(0), |total, leg| {
                total.saturating_add(leg.travel_ticks)
            })
    });
    let destination_id = route.last().cloned().expect("routes contain a destination");
    let required_energy = route_travel_energy(session.graph(), route, travel_burn_per_distance)
        .unwrap_or(Energy(i64::MAX));
    RouteView {
        destination_name: system_names
            .get(&destination_id)
            .cloned()
            .unwrap_or_else(|| "Unknown system".into()),
        destination_id,
        total_distance: legs.iter().map(|leg| leg.distance).sum(),
        total_ticks,
        remaining_ticks,
        required_energy,
        current_leg,
        legs,
    }
}

struct EventLabels {
    systems: BTreeMap<ContentId, String>,
    traders: BTreeMap<ContentId, String>,
    goods: BTreeMap<ContentId, String>,
    recipes: BTreeMap<ContentId, String>,
}

impl EventLabels {
    fn system(&self, id: &ContentId) -> &str {
        self.systems
            .get(id)
            .map_or("Unknown system", String::as_str)
    }

    fn trader(&self, id: &ContentId) -> &str {
        self.traders
            .get(id)
            .map_or("Unknown trader", String::as_str)
    }

    fn good(&self, id: &ContentId) -> &str {
        self.goods.get(id).map_or("Unknown good", String::as_str)
    }

    fn recipe(&self, id: &ContentId) -> &str {
        self.recipes
            .get(id)
            .map_or("Unknown process", String::as_str)
    }
}

fn format_event(event: &GameEvent, labels: &EventLabels) -> String {
    match event {
        GameEvent::TickAdvanced(tick) => format!("Tick {tick}"),
        GameEvent::EnergyGenerated {
            system,
            amount,
            curtailed,
        } => format!(
            "{} generated {} energy ({} curtailed)",
            labels.system(system),
            amount.0,
            curtailed.0
        ),
        GameEvent::LifeSupport {
            system,
            burned,
            unsupplied,
        } => format!(
            "{} life support burned {} energy ({} unsupplied)",
            labels.system(system),
            burned.0,
            unsupplied.0
        ),
        GameEvent::ExternalDeliveryRecorded {
            system,
            good,
            quantity,
            energy_inflow,
            tick,
        } => format!(
            "Recorded external delivery of {quantity} {} to {} at tick {tick} ({} energy inflow)",
            labels.good(good),
            labels.system(system),
            energy_inflow.0
        ),
        GameEvent::BrownoutTransition {
            system,
            from,
            to,
            ticks_of_burn,
            tick,
        } => format!(
            "{} brownout stage {} → {} at tick {} ({} ticks runway)",
            labels.system(system),
            from.label(),
            to.label(),
            tick,
            ticks_of_burn
        ),
        GameEvent::PopulationChanged { system, from, to } => format!(
            "{} population changed from {} to {}",
            labels.system(system),
            from,
            to
        ),
        GameEvent::PopulationTierChanged {
            system,
            from,
            to,
            population,
        } => format!(
            "{} population milestone tier {} → {} at {}",
            labels.system(system),
            from,
            to,
            population
        ),
        GameEvent::TraderSpawned { trader, system } => format!(
            "{} entered service at {}",
            labels.trader(trader),
            labels.system(system)
        ),
        GameEvent::TraderRetired { trader, system } => format!(
            "{} retired at {}",
            labels.trader(trader),
            labels.system(system)
        ),
        GameEvent::InvestmentCompleted {
            system,
            kind,
            level,
            cost,
        } => format!(
            "{} completed {kind:?} investment level {level} for {} energy",
            labels.system(system),
            cost.0
        ),
        GameEvent::InvestmentDeferred {
            system,
            kind,
            reason,
        } => format!(
            "{} deferred {kind:?} investment: {reason}",
            labels.system(system)
        ),
        GameEvent::GovernorPolicyRejected { system, reason } => format!(
            "{} governor policy rejected: {}",
            labels.system(system),
            reason.label()
        ),
        GameEvent::Produced { system, recipe } => format!(
            "{}: completed {}",
            labels.system(system),
            labels.recipe(recipe)
        ),
        GameEvent::Bought {
            trader,
            good,
            quantity,
            total,
        } => format!(
            "{} bought {quantity} {} for {} energy",
            labels.trader(trader),
            labels.good(good),
            total.0
        ),
        GameEvent::Sold {
            trader,
            good,
            quantity,
            total,
            partial,
        } => format!(
            "{} sold {quantity} {} for {} energy{}",
            labels.trader(trader),
            labels.good(good),
            total.0,
            if *partial {
                " (partial settlement)"
            } else {
                ""
            }
        ),
        GameEvent::ReservationCreated {
            reservation,
            trader,
            destination,
            good,
            quantity,
            reserved_energy,
        } => format!(
            "Reservation {reservation}: {} committed {quantity} {} to {} ({} energy claimed)",
            labels.trader(trader),
            labels.good(good),
            labels.system(destination),
            reserved_energy.0
        ),
        GameEvent::ReservationReleased {
            reservation,
            status,
            released_energy,
        } => format!(
            "Reservation {reservation} {} ({} energy claim released)",
            reservation_status(*status),
            released_energy.0
        ),
        GameEvent::SaleDeferred {
            trader,
            good,
            reason,
        } => format!(
            "{} deferred sale of {}: {reason}",
            labels.trader(trader),
            labels.good(good)
        ),
        GameEvent::Departed {
            trader,
            destination,
            travel_burn,
        } => format!(
            "{} departed for {} ({} tank energy committed)",
            labels.trader(trader),
            labels.system(destination),
            travel_burn.0
        ),
        GameEvent::Arrived { trader, system } => format!(
            "{} arrived at {}",
            labels.trader(trader),
            labels.system(system)
        ),
        GameEvent::PolicyChanged { system } => {
            format!("{} market pricing policy changed", labels.system(system))
        }
        GameEvent::Rejected(reason) => format!("Rejected: {reason}"),
    }
}

fn reservation_status(status: ReservationStatus) -> &'static str {
    match status {
        ReservationStatus::Active => "active",
        ReservationStatus::Fulfilled => "fulfilled",
        ReservationStatus::Cancelled => "cancelled",
        ReservationStatus::Expired => "expired",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{
        EconomyConfig, FleetDynamics, FleetMode, GameDefinition, GoodAmount, GoodCategory,
        GoodDefinition, Governance, InvestmentPolicy, MarketAuthority, MarketPolicy,
        PopulationState, Position3, RecipeDefinition, RecipeLayer, RecipeOutput, RefuelPolicy,
        SeasonalGenerationState, SourceDefinition, SystemDefinition, TraderDefinition,
    };

    fn id(value: &str) -> ContentId {
        ContentId::new(value).unwrap()
    }

    fn definition() -> GameDefinition {
        let goods = vec![
            GoodDefinition {
                id: id(ENERGY_ID),
                name: "Energy".into(),
                category: GoodCategory::Energy,
                bootstrap_cost: Energy(1),
            },
            GoodDefinition {
                id: id("core:ore"),
                name: "Ore".into(),
                category: GoodCategory::Raw,
                bootstrap_cost: Energy(10),
            },
        ];
        let systems = (0..2)
            .map(|i| SystemDefinition {
                id: id(&format!("core:s{i}")),
                name: format!("S{i}"),
                position: Position3 {
                    x: f64::from(i),
                    y: 0.0,
                    z: 0.0,
                },
                inventory: BTreeMap::from([(id(ENERGY_ID), 1_000), (id("core:ore"), 10)]),
                targets: BTreeMap::from([(id("core:ore"), 10)]),
                recipes: vec![],
                sources: Vec::<SourceDefinition>::new(),
                energy_output_per_tick: Energy(10),
                seasonal_generation: SeasonalGenerationState {
                    base_output: Energy(10),
                    amplitude_percent: 0,
                    period_ticks: 100,
                    phase_ticks: 0,
                    current_effective_output: Energy(10),
                },
                energy_storage_cap: Energy(2_000),
                population: 1,
                population_state: PopulationState {
                    current: 1,
                    reference: 1,
                    carrying_capacity: 1,
                    ..PopulationState::default()
                },
                investment_policy: InvestmentPolicy::default(),
                governance: if i == 0 {
                    Governance {
                        authority: MarketAuthority::Player(id("core:player")),
                    }
                } else {
                    Governance::default()
                },
                policy: MarketPolicy::default(),
                protected_liquidation_budget: Energy(10),
                bootstrap_risk_acknowledged: false,
            })
            .collect();
        let traders = vec![TraderDefinition {
            id: id("core:player"),
            name: "Player".into(),
            system: id("core:s0"),
            energy_tank: Energy(100),
            energy_tank_capacity: Energy(1_000),
            cargo_capacity: 10,
            speed: 1.0,
            travel_burn_per_distance: Energy(1),
            refuel_policy: RefuelPolicy::DepositAndWithdraw,
            player: true,
        }];
        GameDefinition {
            goods,
            recipes: vec![],
            systems,
            traders,
            player_trade_network_access: TradeNetworkAccess::Offline,
            fleet: FleetDynamics {
                mode: Some(FleetMode::Fixed { count: 0 }),
                ..FleetDynamics::default()
            },
            economy: EconomyConfig::default(),
        }
    }

    #[test]
    fn event_formatter_resolves_all_player_facing_labels() {
        let labels = EventLabels {
            systems: BTreeMap::from([(id("core:s0"), "Aster Reach".into())]),
            traders: BTreeMap::from([(id("core:player"), "Free Trader".into())]),
            goods: BTreeMap::from([(id("core:ore"), "Ferrite Ore".into())]),
            recipes: BTreeMap::from([(id("core:smelt"), "Alloy Smelting".into())]),
        };
        let events = [
            GameEvent::Produced {
                system: id("core:s0"),
                recipe: id("core:smelt"),
            },
            GameEvent::Bought {
                trader: id("core:player"),
                good: id("core:ore"),
                quantity: 2,
                total: Energy(10),
            },
            GameEvent::Sold {
                trader: id("core:player"),
                good: id("core:ore"),
                quantity: 2,
                total: Energy(12),
                partial: false,
            },
            GameEvent::Departed {
                trader: id("core:player"),
                destination: id("core:s0"),
                travel_burn: Energy(1),
            },
            GameEvent::Arrived {
                trader: id("core:player"),
                system: id("core:s0"),
            },
            GameEvent::BrownoutTransition {
                system: id("core:s0"),
                from: BrownoutStage::Normal,
                to: BrownoutStage::Emergency,
                ticks_of_burn: 4,
                tick: 7,
            },
        ];
        let rendered = events
            .iter()
            .map(|event| format_event(event, &labels))
            .collect::<Vec<_>>();
        assert!(
            rendered
                .iter()
                .any(|event| event.contains("Aster Reach: completed Alloy Smelting"))
        );
        assert!(
            rendered
                .iter()
                .any(|event| event.contains("Free Trader bought 2 Ferrite Ore"))
        );
        assert!(
            rendered
                .iter()
                .any(|event| event == "Free Trader arrived at Aster Reach")
        );
        assert!(rendered.iter().any(|event| {
            event == "Aster Reach brownout stage Normal → Emergency at tick 7 (4 ticks runway)"
        }));
        assert!(rendered.iter().all(|event| !event.contains("core:")));
    }

    #[tokio::test]
    async fn paused_step_advances_once() {
        let session = GameSession::new(definition()).unwrap();
        let handle = spawn(session);
        handle.request(AppRequest::Step).await.unwrap();
        let view = handle.views.borrow().clone();
        assert_eq!(view.tick, 1);
        handle.shutdown().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn paused_and_running_timers_follow_selected_rates() {
        for rate in [TickRate::Slow, TickRate::Normal, TickRate::Fast] {
            let session = GameSession::new(definition()).unwrap();
            let mut handle = spawn(session);
            tokio::time::advance(rate.duration() * 2).await;
            tokio::task::yield_now().await;
            assert_eq!(
                handle.views.borrow().tick,
                0,
                "paused sessions must not tick"
            );

            handle.request(AppRequest::SetTickRate(rate)).await.unwrap();
            handle
                .request(AppRequest::SetRunState(RunState::Running))
                .await
                .unwrap();
            handle.views.borrow_and_update();
            tokio::time::advance(rate.duration()).await;
            handle.views.changed().await.unwrap();
            assert_eq!(handle.views.borrow_and_update().tick, 1, "rate {rate:?}");
            handle.shutdown().await.unwrap();
        }
    }

    #[tokio::test(start_paused = true)]
    async fn changing_rate_does_not_emit_an_immediate_duplicate_tick() {
        let session = GameSession::new(definition()).unwrap();
        let mut handle = spawn(session);
        handle
            .request(AppRequest::SetRunState(RunState::Running))
            .await
            .unwrap();
        handle
            .request(AppRequest::SetTickRate(TickRate::Fast))
            .await
            .unwrap();
        handle.views.borrow_and_update();
        tokio::time::advance(TickRate::Fast.duration() - Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(handle.views.borrow().tick, 0);
        tokio::time::advance(Duration::from_millis(1)).await;
        handle.views.changed().await.unwrap();
        assert_eq!(handle.views.borrow_and_update().tick, 1);
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn views_project_market_route_and_player_statistics() {
        let session = GameSession::new(definition()).unwrap();
        let handle = spawn(session);
        let initial = handle.views.borrow().clone();
        assert_eq!(initial.systems.len(), 2);
        assert_eq!(initial.systems[0].connections[0].system_name, "S1");
        assert_eq!(initial.inspection.market.len(), 2);
        assert_eq!(initial.player.energy_value_rank, 1);
        assert_eq!(initial.player.energy_value_share_percent, 100.0);
        assert_eq!(
            initial.inspection.market_energy.health,
            EnergyHealth::Healthy
        );
        assert_eq!(initial.inspection.population.current, 1);
        assert_eq!(initial.inspection.population.carrying_capacity, 1);
        assert_eq!(initial.inspection.population.trend, PopulationTrend::Stable);
        assert_eq!(initial.systems[0].population, initial.inspection.population);
        assert_eq!(initial.dynamics.population_changes, 0);
        assert_eq!(initial.dynamics.stage_occupancy_ticks, [0; 4]);

        handle
            .request(AppRequest::Buy {
                good: id("core:ore"),
                quantity: 1,
            })
            .await
            .unwrap();
        let bought = handle.views.borrow().clone();
        assert_eq!(bought.player.cargo_used, 1);
        assert_eq!(bought.player.cargo[0].good_name, "Ore");
        assert_eq!(bought.player.transactions, 1);
        assert!(bought.player.total_energy_value.0 > 0);
        assert!(
            bought
                .events
                .iter()
                .any(|event| event.contains("Player bought 1 Ore"))
        );
        assert!(bought.events.iter().all(|event| !event.contains("core:")));
        handle
            .request(AppRequest::SelectSystem(id("core:s1")))
            .await
            .unwrap();
        let selected = handle.views.borrow().clone();
        let route = selected.selected_route.expect("route preview");
        assert_eq!(route.destination_name, "S1");
        assert_eq!(route.legs[0].from_name, "S0");
        assert_eq!(route.legs[0].to_name, "S1");
        assert_eq!(route.current_leg, None);
        assert_eq!(route.required_energy, Energy(1));
        handle
            .request(AppRequest::BeginTravel {
                destination: id("core:s1"),
            })
            .await
            .unwrap();
        let traveling = handle.views.borrow().clone();
        assert!(
            traveling
                .events
                .iter()
                .any(|event| event.starts_with("Player departed for S1"))
        );
        assert!(
            traveling
                .events
                .iter()
                .all(|event| !event.contains("core:"))
        );
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn trade_comparison_is_remote_read_only_and_does_not_replace_local_action_data() {
        let mut definition = definition();
        definition.systems[1].inventory.insert(id("core:ore"), 2);
        definition.systems[1].targets.insert(id("core:ore"), 25);
        let handle = spawn(GameSession::new(definition).unwrap());
        let initial = handle.views.borrow().clone();
        let local = initial
            .local_trade
            .market
            .iter()
            .find(|row| row.good_id == id("core:ore"))
            .unwrap();
        assert_eq!((local.inventory, local.target), (10, 10));

        let remote = initial
            .trade_markets
            .iter()
            .find(|market| market.system.id == id("core:s1"))
            .unwrap();
        let remote_ore = remote
            .market
            .iter()
            .find(|row| row.good_id == id("core:ore"))
            .unwrap();
        assert!(remote.read_only);
        assert!(!remote.local);
        assert_eq!((remote_ore.inventory, remote_ore.target), (2, 25));
        assert_ne!(remote_ore.sell_quote, local.sell_quote);
        assert_eq!(remote.availability, TradeDestinationAvailability::Available);
        assert_eq!(remote.route.as_ref().unwrap().required_energy, Energy(1));

        handle
            .request(AppRequest::SelectSystem(id("core:s1")))
            .await
            .unwrap();
        let selected = handle.views.borrow().clone();
        let still_local = selected
            .local_trade
            .market
            .iter()
            .find(|row| row.good_id == id("core:ore"))
            .unwrap();
        assert_eq!((still_local.inventory, still_local.target), (10, 10));
        assert!(!selected.player.traveling);

        handle
            .request(AppRequest::BeginTravel {
                destination: id("core:s1"),
            })
            .await
            .unwrap();
        assert!(
            handle
                .views
                .borrow()
                .trade_markets
                .iter()
                .all(|market| market.availability == TradeDestinationAvailability::Traveling)
        );
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn encyclopedia_resolves_catalog_recipe_and_access_facts() {
        let mut definition = definition();
        definition.recipes.push(RecipeDefinition {
            id: id("core:smelt"),
            name: "Ore Smelting".into(),
            layer: RecipeLayer::Primary,
            inputs: vec![GoodAmount {
                good: id("core:ore"),
                quantity: 3,
            }],
            outputs: vec![RecipeOutput {
                good: id(ENERGY_ID),
                quantity: 2,
                cost_weight: 1,
            }],
            operating_energy: Energy(6),
            margin_percent: None,
        });
        definition.player_trade_network_access = TradeNetworkAccess::ReservationContracts;
        let handle = spawn(GameSession::new(definition).unwrap());
        let view = handle.views.borrow().clone();
        let articles = view
            .encyclopedia
            .sections
            .iter()
            .flat_map(|section| &section.articles)
            .collect::<Vec<_>>();
        let recipe = articles
            .iter()
            .find(|article| article.title == "Ore Smelting")
            .unwrap();
        let recipe_text = recipe.paragraphs.join(" ");
        assert!(recipe_text.contains("3 Ore"));
        assert!(recipe_text.contains("2 Energy"));
        assert!(!recipe_text.contains("core:"));
        let access = articles
            .iter()
            .find(|article| article.title == "Trade Network Access")
            .unwrap()
            .paragraphs
            .join(" ");
        assert!(access.contains("Current player access: Reservation Contracts"));
        assert!(access.contains("atomically buys, reserves destination funding, and departs"));
        assert!(access.contains("current TUI does not expose CommitTrade yet"));

        let brownouts = articles
            .iter()
            .find(|article| article.title == "Brownouts")
            .unwrap()
            .paragraphs
            .join(" ");
        assert!(brownouts.contains(
            "market Energy stock divided by the mandatory life-support Energy obligation"
        ));
        assert!(brownouts.contains("not a measure of general or current burn"));

        let governance = articles
            .iter()
            .find(|article| article.title == "Governance Policies")
            .unwrap()
            .paragraphs
            .join(" ");
        assert!(governance.contains("weight normal import bids relative to asks"));
        assert!(
            governance.contains("subject to processor solvency and available destination funding")
        );
        assert!(governance.contains("do not scale target deficit or configured demand quantity"));
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn views_project_seasonal_base_effective_phase_and_turning_point() {
        let mut definition = definition();
        definition.systems[0].seasonal_generation.amplitude_percent = 20;
        definition.systems[0].seasonal_generation.period_ticks = 4;
        definition.systems[0].seasonal_generation.phase_ticks = 0;
        let handle = spawn(GameSession::new(definition).unwrap());
        let initial = handle.views.borrow().clone();
        let system = &initial.systems[0].seasonal_generation;
        assert_eq!(system.base_output, Energy(10));
        assert_eq!(system.effective_output, Energy(8));
        assert_eq!(system.phase_ticks, 0);
        assert_eq!(system.period_ticks, 4);
        assert_eq!(system.trend, SeasonalTrend::Rising);
        assert_eq!(system.next_turning_point_tick, Some(2));
        assert_eq!(
            initial.inspection.market_energy.seasonal_generation,
            *system
        );
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn views_project_trade_network_access() {
        let mut definition = definition();
        definition.player_trade_network_access = TradeNetworkAccess::ReservationContracts;
        let handle = spawn(GameSession::new(definition).unwrap());
        assert_eq!(
            handle.views.borrow().player.trade_network_access,
            TradeNetworkAccess::ReservationContracts
        );
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn actor_dispatches_supported_economy_commands_without_crossing_owner_boundary() {
        let mut definition = definition();
        definition.player_trade_network_access = TradeNetworkAccess::ReservationContracts;
        let session = GameSession::new(definition).unwrap();
        let handle = spawn(session);
        handle
            .request(AppRequest::DepositTank { amount: Energy(1) })
            .await
            .unwrap();
        assert_eq!(handle.views.borrow().player.tank_energy, Energy(99));
        handle
            .request(AppRequest::WithdrawTank { amount: Energy(1) })
            .await
            .unwrap();
        assert_eq!(handle.views.borrow().player.tank_energy, Energy(100));

        let policy = GovernorMarketPolicy {
            producer_margin_percent: 33,
            operating_reserve_ticks: 3,
            import_priorities: BTreeMap::new(),
        };
        handle
            .request(AppRequest::SetMarketPolicy {
                system: id("core:s0"),
                policy,
            })
            .await
            .unwrap();
        handle
            .request(AppRequest::CommitTrade {
                origin: id("core:s0"),
                destination: id("core:s1"),
                good: id("core:ore"),
                quantity: 1,
            })
            .await
            .unwrap();
        assert_eq!(handle.views.borrow().tick, 0);
        assert!(!handle.views.borrow().player.traveling);
        handle.request(AppRequest::Step).await.unwrap();
        assert!(handle.views.borrow().player.traveling);
        handle.request(AppRequest::CancelReservation).await.unwrap();
        assert!(
            handle
                .views
                .borrow()
                .events
                .iter()
                .any(|event| event.contains("cancelled"))
        );
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn governor_request_command_event_and_view_flow_is_typed_and_autonomous() {
        let mut definition = definition();
        definition.economy.life_support_burn_per_capita = Energy::ZERO;
        definition.economy.investments.insert(
            InvestmentKind::Storage,
            game_core::InvestmentShape {
                enabled: true,
                base_cost: Energy(100),
                cost_growth_percent: 150,
                maximum_level: 2,
                cooldown_ticks: 2,
                effect_per_level: 100,
            },
        );
        let handle = spawn(GameSession::new(definition).unwrap());
        assert!(handle.views.borrow().inspection.governor.governed);
        handle
            .request(AppRequest::SetInvestmentPolicy {
                system: id("core:s0"),
                policy: GovernorInvestmentPolicy {
                    allocation_percent: BTreeMap::from([(InvestmentKind::Storage, 100)]),
                },
            })
            .await
            .unwrap();
        assert!(
            handle
                .views
                .borrow()
                .events
                .iter()
                .any(|event| event.contains("policy changed"))
        );
        handle.request(AppRequest::Step).await.unwrap();
        let view = handle.views.borrow().clone();
        let storage = view
            .inspection
            .governor
            .investments
            .iter()
            .find(|investment| investment.kind == InvestmentKind::Storage)
            .unwrap();
        assert_eq!(storage.level, 1);
        assert_eq!(storage.next_cost, Some(Energy(150)));
        assert!(matches!(
            storage.status,
            InvestmentStatus::Completed {
                tick: 0,
                cost: Energy(100)
            }
        ));
        assert!(
            view.events
                .iter()
                .any(|event| event.contains("completed Storage investment level 1"))
        );

        handle
            .request(AppRequest::SelectSystem(id("core:s1")))
            .await
            .unwrap();
        assert!(!handle.views.borrow().inspection.governor.governed);
        assert!(matches!(
            handle
                .request(AppRequest::SetInvestmentPolicy {
                    system: id("core:s1"),
                    policy: GovernorInvestmentPolicy::default(),
                })
                .await,
            Err(AppError::Core(CoreError::UnauthorizedMarketPolicy))
        ));
        assert!(
            handle
                .views
                .borrow()
                .events
                .iter()
                .any(|event| event.contains("not authorized for this market"))
        );
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn governor_view_requires_the_exact_player_stable_id() {
        let mut definition = definition();
        definition.systems[0].governance = Governance {
            authority: MarketAuthority::Player(id("core:someone_else")),
        };
        let handle = spawn(GameSession::new(definition).unwrap());
        assert!(!handle.views.borrow().inspection.governor.governed);
        assert!(matches!(
            handle
                .request(AppRequest::SetMarketPolicy {
                    system: id("core:s0"),
                    policy: GovernorMarketPolicy {
                        producer_margin_percent: 25,
                        operating_reserve_ticks: 4,
                        import_priorities: BTreeMap::new(),
                    },
                })
                .await,
            Err(AppError::Core(CoreError::UnauthorizedMarketPolicy))
        ));
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn policy_requests_reject_autonomous_markets_without_changing_views() {
        let session = GameSession::new(definition()).unwrap();
        let handle = spawn(session);
        handle
            .request(AppRequest::SelectSystem(id("core:s1")))
            .await
            .unwrap();
        let before = handle.views.borrow().clone();
        assert!(matches!(
            handle
                .request(AppRequest::SetMarketPolicy {
                    system: id("core:s1"),
                    policy: GovernorMarketPolicy {
                        producer_margin_percent: 44,
                        operating_reserve_ticks: before
                            .inspection
                            .governor
                            .policy
                            .operating_reserve_ticks,
                        import_priorities: before
                            .inspection
                            .governor
                            .policy
                            .import_priorities
                            .clone(),
                    },
                })
                .await,
            Err(AppError::Core(CoreError::UnauthorizedMarketPolicy))
        ));
        let after = handle.views.borrow().clone();
        assert_eq!(
            after
                .inspection
                .market
                .iter()
                .map(|row| (row.good_id.clone(), row.buy_quote, row.funded_demand))
                .collect::<Vec<_>>(),
            before
                .inspection
                .market
                .iter()
                .map(|row| (row.good_id.clone(), row.buy_quote, row.funded_demand))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            after.inspection.market_energy.stock,
            before.inspection.market_energy.stock
        );
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn app_funded_demand_matches_canonical_core_snapshot_in_normal_and_emergency() {
        for emergency in [false, true] {
            let mut definition = definition();
            if emergency {
                definition.systems[1].energy_output_per_tick = Energy::ZERO;
                definition.systems[1].seasonal_generation.base_output = Energy::ZERO;
                definition.systems[1]
                    .seasonal_generation
                    .current_effective_output = Energy::ZERO;
                definition.systems[1].inventory.insert(id(ENERGY_ID), 7);
            }
            let mut expected_session = GameSession::new(definition.clone()).unwrap();
            if emergency {
                expected_session.step().unwrap();
            }
            let expected = expected_session
                .snapshot()
                .markets
                .into_iter()
                .find(|market| market.system_id == id("core:s1"))
                .unwrap()
                .demand;

            let handle = spawn(GameSession::new(definition).unwrap());
            handle
                .request(AppRequest::SelectSystem(id("core:s1")))
                .await
                .unwrap();
            if emergency {
                handle.request(AppRequest::Step).await.unwrap();
            }
            for row in &handle.views.borrow().inspection.market {
                assert_eq!(
                    row.funded_demand,
                    u64::from(expected[&row.good_id].funded),
                    "{} demand mismatch in emergency={emergency}",
                    row.good_id
                );
            }
            handle.shutdown().await.unwrap();
        }
    }

    #[tokio::test]
    async fn policy_requests_publish_recomputed_protection_and_reject_atomically() {
        let session = GameSession::new(definition()).unwrap();
        let handle = spawn(session);
        let system = id("core:s0");
        let policy = GovernorMarketPolicy {
            producer_margin_percent: 15,
            operating_reserve_ticks: 3,
            import_priorities: BTreeMap::new(),
        };
        handle
            .request(AppRequest::SetMarketPolicy {
                system: system.clone(),
                policy: policy.clone(),
            })
            .await
            .unwrap();
        assert_eq!(handle.views.borrow().inspection.governor.policy, policy);
        assert_eq!(
            handle
                .views
                .borrow()
                .inspection
                .market_energy
                .protected_liquidation_budget,
            Energy(5)
        );

        let before_budget = handle
            .views
            .borrow()
            .inspection
            .market_energy
            .protected_liquidation_budget;
        let before_quotes = handle
            .views
            .borrow()
            .inspection
            .market
            .iter()
            .map(|row| (row.good_id.clone(), row.buy_quote, row.sell_quote))
            .collect::<Vec<_>>();
        let infeasible = GovernorMarketPolicy {
            producer_margin_percent: 10_001,
            ..policy
        };
        assert!(matches!(
            handle
                .request(AppRequest::SetMarketPolicy {
                    system,
                    policy: infeasible,
                })
                .await,
            Err(AppError::Core(CoreError::InvalidPolicy))
        ));
        assert_eq!(
            handle
                .views
                .borrow()
                .inspection
                .market_energy
                .protected_liquidation_budget,
            before_budget
        );
        assert_eq!(
            handle
                .views
                .borrow()
                .inspection
                .market
                .iter()
                .map(|row| (row.good_id.clone(), row.buy_quote, row.sell_quote))
                .collect::<Vec<_>>(),
            before_quotes
        );
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_finishes_within_a_timeout() {
        let session = GameSession::new(definition()).unwrap();
        let handle = spawn(session);
        tokio::time::timeout(Duration::from_secs(1), handle.shutdown())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn concurrent_requests_are_acknowledged_in_channel_order() {
        let session = GameSession::new(definition()).unwrap();
        let handle = spawn(session);
        let (first, second) = tokio::join!(
            handle.request(AppRequest::SelectSystem(id("core:s1"))),
            handle.request(AppRequest::SelectSystem(id("core:s0")))
        );
        first.unwrap();
        second.unwrap();
        assert_eq!(handle.views.borrow().selected_system, id("core:s0"));
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn dropping_all_request_senders_stops_the_owner() {
        let session = GameSession::new(definition()).unwrap();
        let AppHandle {
            requests,
            views,
            task,
        } = spawn(session);
        drop(requests);
        drop(views);
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn request_queue_is_bounded() {
        let (sender, _receiver) = mpsc::channel(REQUEST_CAPACITY);
        for _ in 0..REQUEST_CAPACITY {
            let (reply, _response) = oneshot::channel();
            sender
                .try_send(Envelope {
                    request: AppRequest::Step,
                    reply,
                })
                .unwrap();
        }
        let (reply, _response) = oneshot::channel();
        assert!(matches!(
            sender.try_send(Envelope {
                request: AppRequest::Step,
                reply
            }),
            Err(mpsc::error::TrySendError::Full(_))
        ));
    }

    #[tokio::test]
    async fn view_retains_a_bounded_recent_event_history() {
        let session = GameSession::new(definition()).unwrap();
        let handle = spawn(session);
        for _ in 0..150 {
            handle.request(AppRequest::Step).await.unwrap();
        }
        let view = handle.views.borrow().clone();
        assert_eq!(view.tick, 150);
        assert_eq!(view.events.len(), EVENT_HISTORY);
        assert_eq!(
            view.events.last().map(|event| event.text.as_str()),
            Some("Tick 150")
        );
        assert!(
            view.events
                .windows(2)
                .all(|pair| pair[0].sequence < pair[1].sequence)
        );
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn remote_inspection_never_changes_the_local_trade_projection_or_target() {
        let mut changed = definition();
        changed.systems[1].inventory.insert(id("core:ore"), 50);
        let handle = spawn(GameSession::new(changed).unwrap());

        handle
            .request(AppRequest::SelectSystem(id("core:s1")))
            .await
            .unwrap();
        let remote = handle.views.borrow().clone();
        assert_eq!(remote.inspection.system.id, id("core:s1"));
        assert_eq!(remote.local_trade.system.id, id("core:s0"));
        assert!(remote.inspection.read_only_market);
        assert!(remote.local_trade.available);
        assert_eq!(
            remote
                .inspection
                .market
                .iter()
                .find(|row| row.good_id == id("core:ore"))
                .unwrap()
                .inventory,
            50
        );
        assert_eq!(
            remote
                .local_trade
                .market
                .iter()
                .find(|row| row.good_id == id("core:ore"))
                .unwrap()
                .inventory,
            10
        );

        handle
            .request(AppRequest::Buy {
                good: id("core:ore"),
                quantity: 1,
            })
            .await
            .unwrap();
        let bought = handle.views.borrow().clone();
        assert_eq!(bought.player.cargo_used, 1);
        assert_eq!(
            bought
                .inspection
                .market
                .iter()
                .find(|row| row.good_id == id("core:ore"))
                .unwrap()
                .inventory,
            50
        );
        assert_eq!(
            bought
                .local_trade
                .market
                .iter()
                .find(|row| row.good_id == id("core:ore"))
                .unwrap()
                .inventory,
            9
        );
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn system_summaries_expose_governed_identity_and_routes_from_player_location() {
        let handle = spawn(GameSession::new(definition()).unwrap());
        let view = handle.views.borrow().clone();
        assert_eq!(
            view.governed_system.as_ref().map(|system| &system.id),
            Some(&id("core:s0"))
        );
        let local = view
            .systems
            .iter()
            .find(|system| system.id == id("core:s0"))
            .unwrap();
        let remote = view
            .systems
            .iter()
            .find(|system| system.id == id("core:s1"))
            .unwrap();
        assert!(local.player_location);
        assert!(local.player_governed);
        assert_eq!(local.route_ticks_from_player, Some(0));
        assert_eq!(local.route_distance_from_player, Some(0.0));
        assert!(!remote.player_location);
        assert!(!remote.player_governed);
        assert!(remote.route_ticks_from_player.unwrap() > 0);
        assert!(remote.route_distance_from_player.unwrap() > 0.0);
        handle.shutdown().await.unwrap();
    }
}
