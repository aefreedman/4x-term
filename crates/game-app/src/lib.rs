//! Async owner and immutable view boundary for the headless simulation.

use game_core::{ContentId, CoreError, GameCommand, GameEvent, GameSession, Money};
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
    Buy { good: ContentId, quantity: u32 },
    Sell { good: ContentId, quantity: u32 },
    BeginTravel { destination: ContentId },
    Shutdown,
}

#[derive(Clone, Debug)]
pub struct SystemListItem {
    pub id: ContentId,
    pub name: String,
    pub coordinates: (f64, f64, f64),
}

#[derive(Clone, Debug)]
pub struct RouteView {
    pub systems: Vec<ContentId>,
    pub distance: f64,
    pub remaining_ticks: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct MarketRow {
    pub good_id: ContentId,
    pub name: String,
    pub inventory: u32,
    pub target: u32,
    pub buy_quote: Money,
    pub sell_quote: Money,
}

#[derive(Clone, Debug)]
pub struct PlayerStatusView {
    pub location: ContentId,
    pub currency: Money,
    pub cargo: BTreeMap<ContentId, u32>,
    pub cargo_used: u32,
    pub cargo_capacity: u32,
    pub cargo_value: Money,
    pub net_worth: Money,
    pub purchase_cost: i64,
    pub sales_revenue: i64,
    pub realized_profit: i64,
    pub units_moved: u64,
    pub transactions: u64,
    pub net_worth_rank: usize,
    pub net_worth_share_percent: f64,
    pub sales_share_percent: f64,
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
    let mut interval = tokio::time::interval(rate.duration());
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
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
                    AppRequest::SetTickRate(next) => { rate = next; interval = tokio::time::interval(rate.duration()); Ok(()) },
                    AppRequest::SelectSystem(id) => { selected = id; Ok(()) },
                    AppRequest::Buy { good, quantity } => session.submit(GameCommand::Buy { good, quantity }),
                    AppRequest::Sell { good, quantity } => session.submit(GameCommand::Sell { good, quantity }),
                    AppRequest::BeginTravel { destination } => session.submit(GameCommand::BeginTravel { destination }),
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

fn collect_events(session: &mut GameSession, history: &mut VecDeque<String>) {
    for event in session.drain_events() {
        if history.len() == EVENT_HISTORY {
            history.pop_front();
        }
        history.push_back(format_event(&event));
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
    let cargo_value = player
        .cargo
        .iter()
        .map(|(good, quantity)| {
            player_market
                .and_then(|market| session.quotes(&market.system_id, good).ok())
                .map_or(0, |(buy, _)| buy.0 * i64::from(*quantity))
        })
        .sum::<i64>();
    let net_worths = snapshot
        .traders
        .iter()
        .map(|trader| {
            (
                trader.id.clone(),
                trader.currency.0
                    + trader
                        .cargo
                        .values()
                        .map(|quantity| i64::from(*quantity) * 10)
                        .sum::<i64>(),
            )
        })
        .collect::<Vec<_>>();
    let total_net: i64 = net_worths.iter().map(|(_, worth)| *worth).sum();
    let player_net = player.currency.0 + cargo_value;
    let rank = 1 + net_worths
        .iter()
        .filter(|(id, worth)| *worth > player_net || (*worth == player_net && id < &player.id))
        .count();
    let total_sales: i64 = snapshot
        .traders
        .iter()
        .map(|trader| trader.ledger.sales_revenue)
        .sum();
    let systems = snapshot
        .markets
        .iter()
        .map(|market| SystemListItem {
            id: market.system_id.clone(),
            name: market.name.clone(),
            coordinates: (market.position.x, market.position.y, market.position.z),
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
                .unwrap_or((Money(0), Money(0)));
            MarketRow {
                inventory: selected_market.inventory.get(&id).copied().unwrap_or(0),
                target: selected_market.targets.get(&id).copied().unwrap_or(0),
                good_id: id,
                name,
                buy_quote: buy,
                sell_quote: sell,
            }
        })
        .collect();
    let route = if let Some(travel) = &player.travel {
        Some(RouteView {
            systems: travel.route.clone(),
            distance: session.graph().route_distance(&travel.route),
            remaining_ticks: Some(travel.remaining_ticks),
        })
    } else if selected != player.system {
        session
            .shortest_path(&player.system, &selected)
            .map(|(systems, distance)| RouteView {
                systems,
                distance,
                remaining_ticks: None,
            })
    } else {
        None
    };
    ApplicationView {
        tick: snapshot.tick,
        run_state,
        tick_rate,
        systems,
        selected_system: selected_market.system_id.clone(),
        selected_route: route,
        market,
        player: PlayerStatusView {
            location: player.system,
            currency: player.currency,
            cargo: player.cargo.clone(),
            cargo_used: player.cargo.values().sum(),
            cargo_capacity: player.cargo_capacity,
            cargo_value: Money(cargo_value),
            net_worth: Money(player_net),
            purchase_cost: player.ledger.purchase_cost,
            sales_revenue: player.ledger.sales_revenue,
            realized_profit: player.ledger.sales_revenue - player.ledger.purchase_cost,
            units_moved: player.ledger.cargo_units_moved,
            transactions: player.ledger.completed_transactions,
            net_worth_rank: rank,
            net_worth_share_percent: if total_net == 0 {
                0.0
            } else {
                player_net as f64 * 100.0 / total_net as f64
            },
            sales_share_percent: if total_sales == 0 {
                0.0
            } else {
                player.ledger.sales_revenue as f64 * 100.0 / total_sales as f64
            },
            traveling: player.travel.is_some(),
        },
        events: events.iter().cloned().collect(),
    }
}

fn format_event(event: &GameEvent) -> String {
    match event {
        GameEvent::TickAdvanced(tick) => format!("Tick {tick}"),
        GameEvent::Produced { system, recipe } => format!("{system}: completed {recipe}"),
        GameEvent::Consumed { system, recipe } => format!("{system}: consumed goods at {recipe}"),
        GameEvent::Bought {
            trader,
            good,
            quantity,
            total,
        } => format!("{trader} bought {quantity} {good} for ¤{}", total.0),
        GameEvent::Sold {
            trader,
            good,
            quantity,
            total,
        } => format!("{trader} sold {quantity} {good} for ¤{}", total.0),
        GameEvent::Departed {
            trader,
            destination,
        } => format!("{trader} departed for {destination}"),
        GameEvent::Arrived { trader, system } => format!("{trader} arrived at {system}"),
        GameEvent::Rejected(reason) => format!("Rejected: {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{
        GameDefinition, GoodCategory, GoodDefinition, Position3, SystemDefinition, TraderDefinition,
    };

    fn id(value: &str) -> ContentId {
        ContentId::new(value).unwrap()
    }

    fn definition() -> GameDefinition {
        let goods = vec![GoodDefinition {
            id: id("core:ore"),
            name: "Ore".into(),
            category: GoodCategory::Raw,
            base_price: Money(10),
        }];
        let systems = (0..2)
            .map(|i| SystemDefinition {
                id: id(&format!("core:s{i}")),
                name: format!("S{i}"),
                position: Position3 {
                    x: f64::from(i),
                    y: 0.0,
                    z: 0.0,
                },
                inventory: BTreeMap::from([(id("core:ore"), 10)]),
                targets: BTreeMap::from([(id("core:ore"), 10)]),
                currency: Money(1000),
                recipes: vec![],
                sources: vec![],
            })
            .collect();
        let traders = vec![TraderDefinition {
            id: id("core:player"),
            name: "Player".into(),
            system: id("core:s0"),
            currency: Money(100),
            cargo_capacity: 10,
            speed: 1.0,
            player: true,
        }];
        GameDefinition {
            goods,
            recipes: vec![],
            systems,
            traders,
        }
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
