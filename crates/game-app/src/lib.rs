//! Async owner and immutable view boundary for the headless simulation.

use game_core::{
    BrownoutStage, ContentId, CoreError, ENERGY_ID, Energy, GameCommand, GameEvent, GameSession,
    MarketPolicy, ReservationStatus, route_travel_energy, ticks_for_distance, travel_energy,
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
        policy: MarketPolicy,
    },
    CancelReservation,
    Shutdown,
}

#[derive(Clone, Debug)]
pub struct SystemListItem {
    pub id: ContentId,
    pub name: String,
    pub coordinates: (f64, f64, f64),
    pub energy_stock: Energy,
    pub energy_capacity: Energy,
    pub health: EnergyHealth,
    pub brownout_stage: BrownoutStage,
    pub runway_ticks: u32,
    pub connections: Vec<ConnectionView>,
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
    pub route_energy_required: Option<Energy>,
    pub runway_jumps: Option<u64>,
    pub traveling: bool,
}

#[derive(Clone, Debug)]
pub struct ApplicationView {
    pub tick: u64,
    pub run_state: RunState,
    pub tick_rate: TickRate,
    pub systems: Vec<SystemListItem>,
    pub selected_system: ContentId,
    pub selected_route: Option<RouteView>,
    pub market_energy: MarketEnergyView,
    pub market: Vec<MarketRow>,
    pub player: PlayerStatusView,
    pub events: Vec<String>,
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
                    AppRequest::SetMarketPolicy { system, policy } => session.submit(GameCommand::SetMarketPolicy { system, policy }),
                    AppRequest::CancelReservation => session.submit(GameCommand::CancelReservation),
                    AppRequest::Shutdown => { let _ = envelope.reply.send(Ok(())); break; }
                };
                if result.is_err() { publish = true; }
                collect_events(&mut session, &mut history);
                if publish { views.send_replace(build_view(&mut session, selected.clone(), state, rate, &history)); }
                let _ = envelope.reply.send(result);
            }
            _ = interval.tick(), if state == RunState::Running => {
                session.step()?;
                collect_events(&mut session, &mut history);
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

fn collect_events(session: &mut GameSession, history: &mut VecDeque<String>) {
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
        history.push_back(format_event(&event, &labels));
    }
}

