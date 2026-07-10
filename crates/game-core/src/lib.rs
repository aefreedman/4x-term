//! Headless, synchronously stepped ECS simulation.

use bevy_ecs::prelude::*;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::fmt::{Display, Formatter};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentId(String);

impl ContentId {
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        let Some((namespace, name)) = value.split_once(':') else {
            return Err(CoreError::InvalidId(value));
        };
        if namespace.is_empty()
            || name.is_empty()
            || !value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, ':' | '_'))
        {
            return Err(CoreError::InvalidId(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ContentId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Money(pub i64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Position3 {
    #[must_use]
    pub fn distance(self, other: Self) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2) + (self.z - other.z).powi(2))
            .sqrt()
    }

    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoodCategory {
    Raw,
    Primary,
    Secondary,
}

#[derive(Clone, Debug)]
pub struct GoodDefinition {
    pub id: ContentId,
    pub name: String,
    pub category: GoodCategory,
    pub base_price: Money,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipeLayer {
    Primary,
    Secondary,
    Tertiary,
}

#[derive(Clone, Debug)]
pub struct GoodAmount {
    pub good: ContentId,
    pub quantity: u32,
}

#[derive(Clone, Debug)]
pub struct RecipeDefinition {
    pub id: ContentId,
    pub name: String,
    pub layer: RecipeLayer,
    pub inputs: Vec<GoodAmount>,
    pub outputs: Vec<GoodAmount>,
}

#[derive(Clone, Debug)]
pub struct SourceDefinition {
    pub good: ContentId,
    pub quantity_per_tick: u32,
}

#[derive(Clone, Debug)]
pub struct SystemDefinition {
    pub id: ContentId,
    pub name: String,
    pub position: Position3,
    pub inventory: BTreeMap<ContentId, u32>,
    pub targets: BTreeMap<ContentId, u32>,
    pub currency: Money,
    pub recipes: Vec<ContentId>,
    pub sources: Vec<SourceDefinition>,
}

#[derive(Clone, Debug)]
pub struct TraderDefinition {
    pub id: ContentId,
    pub name: String,
    pub system: ContentId,
    pub currency: Money,
    pub cargo_capacity: u32,
    pub speed: f64,
    pub player: bool,
}

#[derive(Clone, Debug)]
pub struct GameDefinition {
    pub goods: Vec<GoodDefinition>,
    pub recipes: Vec<RecipeDefinition>,
    pub systems: Vec<SystemDefinition>,
    pub traders: Vec<TraderDefinition>,
}

#[derive(Component, Clone, Debug)]
pub struct StableId(pub ContentId);

#[derive(Component, Clone, Debug)]
pub struct DisplayName(pub String);

#[derive(Component, Clone, Copy, Debug)]
pub struct SystemMarker;

#[derive(Component, Clone, Copy, Debug)]
pub struct SpatialPosition(pub Position3);

#[derive(Component, Clone, Debug)]
pub struct Market {
    pub inventory: BTreeMap<ContentId, u32>,
    pub targets: BTreeMap<ContentId, u32>,
    pub currency: Money,
    pub recipes: Vec<ContentId>,
    pub sources: Vec<SourceDefinition>,
}

#[derive(Clone, Debug)]
pub struct TravelPlan {
    pub destination: ContentId,
    pub route: Vec<ContentId>,
    pub next_leg: usize,
    pub remaining_ticks: u32,
}

#[derive(Clone, Debug, Default)]
pub struct TradeLedger {
    pub purchase_cost: i64,
    pub sales_revenue: i64,
    pub cargo_units_moved: u64,
    pub completed_transactions: u64,
}

#[derive(Component, Clone, Debug)]
pub struct Trader {
    pub system: ContentId,
    pub currency: Money,
    pub cargo: BTreeMap<ContentId, u32>,
    pub cargo_capacity: u32,
    pub speed: f64,
    pub travel: Option<TravelPlan>,
    pub ledger: TradeLedger,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct PlayerControlled;

#[derive(Resource, Clone, Debug)]
pub struct Catalog {
    pub goods: BTreeMap<ContentId, GoodDefinition>,
    pub recipes: BTreeMap<ContentId, RecipeDefinition>,
}

#[derive(Resource, Clone, Debug)]
pub struct SystemGraph {
    positions: BTreeMap<ContentId, Position3>,
    edges: BTreeMap<ContentId, Vec<(ContentId, f64)>>,
}

impl SystemGraph {
    pub fn build(systems: &[SystemDefinition]) -> Result<Self, CoreError> {
        if systems.is_empty() {
            return Err(CoreError::EmptyGraph);
        }
        let positions: BTreeMap<_, _> = systems
            .iter()
            .map(|system| (system.id.clone(), system.position))
            .collect();
        let mut undirected = BTreeSet::new();
        for system in systems {
            let mut neighbors: Vec<_> = systems
                .iter()
                .filter(|other| other.id != system.id)
                .map(|other| (system.position.distance(other.position), other.id.clone()))
                .collect();
            neighbors.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            for (_, neighbor) in neighbors.into_iter().take(3) {
                let edge = if system.id < neighbor {
                    (system.id.clone(), neighbor)
                } else {
                    (neighbor, system.id.clone())
                };
                undirected.insert(edge);
            }
        }
        let mut edges: BTreeMap<ContentId, Vec<(ContentId, f64)>> = positions
            .keys()
            .cloned()
            .map(|id| (id, Vec::new()))
            .collect();
        for (a, b) in undirected {
            let distance = positions[&a].distance(positions[&b]);
            edges
                .get_mut(&a)
                .expect("known graph node")
                .push((b.clone(), distance));
            edges
                .get_mut(&b)
                .expect("known graph node")
                .push((a.clone(), distance));
        }
        for neighbors in edges.values_mut() {
            neighbors.sort_by(|a, b| a.0.cmp(&b.0));
        }
        let graph = Self { positions, edges };
        if graph.reachable_count(systems[0].id.clone()) != systems.len() {
            return Err(CoreError::DisconnectedGraph);
        }
        Ok(graph)
    }

    fn reachable_count(&self, start: ContentId) -> usize {
        let mut seen = BTreeSet::from([start.clone()]);
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            if let Some(neighbors) = self.edges.get(&node) {
                for (next, _) in neighbors {
                    if seen.insert(next.clone()) {
                        stack.push(next.clone());
                    }
                }
            }
        }
        seen.len()
    }

    #[must_use]
    pub fn neighbors(&self, id: &ContentId) -> &[(ContentId, f64)] {
        self.edges.get(id).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn position(&self, id: &ContentId) -> Option<Position3> {
        self.positions.get(id).copied()
    }

    pub fn shortest_path(
        &self,
        start: &ContentId,
        goal: &ContentId,
    ) -> Option<(Vec<ContentId>, f64)> {
        if start == goal {
            return Some((vec![start.clone()], 0.0));
        }
        #[derive(Clone)]
        struct State {
            cost: f64,
            id: ContentId,
        }
        impl Eq for State {}
        impl PartialEq for State {
            fn eq(&self, other: &Self) -> bool {
                self.cost.total_cmp(&other.cost) == Ordering::Equal && self.id == other.id
            }
        }
        impl Ord for State {
            fn cmp(&self, other: &Self) -> Ordering {
                other
                    .cost
                    .total_cmp(&self.cost)
                    .then_with(|| other.id.cmp(&self.id))
            }
        }
        impl PartialOrd for State {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        let mut distances = BTreeMap::from([(start.clone(), 0.0)]);
        let mut previous = BTreeMap::<ContentId, ContentId>::new();
        let mut heap = BinaryHeap::from([State {
            cost: 0.0,
            id: start.clone(),
        }]);
        while let Some(State { cost, id }) = heap.pop() {
            if id == *goal {
                let mut path = vec![goal.clone()];
                let mut cursor = goal;
                while let Some(parent) = previous.get(cursor) {
                    path.push(parent.clone());
                    cursor = parent;
                }
                path.reverse();
                return Some((path, cost));
            }
            if cost > *distances.get(&id).unwrap_or(&f64::INFINITY) {
                continue;
            }
            for (next, edge_cost) in self.neighbors(&id) {
                let candidate = cost + edge_cost;
                let current = distances.get(next).copied().unwrap_or(f64::INFINITY);
                let replace = candidate < current
                    || (candidate.total_cmp(&current) == Ordering::Equal
                        && previous.get(next).is_none_or(|old| id < *old));
                if replace {
                    distances.insert(next.clone(), candidate);
                    previous.insert(next.clone(), id.clone());
                    heap.push(State {
                        cost: candidate,
                        id: next.clone(),
                    });
                }
            }
        }
        None
    }

    #[must_use]
    pub fn route_distance(&self, route: &[ContentId]) -> f64 {
        route
            .windows(2)
            .filter_map(|pair| {
                self.neighbors(&pair[0])
                    .iter()
                    .find(|(id, _)| id == &pair[1])
                    .map(|(_, distance)| *distance)
            })
            .sum()
    }
}

#[derive(Resource, Default)]
struct EventBuffer(Vec<GameEvent>);

#[derive(Resource, Default)]
struct Clock(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameEvent {
    TickAdvanced(u64),
    Produced {
        system: ContentId,
        recipe: ContentId,
    },
    Consumed {
        system: ContentId,
        recipe: ContentId,
    },
    Bought {
        trader: ContentId,
        good: ContentId,
        quantity: u32,
        total: Money,
    },
    Sold {
        trader: ContentId,
        good: ContentId,
        quantity: u32,
        total: Money,
    },
    Departed {
        trader: ContentId,
        destination: ContentId,
    },
    Arrived {
        trader: ContentId,
        system: ContentId,
    },
    Rejected(String),
}

#[derive(Clone, Debug)]
pub enum GameCommand {
    Buy { good: ContentId, quantity: u32 },
    Sell { good: ContentId, quantity: u32 },
    BeginTravel { destination: ContentId },
}

#[derive(Error, Debug, Clone, Eq, PartialEq)]
pub enum CoreError {
    #[error("invalid content id: {0}")]
    InvalidId(String),
    #[error("graph has no systems")]
    EmptyGraph,
    #[error("system graph is disconnected")]
    DisconnectedGraph,
    #[error("unknown {kind}: {id}")]
    Unknown { kind: &'static str, id: String },
    #[error("quantity must be positive")]
    ZeroQuantity,
    #[error("trader is in transit")]
    InTransit,
    #[error("insufficient stock")]
    InsufficientStock,
    #[error("insufficient funds")]
    InsufficientFunds,
    #[error("insufficient cargo capacity")]
    InsufficientCapacity,
    #[error("destination is current system")]
    AlreadyThere,
    #[error("no route to destination")]
    NoRoute,
    #[error("arithmetic overflow")]
    Overflow,
    #[error("definition must contain exactly one player")]
    InvalidPlayerCount,
}

#[derive(Clone, Debug)]
pub struct MarketSnapshot {
    pub system_id: ContentId,
    pub name: String,
    pub position: Position3,
    pub inventory: BTreeMap<ContentId, u32>,
    pub targets: BTreeMap<ContentId, u32>,
    pub currency: Money,
}

#[derive(Clone, Debug)]
pub struct TraderSnapshot {
    pub id: ContentId,
    pub name: String,
    pub system: ContentId,
    pub currency: Money,
    pub cargo: BTreeMap<ContentId, u32>,
    pub cargo_capacity: u32,
    pub travel: Option<TravelPlan>,
    pub ledger: TradeLedger,
    pub player: bool,
}

#[derive(Clone, Debug)]
pub struct CoreSnapshot {
    pub tick: u64,
    pub markets: Vec<MarketSnapshot>,
    pub traders: Vec<TraderSnapshot>,
}

pub struct GameSession {
    world: World,
}

impl GameSession {
    pub fn new(definition: GameDefinition) -> Result<Self, CoreError> {
        let player_count = definition
            .traders
            .iter()
            .filter(|trader| trader.player)
            .count();
        if player_count != 1 {
            return Err(CoreError::InvalidPlayerCount);
        }
        let graph = SystemGraph::build(&definition.systems)?;
        let catalog = Catalog {
            goods: definition
                .goods
                .into_iter()
                .map(|good| (good.id.clone(), good))
                .collect(),
            recipes: definition
                .recipes
                .into_iter()
                .map(|recipe| (recipe.id.clone(), recipe))
                .collect(),
        };
        let mut world = World::new();
        world.insert_resource(graph);
        world.insert_resource(catalog);
        world.insert_resource(Clock::default());
        world.insert_resource(EventBuffer::default());
        for system in definition.systems {
            world.spawn((
                StableId(system.id),
                DisplayName(system.name),
                SystemMarker,
                SpatialPosition(system.position),
                Market {
                    inventory: system.inventory,
                    targets: system.targets,
                    currency: system.currency,
                    recipes: system.recipes,
                    sources: system.sources,
                },
            ));
        }
        for trader in definition.traders {
            let mut entity = world.spawn((
                StableId(trader.id),
                DisplayName(trader.name),
                Trader {
                    system: trader.system,
                    currency: trader.currency,
                    cargo: BTreeMap::new(),
                    cargo_capacity: trader.cargo_capacity,
                    speed: trader.speed,
                    travel: None,
                    ledger: TradeLedger::default(),
                },
            ));
            if trader.player {
                entity.insert(PlayerControlled);
            }
        }
        Ok(Self { world })
    }

    #[must_use]
    pub fn tick(&self) -> u64 {
        self.world.resource::<Clock>().0
    }

    pub fn submit(&mut self, command: GameCommand) -> Result<(), CoreError> {
        let result = match command {
            GameCommand::Buy { good, quantity } => self.player_buy(&good, quantity),
            GameCommand::Sell { good, quantity } => self.player_sell(&good, quantity),
            GameCommand::BeginTravel { destination } => self.player_travel(&destination),
        };
        if let Err(error) = &result {
            self.world
                .resource_mut::<EventBuffer>()
                .0
                .push(GameEvent::Rejected(error.to_string()));
        }
        result
    }

    pub fn step(&mut self) -> Result<(), CoreError> {
        self.advance_travel();
        self.replenish_sources()?;
        for layer in [
            RecipeLayer::Primary,
            RecipeLayer::Secondary,
            RecipeLayer::Tertiary,
        ] {
            self.execute_recipes(layer)?;
        }
        self.run_automated_traders()?;
        self.world.resource_mut::<Clock>().0 += 1;
        let tick = self.tick();
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::TickAdvanced(tick));
        Ok(())
    }

    pub fn drain_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.world.resource_mut::<EventBuffer>().0)
    }

    pub fn snapshot(&mut self) -> CoreSnapshot {
        let mut markets = self
            .world
            .query_filtered::<(&StableId, &DisplayName, &SpatialPosition, &Market), With<SystemMarker>>()
            .iter(&self.world)
            .map(|(id, name, position, market)| MarketSnapshot {
                system_id: id.0.clone(),
                name: name.0.clone(),
                position: position.0,
                inventory: market.inventory.clone(),
                targets: market.targets.clone(),
                currency: market.currency,
            })
            .collect::<Vec<_>>();
        markets.sort_by(|a, b| a.system_id.cmp(&b.system_id));
        let mut traders = self
            .world
            .query::<(&StableId, &DisplayName, &Trader, Option<&PlayerControlled>)>()
            .iter(&self.world)
            .map(|(id, name, trader, player)| TraderSnapshot {
                id: id.0.clone(),
                name: name.0.clone(),
                system: trader.system.clone(),
                currency: trader.currency,
                cargo: trader.cargo.clone(),
                cargo_capacity: trader.cargo_capacity,
                travel: trader.travel.clone(),
                ledger: trader.ledger.clone(),
                player: player.is_some(),
            })
            .collect::<Vec<_>>();
        traders.sort_by(|a, b| a.id.cmp(&b.id));
        CoreSnapshot {
            tick: self.tick(),
            markets,
            traders,
        }
    }

    #[must_use]
    pub fn graph(&self) -> &SystemGraph {
        self.world.resource::<SystemGraph>()
    }

    #[must_use]
    pub fn catalog(&self) -> &Catalog {
        self.world.resource::<Catalog>()
    }

    pub fn quotes(
        &mut self,
        system: &ContentId,
        good: &ContentId,
    ) -> Result<(Money, Money), CoreError> {
        let entity = self.market_entity(system)?;
        let market = self.world.get::<Market>(entity).expect("market");
        Ok((
            self.buy_quote(market, good)?,
            self.sell_quote(market, good)?,
        ))
    }

    #[must_use]
    pub fn shortest_path(
        &self,
        start: &ContentId,
        destination: &ContentId,
    ) -> Option<(Vec<ContentId>, f64)> {
        self.world
            .resource::<SystemGraph>()
            .shortest_path(start, destination)
    }

    fn player_entity(&mut self) -> Result<Entity, CoreError> {
        self.world
            .query_filtered::<Entity, (With<Trader>, With<PlayerControlled>)>()
            .iter(&self.world)
            .next()
            .ok_or(CoreError::InvalidPlayerCount)
    }

    fn market_entity(&mut self, system_id: &ContentId) -> Result<Entity, CoreError> {
        self.world
            .query_filtered::<(Entity, &StableId), With<Market>>()
            .iter(&self.world)
            .find(|(_, id)| &id.0 == system_id)
            .map(|(entity, _)| entity)
            .ok_or_else(|| CoreError::Unknown {
                kind: "system",
                id: system_id.to_string(),
            })
    }

    fn player_buy(&mut self, good: &ContentId, quantity: u32) -> Result<(), CoreError> {
        let trader_entity = self.player_entity()?;
        let system = {
            let trader = self
                .world
                .get::<Trader>(trader_entity)
                .expect("trader component");
            if trader.travel.is_some() {
                return Err(CoreError::InTransit);
            }
            trader.system.clone()
        };
        self.buy(trader_entity, &system, good, quantity)
    }

    fn player_sell(&mut self, good: &ContentId, quantity: u32) -> Result<(), CoreError> {
        let trader_entity = self.player_entity()?;
        let system = {
            let trader = self
                .world
                .get::<Trader>(trader_entity)
                .expect("trader component");
            if trader.travel.is_some() {
                return Err(CoreError::InTransit);
            }
            trader.system.clone()
        };
        self.sell(trader_entity, &system, good, quantity)
    }

    fn player_travel(&mut self, destination: &ContentId) -> Result<(), CoreError> {
        let entity = self.player_entity()?;
        self.begin_travel(entity, destination)
    }

    fn buy(
        &mut self,
        trader_entity: Entity,
        system: &ContentId,
        good: &ContentId,
        quantity: u32,
    ) -> Result<(), CoreError> {
        if quantity == 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let market_entity = self.market_entity(system)?;
        let price = {
            let market = self.world.get::<Market>(market_entity).expect("market");
            self.sell_quote(market, good)?
        };
        let total = price
            .0
            .checked_mul(i64::from(quantity))
            .ok_or(CoreError::Overflow)?;
        let (
            market_stock,
            market_currency,
            trader_currency,
            cargo_quantity,
            purchase_cost,
            transactions,
        ) = {
            let market = self.world.get::<Market>(market_entity).expect("market");
            let stock = market.inventory.get(good).copied().unwrap_or(0);
            if stock < quantity {
                return Err(CoreError::InsufficientStock);
            }
            let trader = self.world.get::<Trader>(trader_entity).expect("trader");
            if trader.currency.0 < total {
                return Err(CoreError::InsufficientFunds);
            }
            let used = trader
                .cargo
                .values()
                .try_fold(0_u32, |sum, value| sum.checked_add(*value))
                .ok_or(CoreError::Overflow)?;
            if used.checked_add(quantity).ok_or(CoreError::Overflow)? > trader.cargo_capacity {
                return Err(CoreError::InsufficientCapacity);
            }
            (
                stock - quantity,
                market
                    .currency
                    .0
                    .checked_add(total)
                    .ok_or(CoreError::Overflow)?,
                trader.currency.0 - total,
                trader
                    .cargo
                    .get(good)
                    .copied()
                    .unwrap_or(0)
                    .checked_add(quantity)
                    .ok_or(CoreError::Overflow)?,
                trader
                    .ledger
                    .purchase_cost
                    .checked_add(total)
                    .ok_or(CoreError::Overflow)?,
                trader
                    .ledger
                    .completed_transactions
                    .checked_add(1)
                    .ok_or(CoreError::Overflow)?,
            )
        };
        let mut market = self.world.get_mut::<Market>(market_entity).expect("market");
        market.inventory.insert(good.clone(), market_stock);
        market.currency.0 = market_currency;
        let trader_id = self
            .world
            .get::<StableId>(trader_entity)
            .expect("id")
            .0
            .clone();
        let mut trader = self.world.get_mut::<Trader>(trader_entity).expect("trader");
        trader.currency.0 = trader_currency;
        trader.cargo.insert(good.clone(), cargo_quantity);
        trader.ledger.purchase_cost = purchase_cost;
        trader.ledger.completed_transactions = transactions;
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::Bought {
                trader: trader_id,
                good: good.clone(),
                quantity,
                total: Money(total),
            });
        Ok(())
    }

    fn sell(
        &mut self,
        trader_entity: Entity,
        system: &ContentId,
        good: &ContentId,
        quantity: u32,
    ) -> Result<(), CoreError> {
        if quantity == 0 {
            return Err(CoreError::ZeroQuantity);
        }
        let market_entity = self.market_entity(system)?;
        let price = {
            let market = self.world.get::<Market>(market_entity).expect("market");
            self.buy_quote(market, good)?
        };
        let total = price
            .0
            .checked_mul(i64::from(quantity))
            .ok_or(CoreError::Overflow)?;
        let (
            market_currency,
            market_stock,
            trader_currency,
            cargo_quantity,
            sales_revenue,
            units_moved,
            transactions,
        ) = {
            let market = self.world.get::<Market>(market_entity).expect("market");
            if market.currency.0 < total {
                return Err(CoreError::InsufficientFunds);
            }
            let trader = self.world.get::<Trader>(trader_entity).expect("trader");
            let cargo = trader.cargo.get(good).copied().unwrap_or(0);
            if cargo < quantity {
                return Err(CoreError::InsufficientStock);
            }
            (
                market.currency.0 - total,
                market
                    .inventory
                    .get(good)
                    .copied()
                    .unwrap_or(0)
                    .checked_add(quantity)
                    .ok_or(CoreError::Overflow)?,
                trader
                    .currency
                    .0
                    .checked_add(total)
                    .ok_or(CoreError::Overflow)?,
                cargo - quantity,
                trader
                    .ledger
                    .sales_revenue
                    .checked_add(total)
                    .ok_or(CoreError::Overflow)?,
                trader
                    .ledger
                    .cargo_units_moved
                    .checked_add(u64::from(quantity))
                    .ok_or(CoreError::Overflow)?,
                trader
                    .ledger
                    .completed_transactions
                    .checked_add(1)
                    .ok_or(CoreError::Overflow)?,
            )
        };
        let trader_id = self
            .world
            .get::<StableId>(trader_entity)
            .expect("id")
            .0
            .clone();
        let mut trader = self.world.get_mut::<Trader>(trader_entity).expect("trader");
        trader.currency.0 = trader_currency;
        if cargo_quantity == 0 {
            trader.cargo.remove(good);
        } else {
            trader.cargo.insert(good.clone(), cargo_quantity);
        }
        trader.ledger.sales_revenue = sales_revenue;
        trader.ledger.cargo_units_moved = units_moved;
        trader.ledger.completed_transactions = transactions;
        let mut market = self.world.get_mut::<Market>(market_entity).expect("market");
        market.currency.0 = market_currency;
        market.inventory.insert(good.clone(), market_stock);
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::Sold {
                trader: trader_id,
                good: good.clone(),
                quantity,
                total: Money(total),
            });
        Ok(())
    }

    fn midpoint(&self, market: &Market, good: &ContentId) -> Result<Money, CoreError> {
        let definition = self
            .world
            .resource::<Catalog>()
            .goods
            .get(good)
            .ok_or_else(|| CoreError::Unknown {
                kind: "good",
                id: good.to_string(),
            })?;
        let target = i64::from(market.targets.get(good).copied().unwrap_or(1).max(1));
        let inventory = i64::from(market.inventory.get(good).copied().unwrap_or(0));
        let scarcity = (target - inventory).clamp(-target, target);
        let adjustment = definition
            .base_price
            .0
            .checked_mul(scarcity)
            .ok_or(CoreError::Overflow)?
            / (2 * target);
        Ok(Money((definition.base_price.0 + adjustment).max(1)))
    }

    fn buy_quote(&self, market: &Market, good: &ContentId) -> Result<Money, CoreError> {
        Ok(Money((self.midpoint(market, good)?.0 * 90 / 100).max(1)))
    }

    fn sell_quote(&self, market: &Market, good: &ContentId) -> Result<Money, CoreError> {
        Ok(Money((self.midpoint(market, good)?.0 * 110 / 100).max(1)))
    }

    fn begin_travel(
        &mut self,
        trader_entity: Entity,
        destination: &ContentId,
    ) -> Result<(), CoreError> {
        let (start, speed) = {
            let trader = self.world.get::<Trader>(trader_entity).expect("trader");
            if trader.travel.is_some() {
                return Err(CoreError::InTransit);
            }
            (trader.system.clone(), trader.speed)
        };
        if &start == destination {
            return Err(CoreError::AlreadyThere);
        }
        let (route, _) = self
            .world
            .resource::<SystemGraph>()
            .shortest_path(&start, destination)
            .ok_or(CoreError::NoRoute)?;
        let first_distance = self
            .world
            .resource::<SystemGraph>()
            .route_distance(&route[..2]);
        let remaining_ticks = ticks_for_distance(first_distance, speed);
        self.world
            .get_mut::<Trader>(trader_entity)
            .expect("trader")
            .travel = Some(TravelPlan {
            destination: destination.clone(),
            route,
            next_leg: 1,
            remaining_ticks,
        });
        let trader_id = self
            .world
            .get::<StableId>(trader_entity)
            .expect("id")
            .0
            .clone();
        self.world
            .resource_mut::<EventBuffer>()
            .0
            .push(GameEvent::Departed {
                trader: trader_id,
                destination: destination.clone(),
            });
        Ok(())
    }

    fn advance_travel(&mut self) {
        let graph = self.world.resource::<SystemGraph>().clone();
        let mut arrivals = Vec::new();
        let mut query = self.world.query::<(Entity, &StableId, &mut Trader)>();
        for (entity, id, mut trader) in query.iter_mut(&mut self.world) {
            let speed = trader.speed;
            let Some(mut plan) = trader.travel.take() else {
                continue;
            };
            plan.remaining_ticks = plan.remaining_ticks.saturating_sub(1);
            if plan.remaining_ticks > 0 {
                trader.travel = Some(plan);
                continue;
            }
            trader.system = plan.route[plan.next_leg].clone();
            plan.next_leg += 1;
            if plan.next_leg >= plan.route.len() {
                let system = trader.system.clone();
                arrivals.push((entity, id.0.clone(), system));
            } else {
                let from = &plan.route[plan.next_leg - 1];
                let to = &plan.route[plan.next_leg];
                let distance = graph
                    .neighbors(from)
                    .iter()
                    .find(|(id, _)| id == to)
                    .map_or(0.0, |(_, distance)| *distance);
                plan.remaining_ticks = ticks_for_distance(distance, speed);
                trader.travel = Some(plan);
            }
        }
        for (_, trader, system) in arrivals {
            self.world
                .resource_mut::<EventBuffer>()
                .0
                .push(GameEvent::Arrived { trader, system });
        }
    }

    fn replenish_sources(&mut self) -> Result<(), CoreError> {
        let updates = self
            .world
            .query::<(Entity, &Market)>()
            .iter(&self.world)
            .map(|(entity, market)| {
                let mut inventory = market.inventory.clone();
                for source in &market.sources {
                    let stock = inventory.entry(source.good.clone()).or_default();
                    *stock = stock
                        .checked_add(source.quantity_per_tick)
                        .ok_or(CoreError::Overflow)?;
                }
                Ok((entity, inventory))
            })
            .collect::<Result<Vec<_>, CoreError>>()?;
        for (entity, inventory) in updates {
            self.world
                .get_mut::<Market>(entity)
                .expect("market")
                .inventory = inventory;
        }
        Ok(())
    }

    fn execute_recipes(&mut self, layer: RecipeLayer) -> Result<(), CoreError> {
        let recipes = self.world.resource::<Catalog>().recipes.clone();
        let mut produced = Vec::new();
        let mut query = self.world.query::<(&StableId, &mut Market)>();
        for (system_id, mut market) in query.iter_mut(&mut self.world) {
            for recipe_id in market.recipes.clone() {
                let recipe = recipes.get(&recipe_id).ok_or_else(|| CoreError::Unknown {
                    kind: "recipe",
                    id: recipe_id.to_string(),
                })?;
                if recipe.layer != layer
                    || !recipe.inputs.iter().all(|input| {
                        market.inventory.get(&input.good).copied().unwrap_or(0) >= input.quantity
                    })
                {
                    continue;
                }
                let mut next_inventory = market.inventory.clone();
                for input in &recipe.inputs {
                    *next_inventory
                        .get_mut(&input.good)
                        .expect("validated input") -= input.quantity;
                }
                for output in &recipe.outputs {
                    let stock = next_inventory.entry(output.good.clone()).or_default();
                    *stock = stock
                        .checked_add(output.quantity)
                        .ok_or(CoreError::Overflow)?;
                }
                market.inventory = next_inventory;
                produced.push((system_id.0.clone(), recipe.id.clone(), layer));
            }
        }
        for (system, recipe, layer) in produced {
            let event = if layer == RecipeLayer::Tertiary {
                GameEvent::Consumed { system, recipe }
            } else {
                GameEvent::Produced { system, recipe }
            };
            self.world.resource_mut::<EventBuffer>().0.push(event);
        }
        Ok(())
    }

    fn run_automated_traders(&mut self) -> Result<(), CoreError> {
        let automated: Vec<Entity> = self
            .world
            .query_filtered::<Entity, (With<Trader>, Without<PlayerControlled>)>()
            .iter(&self.world)
            .collect();
        for entity in automated {
            let (system, traveling, cargo) = {
                let trader = self.world.get::<Trader>(entity).expect("trader");
                (
                    trader.system.clone(),
                    trader.travel.is_some(),
                    trader.cargo.clone(),
                )
            };
            if traveling {
                continue;
            }
            if let Some((good, quantity)) = cargo
                .iter()
                .next()
                .map(|(good, quantity)| (good.clone(), *quantity))
            {
                let _ = self.sell(entity, &system, &good, quantity);
                continue;
            }
            if let Some((good, destination)) = self.best_trade(entity, &system)? {
                let market_entity = self.market_entity(&system)?;
                let available = self
                    .world
                    .get::<Market>(market_entity)
                    .expect("market")
                    .inventory
                    .get(&good)
                    .copied()
                    .unwrap_or(0);
                let trader = self.world.get::<Trader>(entity).expect("trader");
                let unit = self
                    .sell_quote(
                        self.world.get::<Market>(market_entity).expect("market"),
                        &good,
                    )?
                    .0;
                let affordable = if unit > 0 {
                    u32::try_from(trader.currency.0 / unit).unwrap_or(u32::MAX)
                } else {
                    0
                };
                let quantity = available.min(trader.cargo_capacity).min(affordable);
                if quantity > 0 && self.buy(entity, &system, &good, quantity).is_ok() {
                    self.begin_travel(entity, &destination)?;
                }
            }
        }
        Ok(())
    }

    fn best_trade(
        &mut self,
        _trader: Entity,
        origin: &ContentId,
    ) -> Result<Option<(ContentId, ContentId)>, CoreError> {
        let origin_entity = self.market_entity(origin)?;
        let origin_market = self
            .world
            .get::<Market>(origin_entity)
            .expect("market")
            .clone();
        let graph = self.world.resource::<SystemGraph>().clone();
        let markets: Vec<(ContentId, Market)> = self
            .world
            .query_filtered::<(&StableId, &Market), With<SystemMarker>>()
            .iter(&self.world)
            .map(|(id, market)| (id.0.clone(), market.clone()))
            .collect();
        let mut candidates = Vec::new();
        for (good, stock) in &origin_market.inventory {
            if *stock == 0 {
                continue;
            }
            let origin_price = self.sell_quote(&origin_market, good)?.0;
            for (destination, market) in &markets {
                if destination == origin {
                    continue;
                }
                let Some((route, distance)) = graph.shortest_path(origin, destination) else {
                    continue;
                };
                let destination_price = self.buy_quote(market, good)?.0;
                let profit = destination_price - origin_price;
                if profit > 0 {
                    let ticks = graph.route_distance(&route).ceil().max(1.0);
                    candidates.push((
                        profit as f64 / ticks,
                        good.clone(),
                        destination.clone(),
                        distance,
                    ));
                }
            }
        }
        candidates.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        Ok(candidates
            .first()
            .map(|(_, good, destination, _)| (good.clone(), destination.clone())))
    }
}

#[must_use]
pub fn ticks_for_distance(distance: f64, speed: f64) -> u32 {
    (distance / speed.max(f64::EPSILON)).ceil().max(1.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> ContentId {
        ContentId::new(value).unwrap()
    }

    #[test]
    fn ids_require_a_namespace() {
        assert!(ContentId::new("system").is_err());
        assert_eq!(id("core:system").as_str(), "core:system");
    }

    #[test]
    fn distance_and_duration_are_derived() {
        let a = Position3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let b = Position3 {
            x: 3.0,
            y: 4.0,
            z: 12.0,
        };
        assert_eq!(a.distance(b), 13.0);
        assert_eq!(ticks_for_distance(13.0, 5.0), 3);
    }

    #[test]
    fn rejected_overflow_does_not_mutate_transaction_state() {
        let ore = id("core:ore");
        let systems = (0..2)
            .map(|i| SystemDefinition {
                id: id(&format!("core:s{i}")),
                name: format!("S{i}"),
                position: Position3 {
                    x: f64::from(i),
                    y: 0.0,
                    z: 0.0,
                },
                inventory: BTreeMap::from([(ore.clone(), 10)]),
                targets: BTreeMap::from([(ore.clone(), 10)]),
                currency: if i == 0 { Money(i64::MAX) } else { Money(100) },
                recipes: vec![],
                sources: vec![],
            })
            .collect();
        let definition = GameDefinition {
            goods: vec![GoodDefinition {
                id: ore.clone(),
                name: "Ore".into(),
                category: GoodCategory::Raw,
                base_price: Money(10),
            }],
            recipes: vec![],
            systems,
            traders: vec![TraderDefinition {
                id: id("core:player"),
                name: "Player".into(),
                system: id("core:s0"),
                currency: Money(100),
                cargo_capacity: 10,
                speed: 1.0,
                player: true,
            }],
        };
        let mut session = GameSession::new(definition).unwrap();
        let before = format!("{:?}", session.snapshot());
        assert_eq!(
            session.submit(GameCommand::Buy {
                good: ore,
                quantity: 1
            }),
            Err(CoreError::Overflow)
        );
        assert_eq!(format!("{:?}", session.snapshot()), before);
    }

    #[test]
    fn rejected_sell_overflow_does_not_mutate_transaction_state() {
        let ore = id("core:ore");
        let mut definition = minimal_definition(vec![GoodDefinition {
            id: ore.clone(),
            name: "Ore".into(),
            category: GoodCategory::Raw,
            base_price: Money(10),
        }]);
        definition.systems[0]
            .inventory
            .insert(ore.clone(), u32::MAX);
        let mut session = GameSession::new(definition).unwrap();
        let player = session.player_entity().unwrap();
        let mut trader = session.world.get_mut::<Trader>(player).unwrap();
        trader.currency = Money(i64::MAX);
        trader.cargo.insert(ore.clone(), 1);
        drop(trader);
        let before = format!("{:?}", session.snapshot());
        assert_eq!(
            session.submit(GameCommand::Sell {
                good: ore,
                quantity: 1
            }),
            Err(CoreError::Overflow)
        );
        assert_eq!(format!("{:?}", session.snapshot()), before);
    }

    #[test]
    fn recipe_overflow_preserves_inputs() {
        let input = id("core:input");
        let output = id("core:output");
        let recipe_id = id("core:recipe");
        let mut definition = minimal_definition(vec![
            GoodDefinition {
                id: input.clone(),
                name: "Input".into(),
                category: GoodCategory::Raw,
                base_price: Money(1),
            },
            GoodDefinition {
                id: output.clone(),
                name: "Output".into(),
                category: GoodCategory::Primary,
                base_price: Money(1),
            },
        ]);
        definition.recipes.push(RecipeDefinition {
            id: recipe_id.clone(),
            name: "Recipe".into(),
            layer: RecipeLayer::Primary,
            inputs: vec![GoodAmount {
                good: input.clone(),
                quantity: 1,
            }],
            outputs: vec![GoodAmount {
                good: output.clone(),
                quantity: 1,
            }],
        });
        definition.systems[0].inventory = BTreeMap::from([(input, 1), (output, u32::MAX)]);
        definition.systems[0].recipes.push(recipe_id);
        let mut session = GameSession::new(definition).unwrap();
        let before = format!("{:?}", session.snapshot());
        assert_eq!(session.step(), Err(CoreError::Overflow));
        assert_eq!(format!("{:?}", session.snapshot()), before);
    }

    #[test]
    fn source_overflow_preserves_all_markets() {
        let ore = id("core:ore");
        let mut definition = minimal_definition(vec![GoodDefinition {
            id: ore.clone(),
            name: "Ore".into(),
            category: GoodCategory::Raw,
            base_price: Money(1),
        }]);
        definition.systems[0]
            .inventory
            .insert(ore.clone(), u32::MAX);
        definition.systems[0].sources.push(SourceDefinition {
            good: ore,
            quantity_per_tick: 1,
        });
        let mut session = GameSession::new(definition).unwrap();
        let before = format!("{:?}", session.snapshot());
        assert_eq!(session.step(), Err(CoreError::Overflow));
        assert_eq!(format!("{:?}", session.snapshot()), before);
    }

    fn minimal_definition(goods: Vec<GoodDefinition>) -> GameDefinition {
        let systems = (0..2)
            .map(|i| SystemDefinition {
                id: id(&format!("core:s{i}")),
                name: format!("S{i}"),
                position: Position3 {
                    x: f64::from(i),
                    y: 0.0,
                    z: 0.0,
                },
                inventory: BTreeMap::new(),
                targets: BTreeMap::new(),
                currency: Money(100),
                recipes: vec![],
                sources: vec![],
            })
            .collect();
        GameDefinition {
            goods,
            recipes: vec![],
            systems,
            traders: vec![TraderDefinition {
                id: id("core:player"),
                name: "Player".into(),
                system: id("core:s0"),
                currency: Money(100),
                cargo_capacity: 10,
                speed: 1.0,
                player: true,
            }],
        }
    }

    #[test]
    fn graph_finds_multi_hop_path() {
        let systems = (0..5)
            .map(|i| SystemDefinition {
                id: id(&format!("core:s{i}")),
                name: format!("S{i}"),
                position: Position3 {
                    x: f64::from(i) * 10.0,
                    y: 0.0,
                    z: 0.0,
                },
                inventory: BTreeMap::new(),
                targets: BTreeMap::new(),
                currency: Money(0),
                recipes: vec![],
                sources: vec![],
            })
            .collect::<Vec<_>>();
        let graph = SystemGraph::build(&systems).unwrap();
        let (path, distance) = graph.shortest_path(&id("core:s0"), &id("core:s4")).unwrap();
        assert_eq!(path.first(), Some(&id("core:s0")));
        assert_eq!(path.last(), Some(&id("core:s4")));
        assert_eq!(distance, 40.0);
    }
}
