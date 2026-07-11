//! Async owner and immutable view boundary for the headless simulation.

use game_core::{
    ContentId, CoreError, GameCommand, GameEvent, GameSession, Money, ticks_for_distance,
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
    pub connections: Vec<ConnectionView>,
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
    pub inventory: u32,
    pub target: u32,
    pub buy_quote: Money,
    pub sell_quote: Money,
}

#[derive(Clone, Debug)]
pub struct CargoItemView {
    pub good_id: ContentId,
    pub good_name: String,
    pub quantity: u32,
}

#[derive(Clone, Debug)]
pub struct PlayerStatusView {
    pub location: ContentId,
    pub location_name: String,
    pub currency: Money,
    pub cargo: Vec<CargoItemView>,
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
    ApplicationView {
        tick: snapshot.tick,
        run_state,
        tick_rate,
        systems,
        selected_system: selected_market.system_id.clone(),
        selected_route: route,
        market,
        player: PlayerStatusView {
            location_name: system_names
                .get(&player.system)
                .cloned()
                .unwrap_or_else(|| "Unknown system".into()),
            location: player.system,
            currency: player.currency,
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
        GameEvent::Produced { system, recipe } => format!(
            "{}: completed {}",
            labels.system(system),
            labels.recipe(recipe)
        ),
        GameEvent::Consumed { system, recipe } => format!(
            "{}: consumed goods at {}",
            labels.system(system),
            labels.recipe(recipe)
        ),
        GameEvent::Bought {
            trader,
            good,
            quantity,
            total,
        } => format!(
            "{} bought {quantity} {} for ¤{}",
            labels.trader(trader),
            labels.good(good),
            total.0
        ),
        GameEvent::Sold {
            trader,
            good,
            quantity,
            total,
        } => format!(
            "{} sold {quantity} {} for ¤{}",
            labels.trader(trader),
            labels.good(good),
            total.0
        ),
        GameEvent::Departed {
            trader,
            destination,
        } => format!(
            "{} departed for {}",
            labels.trader(trader),
            labels.system(destination)
        ),
        GameEvent::Arrived { trader, system } => format!(
            "{} arrived at {}",
            labels.trader(trader),
            labels.system(system)
        ),
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
            GameEvent::Consumed {
                system: id("core:s0"),
                recipe: id("core:smelt"),
            },
            GameEvent::Bought {
                trader: id("core:player"),
                good: id("core:ore"),
                quantity: 2,
                total: Money(10),
            },
            GameEvent::Sold {
                trader: id("core:player"),
                good: id("core:ore"),
                quantity: 2,
                total: Money(12),
            },
            GameEvent::Departed {
                trader: id("core:player"),
                destination: id("core:s0"),
            },
            GameEvent::Arrived {
                trader: id("core:player"),
                system: id("core:s0"),
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
        assert_eq!(initial.market.len(), 1);
        assert_eq!(initial.player.net_worth_rank, 1);
        assert_eq!(initial.player.net_worth_share_percent, 100.0);

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
        assert!(bought.player.net_worth.0 > 0);
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
                .any(|event| event == "Player departed for S1")
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