fn build_view(
    session: &mut GameSession,
    selected: ContentId,
    run_state: RunState,
    tick_rate: TickRate,
    events: &VecDeque<String>,
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
    let cargo_energy_value = player
        .cargo
        .iter()
        .map(|(good, quantity)| {
            player_market
                .and_then(|market| session.quotes(&market.system_id, good).ok())
                .and_then(|(buy, _)| {
                    i64::try_from(*quantity)
                        .ok()
                        .and_then(|q| buy.0.checked_mul(q))
                })
                .unwrap_or(0)
        })
        .sum::<i64>();
    let energy_values = snapshot
        .traders
        .iter()
        .map(|trader| {
            let cargo = trader
                .cargo
                .iter()
                .filter_map(|(good, quantity)| {
                    player_market
                        .and_then(|market| session.quotes(&market.system_id, good).ok())
                        .and_then(|(buy, _)| {
                            i64::try_from(*quantity)
                                .ok()
                                .and_then(|q| buy.0.checked_mul(q))
                        })
                })
                .sum::<i64>();
            (
                trader.id.clone(),
                trader.energy_tank.0.saturating_add(cargo),
            )
        })
        .collect::<Vec<_>>();
    let total_energy_value: i64 = energy_values.iter().map(|(_, worth)| *worth).sum();
    let player_energy_value = player.energy_tank.0.saturating_add(cargo_energy_value);
    let rank = 1 + energy_values
        .iter()
        .filter(|(id, worth)| {
            *worth > player_energy_value || (*worth == player_energy_value && id < &player.id)
        })
        .count();
    let total_sales: i64 = snapshot
        .traders
        .iter()
        .map(|trader| trader.ledger.sales_revenue.0)
        .sum();
    let system_names = snapshot
        .markets
        .iter()
        .map(|market| (market.system_id.clone(), market.name.clone()))
        .collect::<BTreeMap<_, _>>();
    let systems = snapshot
        .markets
        .iter()
        .map(|market| SystemListItem {
            id: market.system_id.clone(),
            name: market.name.clone(),
            coordinates: (market.position.x, market.position.y, market.position.z),
            energy_stock: market.energy_stock,
            energy_capacity: market.energy_storage_cap,
            health: energy_health(market),
            brownout_stage: market.brownout.stage,
            runway_ticks: market.brownout.ticks_of_burn,
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
        })
        .collect();
    let selected_market = snapshot
        .markets
        .iter()
        .find(|market| market.system_id == selected)
        .unwrap_or(&snapshot.markets[0]);
    let goods = session
        .catalog()
        .goods
        .values()
        .map(|good| (good.id.clone(), good.name.clone()))
        .collect::<Vec<_>>();
    let market = goods
        .into_iter()
        .map(|(id, name)| {
            let (buy, sell) = session
                .quotes(&selected_market.system_id, &id)
                .unwrap_or((Energy(0), Energy(0)));
            let inventory = selected_market.inventory.get(&id).copied().unwrap_or(0);
            let target = selected_market.targets.get(&id).copied().unwrap_or(0);
            let funded_demand = u64::from(
                selected_market
                    .demand
                    .get(&id)
                    .copied()
                    .unwrap_or_default()
                    .funded,
            );
            MarketRow {
                inventory,
                target,
                unit_cost: selected_market
                    .cost_basis
                    .get(&id)
                    .and_then(|basis| basis.unit_cost_ceil().ok())
                    .unwrap_or(Energy(0)),
                funded_demand,
                good_id: id,
                name,
                buy_quote: buy,
                sell_quote: sell,
            }
        })
        .collect();
    let route = if let Some(travel) = &player.travel {
        Some(build_route_view(
            session,
            &system_names,
            &travel.route,
            player.speed,
            Some(travel.next_leg.saturating_sub(1)),
            Some(travel.remaining_ticks),
        ))
    } else if selected != player.system {
        session
            .shortest_path(&player.system, &selected)
            .map(|(route, _)| {
                build_route_view(session, &system_names, &route, player.speed, None, None)
            })
    } else {
        None
    };
    let route_energy_required = route.as_ref().and_then(|route| {
        let ids = std::iter::once(route.legs.first()?.from_id.clone())
            .chain(route.legs.iter().map(|leg| leg.to_id.clone()))
            .collect::<Vec<_>>();
        route_travel_energy(session.graph(), &ids, player.travel_burn_per_distance).ok()
    });
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
        .and_then(|value| value.checked_add(selected_market.energy_flow.travel_burned.0))
        .expect("checked per-market flow ledger must remain reportable");
    ApplicationView {
        tick: snapshot.tick,
        run_state,
        tick_rate,
        systems,
        selected_system: selected_market.system_id.clone(),
        selected_route: route,
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
        },
        market,
        player: PlayerStatusView {
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
            cargo_used: player.cargo.values().sum(),
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
            route_energy_required,
            runway_jumps,
            traveling: player.travel.is_some(),
        },
        events: events.iter().cloned().collect(),
    }
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
    let total_ticks = legs.iter().map(|leg| leg.travel_ticks).sum();
    let remaining_ticks = current_leg.map(|index| {
        current_leg_remaining.unwrap_or(0)
            + legs
                .iter()
                .skip(index + 1)
                .map(|leg| leg.travel_ticks)
                .sum::<u32>()
    });
    let destination_id = route.last().cloned().expect("routes contain a destination");
    RouteView {
        destination_name: system_names
            .get(&destination_id)
            .cloned()
            .unwrap_or_else(|| "Unknown system".into()),
        destination_id,
        total_distance: legs.iter().map(|leg| leg.distance).sum(),
        total_ticks,
        remaining_ticks,
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
            "{} governor policy rejected: {reason}",
            labels.system(system)
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
        EconomyConfig, FleetDynamics, FleetMode, GameDefinition, GoodCategory, GoodDefinition,
        Governance, InvestmentPolicy, MarketAuthority, MarketPolicy, PopulationState, Position3,
        RefuelPolicy, SeasonalGenerationState, SourceDefinition, SystemDefinition,
        TraderDefinition,
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
        assert_eq!(initial.market.len(), 2);
        assert_eq!(initial.player.energy_value_rank, 1);
        assert_eq!(initial.player.energy_value_share_percent, 100.0);
        assert_eq!(initial.market_energy.health, EnergyHealth::Healthy);

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
    async fn actor_dispatches_supported_economy_commands_without_crossing_owner_boundary() {
        let session = GameSession::new(definition()).unwrap();
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

        let policy = MarketPolicy {
            producer_margin_percent: 33,
            ..MarketPolicy::default()
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
                    policy: MarketPolicy {
                        producer_margin_percent: 44,
                        ..MarketPolicy::default()
                    },
                })
                .await,
            Err(AppError::Core(CoreError::UnauthorizedMarketPolicy))
        ));
        let after = handle.views.borrow().clone();
        assert_eq!(
            after
                .market
                .iter()
                .map(|row| (row.good_id.clone(), row.buy_quote, row.funded_demand))
                .collect::<Vec<_>>(),
            before
                .market
                .iter()
                .map(|row| (row.good_id.clone(), row.buy_quote, row.funded_demand))
                .collect::<Vec<_>>()
        );
        assert_eq!(after.market_energy.stock, before.market_energy.stock);
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
            for row in &handle.views.borrow().market {
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
        let policy = MarketPolicy::default();
        handle
            .request(AppRequest::SetMarketPolicy {
                system: system.clone(),
                policy: policy.clone(),
            })
            .await
            .unwrap();
        assert_eq!(
            handle
                .views
                .borrow()
                .market_energy
                .protected_liquidation_budget,
            Energy(5)
        );

        let before_budget = handle
            .views
            .borrow()
            .market_energy
            .protected_liquidation_budget;
        let before_quotes = handle
            .views
            .borrow()
            .market
            .iter()
            .map(|row| (row.good_id.clone(), row.buy_quote, row.sell_quote))
            .collect::<Vec<_>>();
        let infeasible = MarketPolicy {
            liquidation_threshold_percent: u32::MAX,
            ..policy
        };
        assert!(matches!(
            handle
                .request(AppRequest::SetMarketPolicy {
                    system,
                    policy: infeasible,
                })
                .await,
            Err(AppError::Core(CoreError::InvalidPhysicalDefinition))
        ));
        assert_eq!(
            handle
                .views
                .borrow()
                .market_energy
                .protected_liquidation_budget,
            before_budget
        );
        assert_eq!(
            handle
                .views
                .borrow()
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
        assert_eq!(view.events.last().map(String::as_str), Some("Tick 150"));
        handle.shutdown().await.unwrap();
    }
}
