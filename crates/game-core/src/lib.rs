//! Headless, deterministic origin-and-frontier simulation substrate.

use bevy_ecs::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use thiserror::Error;

pub const ENERGY_ID: &str = "core:energy";

/// Stable, namespace-qualified content identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentId(String);

impl ContentId {
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        let Some((namespace, path)) = value.split_once(':') else {
            return Err(CoreError::InvalidId(value));
        };
        if namespace.is_empty()
            || path.is_empty()
            || !value.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, ':' | '_')
            })
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
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Three-dimensional position in prototype distance units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Position3 {
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    #[must_use]
    pub fn distance(self, other: Self) -> f64 {
        let x = self.x - other.x;
        let y = self.y - other.y;
        let z = self.z - other.z;
        x.hypot(y).hypot(z)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceDefinition {
    pub id: ContentId,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocationDefinition {
    pub id: ContentId,
    pub name: String,
    pub position: Position3,
}

/// Checked physical quantities. At runtime a system owns its available store;
/// queue items use the same type for committed construction resources.
#[derive(Component, Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceStore {
    pub quantities: BTreeMap<ContentId, u64>,
}

impl ResourceStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn quantity(&self, resource: &ContentId) -> u64 {
        self.quantities.get(resource).copied().unwrap_or(0)
    }

    pub fn set(&mut self, resource: ContentId, quantity: u64) {
        self.quantities.insert(resource, quantity);
    }

    pub fn checked_total(&self) -> Result<u64, CoreError> {
        self.quantities.values().try_fold(0_u64, |total, quantity| {
            total.checked_add(*quantity).ok_or(CoreError::Overflow)
        })
    }
}

impl FromIterator<(ContentId, u64)> for ResourceStore {
    fn from_iter<T: IntoIterator<Item = (ContentId, u64)>>(iter: T) -> Self {
        Self {
            quantities: iter.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginCommunityDefinition {
    pub id: ContentId,
    pub location: ContentId,
    pub population: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceDepositDefinition {
    pub id: ContentId,
    pub location: ContentId,
    pub resource: ContentId,
    pub quantity: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReclaimableSiteDefinition {
    pub id: ContentId,
    pub location: ContentId,
}

/// An undirected explicit topology edge. Runtime construction canonicalizes
/// `from` and `to` by stable ID.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TopologyEdge {
    pub from: ContentId,
    pub to: ContentId,
}

impl TopologyEdge {
    #[must_use]
    pub fn new(from: ContentId, to: ContentId) -> Self {
        Self { from, to }
    }

    #[must_use]
    pub fn endpoints(&self) -> (&ContentId, &ContentId) {
        (&self.from, &self.to)
    }

    fn canonicalized(mut self) -> Self {
        if self.to < self.from {
            std::mem::swap(&mut self.from, &mut self.to);
        }
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TopologyDefinition {
    pub edges: Vec<TopologyEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemDefinition {
    pub location: ContentId,
    pub stocks: ResourceStore,
    pub resource_engine: Option<ResourceEngineDefinition>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldDefinition {
    pub resources: Vec<ResourceDefinition>,
    pub locations: Vec<LocationDefinition>,
    pub origin: OriginCommunityDefinition,
    pub systems: Vec<SystemDefinition>,
    pub deposits: Vec<ResourceDepositDefinition>,
    pub sites: Vec<ReclaimableSiteDefinition>,
    pub topology: TopologyDefinition,
}

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub struct StableId(pub ContentId);

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub struct DisplayName(pub String);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocationMarker;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct SpatialPosition(pub Position3);

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub struct Community {
    pub id: ContentId,
    pub population: u64,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct OriginMarker;

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub struct ReclaimableSite {
    pub location: ContentId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TopologyEdgeSnapshot {
    pub from: ContentId,
    pub to: ContentId,
    pub distance: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Topology {
    pub edges: Vec<TopologyEdgeSnapshot>,
    adjacency: BTreeMap<ContentId, Vec<(ContentId, f64)>>,
}

impl Topology {
    #[must_use]
    pub fn neighbors(&self, location: &ContentId) -> &[(ContentId, f64)] {
        self.adjacency
            .get(location)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TopologyPath {
    pub locations: Vec<ContentId>,
    pub distance: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunitySnapshot {
    pub id: ContentId,
    pub location: ContentId,
    pub population: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocationSnapshot {
    pub id: ContentId,
    pub name: String,
    pub position: Position3,
    pub community: Option<CommunitySnapshot>,
    pub is_origin: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldSnapshot {
    pub resources: Vec<ResourceDefinition>,
    pub locations: Vec<LocationSnapshot>,
    pub origin: CommunitySnapshot,
    pub systems: Vec<SystemSnapshot>,
    pub deposits: Vec<ResourceDepositDefinition>,
    pub sites: Vec<ReclaimableSiteDefinition>,
    pub topology: Vec<TopologyEdgeSnapshot>,
}

/// Condition of an installed development. Only functional developments have
/// Stage 4 consequences.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DevelopmentCondition {
    Functional,
    Damaged,
    Ruined,
}

/// The closed Stage 4 infrastructure catalog.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DevelopmentRole {
    Collector,
    Battery,
    Extractor,
    Refinery,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentDefinition {
    pub id: ContentId,
    pub role: DevelopmentRole,
    pub condition: DevelopmentCondition,
    pub extractor_deposit: Option<ContentId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentSlotDefinition {
    pub id: ContentId,
    pub development: Option<DevelopmentDefinition>,
}

/// Bodies deliberately have no type, statistic, compatibility, or bonus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyDefinition {
    pub id: ContentId,
    pub name: String,
    pub slots: Vec<DevelopmentSlotDefinition>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructionRecipe {
    pub cost: ResourceStore,
    pub required_work: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractorParameters {
    pub energy_upkeep: u64,
    pub cycle_duration: u64,
    pub ore_output: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefineryParameters {
    pub energy_upkeep: u64,
    pub cycle_duration: u64,
    pub ore_input: u64,
    pub alloy_output: u64,
}

/// Validated designer-authored Stage 4 tuning. The engine contains ordering and
/// consequence kinds, never fixture balance constants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceEngineConfig {
    pub energy_resource: ContentId,
    pub ore_resource: ContentId,
    pub alloy_resource: ContentId,
    pub life_support_per_population: u64,
    pub origin_construction_work: u64,
    pub intrinsic_energy_capacity: u64,
    pub battery_energy_capacity: u64,
    pub collector_recipe: ConstructionRecipe,
    pub battery_recipe: ConstructionRecipe,
    pub extractor_recipe: ConstructionRecipe,
    pub refinery_recipe: ConstructionRecipe,
    pub extractor: ExtractorParameters,
    pub refinery: RefineryParameters,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceEngineDefinition {
    pub collector_energy_profile: [u64; 10],
    pub bodies: Vec<BodyDefinition>,
    pub config: ResourceEngineConfig,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SimulationTime {
    pub tick: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProductionCycle {
    pub progress: u64,
    pub committed_inputs: ResourceStore,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentState {
    pub definition: DevelopmentDefinition,
    pub cycle: ProductionCycle,
    pub required_cycle_duration: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentSlotState {
    pub id: ContentId,
    pub development: Option<DevelopmentState>,
    pub reserved_by: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyState {
    pub id: ContentId,
    pub name: String,
    pub slots: Vec<DevelopmentSlotState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructionItem {
    pub sequence: u64,
    pub development_id: ContentId,
    pub body: ContentId,
    pub slot: ContentId,
    pub role: DevelopmentRole,
    pub extractor_deposit: Option<ContentId>,
    pub required_work: u64,
    pub work_applied: u64,
    pub committed_resources: ResourceStore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnergyOverflowCause {
    Retention,
    Transfer,
    CancellationRefund,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnergyOverflowEvidence {
    pub tick: u64,
    pub cause: EnergyOverflowCause,
    pub quantity: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EnergyOverflowAccounting {
    pub cumulative: u64,
    pub last_tick_retention: u64,
    pub evidence: Vec<EnergyOverflowEvidence>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LifeSupportEvidence {
    pub required_energy: u64,
    pub paid_energy: u64,
    pub unpaid_energy: u64,
    pub supported_population: u64,
    pub underserved_population: u64,
    pub construction_work: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceAccounting {
    pub construction_spent: ResourceStore,
    pub operation_spent: ResourceStore,
    pub produced: ResourceStore,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemSnapshot {
    pub location: ContentId,
    pub stocks: ResourceStore,
    pub resource_engine: Option<ResourceEngineSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceEngineSnapshot {
    pub location: ContentId,
    pub time: SimulationTime,
    pub seasonal_phase: usize,
    pub collector_energy_profile: [u64; 10],
    pub config: ResourceEngineConfig,
    pub stocks: ResourceStore,
    pub bodies: Vec<BodyState>,
    pub deposits: Vec<ResourceDepositDefinition>,
    pub construction_queue: Vec<ConstructionItem>,
    pub next_construction_sequence: u64,
    pub life_support: LifeSupportEvidence,
    pub energy_capacity: u64,
    pub energy_headroom: u64,
    pub energy_overflow: EnergyOverflowAccounting,
    pub accounting: ResourceAccounting,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResourceEngineState {
    location: ContentId,
    collector_energy_profile: [u64; 10],
    bodies: Vec<BodyState>,
    config: ResourceEngineConfig,
    time: SimulationTime,
    construction_queue: Vec<ConstructionItem>,
    next_construction_sequence: u64,
    life_support: LifeSupportEvidence,
    overflow: EnergyOverflowAccounting,
    accounting: ResourceAccounting,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SystemState {
    stocks: ResourceStore,
    resource_engine: Option<ResourceEngineState>,
}

/// Headless owner of the ECS world. Construction validates and normalizes the
/// complete definition before creating any entities.
pub struct WorldState {
    world: World,
    resources: Vec<ResourceDefinition>,
    locations: Vec<LocationDefinition>,
    deposits: Vec<ResourceDepositDefinition>,
    sites: Vec<ReclaimableSiteDefinition>,
    topology: Topology,
    location_entities: BTreeMap<ContentId, Entity>,
    origin_entity: Entity,
    systems: BTreeMap<ContentId, SystemState>,
}

impl WorldState {
    pub fn new(definition: WorldDefinition) -> Result<Self, CoreError> {
        let definition = validate_and_normalize(definition)?;

        let mut world = World::new();
        let mut location_entities = BTreeMap::new();
        for location in &definition.locations {
            let entity = world
                .spawn((
                    StableId(location.id.clone()),
                    DisplayName(location.name.clone()),
                    LocationMarker,
                    SpatialPosition(location.position),
                ))
                .id();
            location_entities.insert(location.id.clone(), entity);
        }

        for site in &definition.sites {
            world.spawn((
                StableId(site.id.clone()),
                ReclaimableSite {
                    location: site.location.clone(),
                },
            ));
        }

        let origin_entity = location_entities[&definition.origin.location];
        world.entity_mut(origin_entity).insert((
            Community {
                id: definition.origin.id.clone(),
                population: definition.origin.population,
            },
            OriginMarker,
        ));
        Ok(Self {
            world,
            resources: definition.resources,
            locations: definition.locations,
            deposits: definition.deposits,
            sites: definition.sites,
            topology: definition.topology,
            location_entities,
            origin_entity,
            systems: definition.systems,
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> WorldSnapshot {
        let locations = self
            .locations
            .iter()
            .map(|location| {
                let entity = self.location_entities[&location.id];
                let community =
                    self.world
                        .get::<Community>(entity)
                        .map(|community| CommunitySnapshot {
                            id: community.id.clone(),
                            location: location.id.clone(),
                            population: community.population,
                        });
                LocationSnapshot {
                    id: location.id.clone(),
                    name: location.name.clone(),
                    position: location.position,
                    community,
                    is_origin: self.world.get::<OriginMarker>(entity).is_some(),
                }
            })
            .collect::<Vec<_>>();
        let origin = locations
            .iter()
            .find_map(|location| location.community.clone())
            .expect("validated worlds contain exactly one origin community");

        let systems = self
            .systems
            .iter()
            .map(|(location, system)| {
                snapshot_system(location, system, &self.deposits)
                    .expect("validated systems remain snapshotable")
            })
            .collect();
        WorldSnapshot {
            resources: self.resources.clone(),
            locations,
            origin,
            systems,
            deposits: self.deposits.clone(),
            sites: self.sites.clone(),
            topology: self.topology.edges.clone(),
        }
    }

    #[must_use]
    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    /// Returns the available physical stocks owned by one authored system.
    pub fn system_stocks(&self, location: &ContentId) -> Result<&ResourceStore, CoreError> {
        Ok(&self
            .systems
            .get(location)
            .ok_or_else(|| CoreError::UnknownSystem(location.clone()))?
            .stocks)
    }

    /// Snapshots one system whether or not it has a resource engine.
    pub fn system_snapshot(&self, location: &ContentId) -> Result<SystemSnapshot, CoreError> {
        let system = self
            .systems
            .get(location)
            .ok_or_else(|| CoreError::UnknownSystem(location.clone()))?;
        snapshot_system(location, system, &self.deposits)
    }

    /// Snapshots the Stage 4 engine authored for one system.
    pub fn strategic_snapshot(
        &self,
        location: &ContentId,
    ) -> Result<ResourceEngineSnapshot, CoreError> {
        let system = self
            .systems
            .get(location)
            .ok_or_else(|| CoreError::UnknownSystem(location.clone()))?;
        let engine = system
            .resource_engine
            .as_ref()
            .ok_or(CoreError::MissingResourceEnginePrerequisite)?;
        snapshot_engine(engine, &system.stocks, &self.deposits)
    }

    /// Commits a complete construction cost and reserves a generic body slot.
    /// The operation is computed against clones and commits only after all
    /// resource, sequence, slot, and deposit checks pass.
    pub fn enqueue_construction(
        &mut self,
        system_location: &ContentId,
        body: &ContentId,
        slot: &ContentId,
        role: DevelopmentRole,
        extractor_deposit: Option<&ContentId>,
    ) -> Result<u64, CoreError> {
        let mut system = self
            .systems
            .get(system_location)
            .cloned()
            .ok_or_else(|| CoreError::UnknownSystem(system_location.clone()))?;
        let mut engine = system
            .resource_engine
            .clone()
            .ok_or(CoreError::MissingResourceEnginePrerequisite)?;
        let mut stocks = system.stocks.clone();
        let sequence = engine.next_construction_sequence;
        let next_sequence = sequence.checked_add(1).ok_or(CoreError::Overflow)?;
        let development_id = constructed_development_id(&engine.location, sequence)?;
        let id_is_installed = engine
            .bodies
            .iter()
            .flat_map(|body| &body.slots)
            .filter_map(|slot| slot.development.as_ref())
            .any(|development| development.definition.id == development_id);
        let id_is_queued = engine
            .construction_queue
            .iter()
            .any(|item| item.development_id == development_id);
        if id_is_installed || id_is_queued {
            return Err(CoreError::DuplicateDevelopmentId(development_id));
        }
        let recipe = recipe_for(&engine.config, role).clone();

        let slot_state = find_slot(&engine.bodies, body, slot)?;
        if slot_state.development.is_some() || slot_state.reserved_by.is_some() {
            return Err(CoreError::DevelopmentSlotUnavailable {
                body: body.clone(),
                slot: slot.clone(),
            });
        }
        validate_extractor_target(&engine, &self.deposits, role, extractor_deposit)?;
        for (resource, quantity) in &recipe.cost.quantities {
            let available = stocks.quantity(resource);
            let after = available.checked_sub(*quantity).ok_or_else(|| {
                CoreError::InsufficientResource {
                    resource: resource.clone(),
                    available,
                    requested: *quantity,
                }
            })?;
            stocks.set(resource.clone(), after);
        }
        find_slot_mut(&mut engine.bodies, body, slot)?.reserved_by = Some(sequence);
        engine.construction_queue.push(ConstructionItem {
            sequence,
            development_id,
            body: body.clone(),
            slot: slot.clone(),
            role,
            extractor_deposit: extractor_deposit.cloned(),
            required_work: recipe.required_work,
            work_applied: 0,
            committed_resources: recipe.cost,
        });
        engine.next_construction_sequence = next_sequence;
        system.resource_engine = Some(engine);
        system.stocks = stocks;
        self.systems.insert(system_location.clone(), system);
        Ok(sequence)
    }

    /// Cancels one unbegun queue item. Energy that cannot be retained is
    /// reconciled as cancellation-refund overflow rather than blocking.
    pub fn cancel_construction(
        &mut self,
        system_location: &ContentId,
        sequence: u64,
    ) -> Result<(), CoreError> {
        let mut system = self
            .systems
            .get(system_location)
            .cloned()
            .ok_or_else(|| CoreError::UnknownSystem(system_location.clone()))?;
        let mut engine = system
            .resource_engine
            .clone()
            .ok_or(CoreError::MissingResourceEnginePrerequisite)?;
        let mut stocks = system.stocks.clone();
        let index = engine
            .construction_queue
            .iter()
            .position(|item| item.sequence == sequence)
            .ok_or(CoreError::UnknownConstructionSequence(sequence))?;
        let item = engine.construction_queue[index].clone();
        if item.work_applied != 0 {
            return Err(CoreError::ConstructionAlreadyBegun(sequence));
        }
        let capacity = energy_capacity(&engine)?;
        let energy_id = engine.config.energy_resource.clone();
        let mut refund_overflow = 0;
        for (resource, quantity) in &item.committed_resources.quantities {
            let before = stocks.quantity(resource);
            if *resource == energy_id {
                let headroom = capacity.saturating_sub(before);
                let retained = (*quantity).min(headroom);
                refund_overflow = *quantity - retained;
                let after = before.checked_add(retained).ok_or(CoreError::Overflow)?;
                stocks.set(resource.clone(), after);
            } else {
                let after = before.checked_add(*quantity).ok_or(CoreError::Overflow)?;
                stocks.set(resource.clone(), after);
            }
        }
        if refund_overflow != 0 {
            record_overflow(
                &mut engine,
                EnergyOverflowCause::CancellationRefund,
                refund_overflow,
            )?;
        }
        find_slot_mut(&mut engine.bodies, &item.body, &item.slot)?.reserved_by = None;
        engine.construction_queue.remove(index);
        system.resource_engine = Some(engine);
        system.stocks = stocks;
        self.systems.insert(system_location.clone(), system);
        Ok(())
    }

    /// Capacity-aware receipt into the system. Generic transfer semantics stay
    /// exact: the source decrease and movement ledger both equal retained plus
    /// explicitly overflowed quantity.
    pub fn transfer_resource_to_system(
        &mut self,
        system_location: &ContentId,
        source: &mut ResourceStore,
        ledger: &mut ResourceFlowLedger,
        resource: &ContentId,
        quantity: u64,
    ) -> Result<(), CoreError> {
        if quantity == 0 {
            return Err(CoreError::ZeroResourceTransfer);
        }
        if !self
            .resources
            .iter()
            .any(|candidate| candidate.id == *resource)
        {
            return Err(CoreError::UnknownTransferResource(resource.clone()));
        }
        let mut system = self
            .systems
            .get(system_location)
            .cloned()
            .ok_or_else(|| CoreError::UnknownSystem(system_location.clone()))?;
        let mut engine = system
            .resource_engine
            .clone()
            .ok_or(CoreError::MissingResourceEnginePrerequisite)?;
        let mut next_source = source.clone();
        let mut stocks = system.stocks.clone();
        let mut next_ledger = ledger.clone();
        let source_before = next_source.quantity(resource);
        let source_after =
            source_before
                .checked_sub(quantity)
                .ok_or_else(|| CoreError::InsufficientResource {
                    resource: resource.clone(),
                    available: source_before,
                    requested: quantity,
                })?;
        let moved_after = next_ledger
            .quantity_moved(resource)
            .checked_add(quantity)
            .ok_or(CoreError::Overflow)?;
        let mut retained = quantity;
        let mut overflowed = 0;
        if *resource == engine.config.energy_resource {
            let capacity = energy_capacity(&engine)?;
            let available = stocks.quantity(resource);
            let headroom = capacity.saturating_sub(available);
            retained = quantity.min(headroom);
            overflowed = quantity - retained;
        }
        let destination_after = stocks
            .quantity(resource)
            .checked_add(retained)
            .ok_or(CoreError::Overflow)?;
        if overflowed != 0 {
            record_overflow(&mut engine, EnergyOverflowCause::Transfer, overflowed)?;
        }
        next_source.set(resource.clone(), source_after);
        stocks.set(resource.clone(), destination_after);
        next_ledger.moved.insert(resource.clone(), moved_after);
        *source = next_source;
        *ledger = next_ledger;
        system.stocks = stocks;
        system.resource_engine = Some(engine);
        self.systems.insert(system_location.clone(), system);
        Ok(())
    }

    /// Runs one complete deterministic month. Routine shortages are outcomes;
    /// malformed state or checked-arithmetic failure rejects without mutation.
    pub fn advance_tick(&mut self) -> Result<ResourceEngineSnapshot, CoreError> {
        let origin_location = self
            .world
            .get::<StableId>(self.origin_entity)
            .expect("origin is a validated location")
            .0
            .clone();
        let mut system = self
            .systems
            .get(&origin_location)
            .cloned()
            .ok_or(CoreError::MissingResourceEnginePrerequisite)?;
        let mut engine = system
            .resource_engine
            .clone()
            .ok_or(CoreError::MissingResourceEnginePrerequisite)?;
        let mut stocks = system.stocks.clone();
        let mut deposits = self.deposits.clone();
        let population = self
            .world
            .get::<Community>(self.origin_entity)
            .expect("origin community is constructed atomically")
            .population;
        let origin_work = engine.config.origin_construction_work;
        advance_engine_tick(
            &mut engine,
            &mut stocks,
            &mut deposits,
            population,
            origin_work,
        )?;
        let snapshot = snapshot_engine(&engine, &stocks, &deposits)?;
        system.resource_engine = Some(engine);
        system.stocks = stocks;
        self.systems.insert(origin_location, system);
        self.deposits = deposits;
        Ok(snapshot)
    }

    /// Finds a deterministic minimum-distance route over explicit topology.
    pub fn shortest_path(
        &self,
        from: &ContentId,
        to: &ContentId,
    ) -> Result<Option<TopologyPath>, CoreError> {
        if !self.location_entities.contains_key(from) {
            return Err(CoreError::UnknownLocation(from.clone()));
        }
        if !self.location_entities.contains_key(to) {
            return Err(CoreError::UnknownLocation(to.clone()));
        }
        if from == to {
            return Ok(Some(TopologyPath {
                locations: vec![from.clone()],
                distance: 0.0,
            }));
        }

        let mut unsettled = self
            .location_entities
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut distance = self
            .location_entities
            .keys()
            .cloned()
            .map(|id| (id, f64::INFINITY))
            .collect::<BTreeMap<_, _>>();
        let mut previous = BTreeMap::<ContentId, ContentId>::new();
        distance.insert(from.clone(), 0.0);

        while !unsettled.is_empty() {
            let current = unsettled
                .iter()
                .min_by(|left, right| {
                    distance[*left]
                        .total_cmp(&distance[*right])
                        .then_with(|| left.cmp(right))
                })
                .cloned()
                .expect("non-empty set has a minimum");
            let current_distance = distance[&current];
            unsettled.remove(&current);
            if !current_distance.is_finite() {
                break;
            }
            if current == *to {
                break;
            }

            for (neighbor, edge_distance) in self.topology.neighbors(&current) {
                if !unsettled.contains(neighbor) {
                    continue;
                }
                let candidate = current_distance + edge_distance;
                if !candidate.is_finite() {
                    return Err(CoreError::Overflow);
                }
                let ordering = candidate.total_cmp(&distance[neighbor]);
                let improves_tie = ordering.is_eq()
                    && previous
                        .get(neighbor)
                        .is_none_or(|predecessor| current < *predecessor);
                if ordering.is_lt() || improves_tie {
                    distance.insert(neighbor.clone(), candidate);
                    previous.insert(neighbor.clone(), current.clone());
                }
            }
        }

        let total = distance[to];
        if !total.is_finite() {
            return Ok(None);
        }
        let mut locations = vec![to.clone()];
        while locations.last() != Some(from) {
            let predecessor = previous
                .get(locations.last().expect("path is non-empty"))
                .expect("finite destination distance has a predecessor")
                .clone();
            locations.push(predecessor);
        }
        locations.reverse();
        Ok(Some(TopologyPath {
            locations,
            distance: total,
        }))
    }

    pub fn topology_distance(
        &self,
        from: &ContentId,
        to: &ContentId,
    ) -> Result<Option<f64>, CoreError> {
        Ok(self.shortest_path(from, to)?.map(|path| path.distance))
    }
}

fn validate_resource_engine_definition(
    location: ContentId,
    mut definition: ResourceEngineDefinition,
    resources: &[ResourceDefinition],
    deposits: &[ResourceDepositDefinition],
    stocks: &ResourceStore,
) -> Result<ResourceEngineState, CoreError> {
    let resource_ids = resources
        .iter()
        .map(|resource| resource.id.clone())
        .collect::<BTreeSet<_>>();
    for resource in [
        &definition.config.energy_resource,
        &definition.config.ore_resource,
        &definition.config.alloy_resource,
    ] {
        if !resource_ids.contains(resource) {
            return Err(CoreError::UnknownEngineResource(resource.clone()));
        }
    }
    if definition.config.energy_resource.as_str() != ENERGY_ID {
        return Err(CoreError::InvalidResourceEngineConfig(
            "energy_resource must be core:energy".into(),
        ));
    }
    if definition.config.energy_resource == definition.config.ore_resource
        || definition.config.energy_resource == definition.config.alloy_resource
        || definition.config.ore_resource == definition.config.alloy_resource
    {
        return Err(CoreError::InvalidResourceEngineConfig(
            "energy, ore, and alloy resources must be distinct".into(),
        ));
    }
    if definition.config.life_support_per_population == 0
        || definition.config.origin_construction_work == 0
        || definition.config.intrinsic_energy_capacity == 0
        || definition.config.battery_energy_capacity == 0
        || definition.config.extractor.energy_upkeep == 0
        || definition.config.extractor.cycle_duration == 0
        || definition.config.refinery.energy_upkeep == 0
        || definition.config.refinery.cycle_duration == 0
        || definition.config.refinery.ore_input == 0
        || definition.config.refinery.alloy_output == 0
    {
        return Err(CoreError::InvalidResourceEngineConfig(
            "life support, origin construction work, capacities, upkeep, durations, and throughput must be nonzero".into(),
        ));
    }
    if definition.config.extractor.ore_output != 1 {
        return Err(CoreError::InvalidResourceEngineConfig(
            "Extractor ore output must equal 1".into(),
        ));
    }
    for (role, recipe) in [
        (
            DevelopmentRole::Collector,
            &definition.config.collector_recipe,
        ),
        (DevelopmentRole::Battery, &definition.config.battery_recipe),
        (
            DevelopmentRole::Extractor,
            &definition.config.extractor_recipe,
        ),
        (
            DevelopmentRole::Refinery,
            &definition.config.refinery_recipe,
        ),
    ] {
        if recipe.required_work == 0 {
            return Err(CoreError::InvalidConstructionRecipe {
                role,
                reason: "required work must be nonzero".into(),
            });
        }
        if recipe.cost.quantity(&definition.config.energy_resource) == 0 {
            return Err(CoreError::InvalidConstructionRecipe {
                role,
                reason: "Energy cost must be nonzero".into(),
            });
        }
        for resource in recipe.cost.quantities.keys() {
            if !resource_ids.contains(resource) {
                return Err(CoreError::UnknownEngineResource(resource.clone()));
            }
        }
    }
    for role in [
        DevelopmentRole::Collector,
        DevelopmentRole::Battery,
        DevelopmentRole::Extractor,
    ] {
        let recipe = recipe_for(&definition.config, role);
        if recipe.cost.quantity(&definition.config.alloy_resource) == 0
            || recipe.cost.quantity(&definition.config.ore_resource) != 0
        {
            return Err(CoreError::InvalidConstructionRecipe {
                role,
                reason: "must consume Alloy and never Ore".into(),
            });
        }
    }
    if definition
        .config
        .refinery_recipe
        .cost
        .quantity(&definition.config.ore_resource)
        == 0
        || definition
            .config
            .refinery_recipe
            .cost
            .quantity(&definition.config.alloy_resource)
            != 0
    {
        return Err(CoreError::InvalidConstructionRecipe {
            role: DevelopmentRole::Refinery,
            reason: "must consume Ore and never Alloy".into(),
        });
    }
    for resource in stocks.quantities.keys() {
        if !resource_ids.contains(resource) {
            return Err(CoreError::UnknownEngineResource(resource.clone()));
        }
    }

    definition
        .bodies
        .sort_by(|left, right| left.id.cmp(&right.id));
    reject_duplicate_ids(
        definition.bodies.iter().map(|body| &body.id),
        CoreError::DuplicateBodyId,
    )?;
    let mut development_ids = BTreeSet::new();
    let mut assigned_deposits = BTreeSet::new();
    let mut bodies = Vec::with_capacity(definition.bodies.len());
    for mut body in definition.bodies {
        body.slots.sort_by(|left, right| left.id.cmp(&right.id));
        reject_duplicate_ids(body.slots.iter().map(|slot| &slot.id), |slot| {
            CoreError::DuplicateSlotId {
                body: body.id.clone(),
                slot,
            }
        })?;
        let mut slots = Vec::with_capacity(body.slots.len());
        for slot in body.slots {
            let development = slot
                .development
                .map(|development| {
                    if !development_ids.insert(development.id.clone()) {
                        return Err(CoreError::DuplicateDevelopmentId(development.id));
                    }
                    validate_installed_development(
                        &development,
                        &location,
                        &definition.config,
                        deposits,
                        &mut assigned_deposits,
                    )?;
                    let required_cycle_duration =
                        cycle_duration(&definition.config, development.role);
                    Ok(DevelopmentState {
                        definition: development,
                        cycle: ProductionCycle::default(),
                        required_cycle_duration,
                    })
                })
                .transpose()?;
            slots.push(DevelopmentSlotState {
                id: slot.id,
                development,
                reserved_by: None,
            });
        }
        bodies.push(BodyState {
            id: body.id,
            name: body.name,
            slots,
        });
    }
    let engine = ResourceEngineState {
        location,
        collector_energy_profile: definition.collector_energy_profile,
        bodies,
        config: definition.config,
        time: SimulationTime::default(),
        construction_queue: Vec::new(),
        next_construction_sequence: 0,
        life_support: LifeSupportEvidence::default(),
        overflow: EnergyOverflowAccounting::default(),
        accounting: ResourceAccounting::default(),
    };
    let capacity = energy_capacity(&engine)?;
    let available = stocks.quantity(&engine.config.energy_resource);
    if available > capacity {
        return Err(CoreError::EnergyAboveCapacity {
            available,
            capacity,
        });
    }
    Ok(engine)
}

fn validate_installed_development(
    development: &DevelopmentDefinition,
    location: &ContentId,
    config: &ResourceEngineConfig,
    deposits: &[ResourceDepositDefinition],
    assigned: &mut BTreeSet<ContentId>,
) -> Result<(), CoreError> {
    match (development.role, development.extractor_deposit.as_ref()) {
        (DevelopmentRole::Extractor, Some(deposit_id)) => {
            let deposit = deposits
                .iter()
                .find(|deposit| &deposit.id == deposit_id)
                .ok_or_else(|| CoreError::UnknownExtractorDeposit(deposit_id.clone()))?;
            if &deposit.location != location || deposit.resource != config.ore_resource {
                return Err(CoreError::IncompatibleExtractorDeposit(deposit_id.clone()));
            }
            if !assigned.insert(deposit_id.clone()) {
                return Err(CoreError::ExtractorDepositAlreadyAssigned(
                    deposit_id.clone(),
                ));
            }
        }
        (DevelopmentRole::Extractor, None) => return Err(CoreError::ExtractorDepositRequired),
        (_, Some(_)) => return Err(CoreError::UnexpectedExtractorDeposit),
        (_, None) => {}
    }
    Ok(())
}

fn cycle_duration(config: &ResourceEngineConfig, role: DevelopmentRole) -> Option<u64> {
    match role {
        DevelopmentRole::Extractor => Some(config.extractor.cycle_duration),
        DevelopmentRole::Refinery => Some(config.refinery.cycle_duration),
        DevelopmentRole::Collector | DevelopmentRole::Battery => None,
    }
}

fn constructed_development_id(
    system_location: &ContentId,
    sequence: u64,
) -> Result<ContentId, CoreError> {
    ContentId::new(format!(
        "{}:development_{sequence}",
        system_location.as_str()
    ))
}

fn recipe_for(config: &ResourceEngineConfig, role: DevelopmentRole) -> &ConstructionRecipe {
    match role {
        DevelopmentRole::Collector => &config.collector_recipe,
        DevelopmentRole::Battery => &config.battery_recipe,
        DevelopmentRole::Extractor => &config.extractor_recipe,
        DevelopmentRole::Refinery => &config.refinery_recipe,
    }
}

fn find_slot<'a>(
    bodies: &'a [BodyState],
    body: &ContentId,
    slot: &ContentId,
) -> Result<&'a DevelopmentSlotState, CoreError> {
    bodies
        .iter()
        .find(|candidate| &candidate.id == body)
        .ok_or_else(|| CoreError::UnknownBody(body.clone()))?
        .slots
        .iter()
        .find(|candidate| &candidate.id == slot)
        .ok_or_else(|| CoreError::UnknownDevelopmentSlot {
            body: body.clone(),
            slot: slot.clone(),
        })
}

fn find_slot_mut<'a>(
    bodies: &'a mut [BodyState],
    body: &ContentId,
    slot: &ContentId,
) -> Result<&'a mut DevelopmentSlotState, CoreError> {
    bodies
        .iter_mut()
        .find(|candidate| &candidate.id == body)
        .ok_or_else(|| CoreError::UnknownBody(body.clone()))?
        .slots
        .iter_mut()
        .find(|candidate| &candidate.id == slot)
        .ok_or_else(|| CoreError::UnknownDevelopmentSlot {
            body: body.clone(),
            slot: slot.clone(),
        })
}

fn validate_extractor_target(
    engine: &ResourceEngineState,
    deposits: &[ResourceDepositDefinition],
    role: DevelopmentRole,
    requested: Option<&ContentId>,
) -> Result<(), CoreError> {
    if role != DevelopmentRole::Extractor {
        return if requested.is_some() {
            Err(CoreError::UnexpectedExtractorDeposit)
        } else {
            Ok(())
        };
    }
    let deposit_id = requested.ok_or(CoreError::ExtractorDepositRequired)?;
    let deposit = deposits
        .iter()
        .find(|deposit| &deposit.id == deposit_id)
        .ok_or_else(|| CoreError::UnknownExtractorDeposit(deposit_id.clone()))?;
    if deposit.location != engine.location || deposit.resource != engine.config.ore_resource {
        return Err(CoreError::IncompatibleExtractorDeposit(deposit_id.clone()));
    }
    let installed = engine
        .bodies
        .iter()
        .flat_map(|body| &body.slots)
        .any(|slot| {
            slot.development.as_ref().is_some_and(|development| {
                development.definition.role == DevelopmentRole::Extractor
                    && development.definition.extractor_deposit.as_ref() == Some(deposit_id)
            })
        });
    let queued = engine.construction_queue.iter().any(|item| {
        item.role == DevelopmentRole::Extractor
            && item.extractor_deposit.as_ref() == Some(deposit_id)
    });
    if installed || queued {
        return Err(CoreError::ExtractorDepositAlreadyAssigned(
            deposit_id.clone(),
        ));
    }
    Ok(())
}

fn energy_capacity(engine: &ResourceEngineState) -> Result<u64, CoreError> {
    let batteries = engine
        .bodies
        .iter()
        .flat_map(|body| &body.slots)
        .filter(|slot| {
            slot.development.as_ref().is_some_and(|development| {
                development.definition.role == DevelopmentRole::Battery
                    && development.definition.condition == DevelopmentCondition::Functional
            })
        })
        .count();
    let batteries = u64::try_from(batteries).map_err(|_| CoreError::Overflow)?;
    engine
        .config
        .battery_energy_capacity
        .checked_mul(batteries)
        .and_then(|added| engine.config.intrinsic_energy_capacity.checked_add(added))
        .ok_or(CoreError::Overflow)
}

fn snapshot_system(
    location: &ContentId,
    system: &SystemState,
    deposits: &[ResourceDepositDefinition],
) -> Result<SystemSnapshot, CoreError> {
    Ok(SystemSnapshot {
        location: location.clone(),
        stocks: system.stocks.clone(),
        resource_engine: system
            .resource_engine
            .as_ref()
            .map(|engine| snapshot_engine(engine, &system.stocks, deposits))
            .transpose()?,
    })
}

fn snapshot_engine(
    engine: &ResourceEngineState,
    stocks: &ResourceStore,
    deposits: &[ResourceDepositDefinition],
) -> Result<ResourceEngineSnapshot, CoreError> {
    let capacity = energy_capacity(engine)?;
    let available = stocks.quantity(&engine.config.energy_resource);
    Ok(ResourceEngineSnapshot {
        location: engine.location.clone(),
        time: engine.time,
        seasonal_phase: usize::try_from(engine.time.tick % 10).expect("phase is below ten"),
        collector_energy_profile: engine.collector_energy_profile,
        config: engine.config.clone(),
        stocks: stocks.clone(),
        bodies: engine.bodies.clone(),
        deposits: deposits
            .iter()
            .filter(|deposit| deposit.location == engine.location)
            .cloned()
            .collect(),
        construction_queue: engine.construction_queue.clone(),
        next_construction_sequence: engine.next_construction_sequence,
        life_support: engine.life_support.clone(),
        energy_capacity: capacity,
        energy_headroom: capacity.saturating_sub(available),
        energy_overflow: engine.overflow.clone(),
        accounting: engine.accounting.clone(),
    })
}

fn checked_add_store(
    store: &mut ResourceStore,
    resource: &ContentId,
    quantity: u64,
) -> Result<(), CoreError> {
    let after = store
        .quantity(resource)
        .checked_add(quantity)
        .ok_or(CoreError::Overflow)?;
    store.set(resource.clone(), after);
    Ok(())
}

fn checked_sub_store(
    store: &mut ResourceStore,
    resource: &ContentId,
    quantity: u64,
) -> Result<(), CoreError> {
    let available = store.quantity(resource);
    let after = available
        .checked_sub(quantity)
        .ok_or_else(|| CoreError::InsufficientResource {
            resource: resource.clone(),
            available,
            requested: quantity,
        })?;
    store.set(resource.clone(), after);
    Ok(())
}

fn record_overflow(
    engine: &mut ResourceEngineState,
    cause: EnergyOverflowCause,
    quantity: u64,
) -> Result<(), CoreError> {
    engine.overflow.cumulative = engine
        .overflow
        .cumulative
        .checked_add(quantity)
        .ok_or(CoreError::Overflow)?;
    engine.overflow.evidence.push(EnergyOverflowEvidence {
        tick: engine.time.tick,
        cause,
        quantity,
    });
    Ok(())
}

fn functional_coordinates(
    engine: &ResourceEngineState,
    role: DevelopmentRole,
) -> Vec<(usize, usize)> {
    engine
        .bodies
        .iter()
        .enumerate()
        .flat_map(|(body_index, body)| {
            body.slots
                .iter()
                .enumerate()
                .filter_map(move |(slot_index, slot)| {
                    slot.development.as_ref().and_then(|development| {
                        (development.definition.role == role
                            && development.definition.condition == DevelopmentCondition::Functional)
                            .then_some((body_index, slot_index))
                    })
                })
        })
        .collect()
}

fn advance_engine_tick(
    engine: &mut ResourceEngineState,
    stocks: &mut ResourceStore,
    deposits: &mut [ResourceDepositDefinition],
    population: u64,
    origin_construction_work: u64,
) -> Result<(), CoreError> {
    let phase = usize::try_from(engine.time.tick % 10).expect("phase is below ten");
    engine.overflow.last_tick_retention = 0;
    let energy_id = engine.config.energy_resource.clone();
    let ore_id = engine.config.ore_resource.clone();
    let alloy_id = engine.config.alloy_resource.clone();

    // Collector phase.
    let collectors = engine
        .bodies
        .iter()
        .flat_map(|body| &body.slots)
        .filter(|slot| {
            slot.development.as_ref().is_some_and(|development| {
                development.definition.role == DevelopmentRole::Collector
                    && development.definition.condition == DevelopmentCondition::Functional
            })
        })
        .count();
    let collectors = u64::try_from(collectors).map_err(|_| CoreError::Overflow)?;
    let collected = engine.collector_energy_profile[phase]
        .checked_mul(collectors)
        .ok_or(CoreError::Overflow)?;
    checked_add_store(stocks, &energy_id, collected)?;
    checked_add_store(&mut engine.accounting.produced, &energy_id, collected)?;

    // Life support has first claim.
    let required = engine
        .config
        .life_support_per_population
        .checked_mul(population)
        .ok_or(CoreError::Overflow)?;
    let paid = stocks.quantity(&energy_id).min(required);
    checked_sub_store(stocks, &energy_id, paid)?;
    let unpaid = required - paid;
    let supported = paid / engine.config.life_support_per_population;
    let underserved = population - supported;
    let work = supported
        .checked_add(origin_construction_work)
        .ok_or(CoreError::Overflow)?;
    engine.life_support = LifeSupportEvidence {
        required_energy: required,
        paid_energy: paid,
        unpaid_energy: unpaid,
        supported_population: supported,
        underserved_population: underserved,
        construction_work: work,
    };

    // Stable body/slot order is already normalized. Capture coordinates so
    // role phases can mutate independent cycle state without iterator-order
    // coupling.

    // Extractors.
    for (body_index, slot_index) in functional_coordinates(engine, DevelopmentRole::Extractor) {
        let deposit_id = engine.bodies[body_index].slots[slot_index]
            .development
            .as_ref()
            .and_then(|development| development.definition.extractor_deposit.clone())
            .ok_or(CoreError::ExtractorDepositRequired)?;
        let deposit_index = deposits
            .iter()
            .position(|deposit| deposit.id == deposit_id)
            .ok_or_else(|| CoreError::UnknownExtractorDeposit(deposit_id.clone()))?;
        if deposits[deposit_index].quantity < engine.config.extractor.ore_output
            || stocks.quantity(&energy_id) < engine.config.extractor.energy_upkeep
        {
            continue;
        }
        checked_sub_store(stocks, &energy_id, engine.config.extractor.energy_upkeep)?;
        checked_add_store(
            &mut engine.accounting.operation_spent,
            &energy_id,
            engine.config.extractor.energy_upkeep,
        )?;
        let cycle = &mut engine.bodies[body_index].slots[slot_index]
            .development
            .as_mut()
            .expect("coordinate selected an installed development")
            .cycle;
        cycle.progress = cycle.progress.checked_add(1).ok_or(CoreError::Overflow)?;
        if cycle.progress == engine.config.extractor.cycle_duration {
            deposits[deposit_index].quantity -= engine.config.extractor.ore_output;
            checked_add_store(stocks, &ore_id, engine.config.extractor.ore_output)?;
            checked_add_store(
                &mut engine.accounting.produced,
                &ore_id,
                engine.config.extractor.ore_output,
            )?;
            cycle.progress = 0;
        }
    }

    // Refineries independently commit input and retain it through pauses.
    for (body_index, slot_index) in functional_coordinates(engine, DevelopmentRole::Refinery) {
        let cycle = &engine.bodies[body_index].slots[slot_index]
            .development
            .as_ref()
            .expect("coordinate selected an installed development")
            .cycle;
        let idle = cycle.progress == 0 && cycle.committed_inputs.quantity(&ore_id) == 0;
        if stocks.quantity(&energy_id) < engine.config.refinery.energy_upkeep
            || (idle && stocks.quantity(&ore_id) < engine.config.refinery.ore_input)
        {
            continue;
        }
        checked_sub_store(stocks, &energy_id, engine.config.refinery.energy_upkeep)?;
        checked_add_store(
            &mut engine.accounting.operation_spent,
            &energy_id,
            engine.config.refinery.energy_upkeep,
        )?;
        let cycle = &mut engine.bodies[body_index].slots[slot_index]
            .development
            .as_mut()
            .expect("coordinate selected an installed development")
            .cycle;
        if idle {
            checked_sub_store(stocks, &ore_id, engine.config.refinery.ore_input)?;
            cycle
                .committed_inputs
                .set(ore_id.clone(), engine.config.refinery.ore_input);
        }
        cycle.progress = cycle.progress.checked_add(1).ok_or(CoreError::Overflow)?;
        if cycle.progress == engine.config.refinery.cycle_duration {
            checked_add_store(
                &mut engine.accounting.operation_spent,
                &ore_id,
                engine.config.refinery.ore_input,
            )?;
            checked_add_store(stocks, &alloy_id, engine.config.refinery.alloy_output)?;
            checked_add_store(
                &mut engine.accounting.produced,
                &alloy_id,
                engine.config.refinery.alloy_output,
            )?;
            cycle.progress = 0;
            cycle.committed_inputs = ResourceStore::new();
        }
    }

    // FIFO construction with same-tick overflow.
    let mut remaining_work = work;
    while remaining_work != 0 && !engine.construction_queue.is_empty() {
        let needed =
            engine.construction_queue[0].required_work - engine.construction_queue[0].work_applied;
        let applied = remaining_work.min(needed);
        engine.construction_queue[0].work_applied = engine.construction_queue[0]
            .work_applied
            .checked_add(applied)
            .ok_or(CoreError::Overflow)?;
        remaining_work -= applied;
        if engine.construction_queue[0].work_applied == engine.construction_queue[0].required_work {
            let item = engine.construction_queue.remove(0);
            for (resource, quantity) in &item.committed_resources.quantities {
                checked_add_store(
                    &mut engine.accounting.construction_spent,
                    resource,
                    *quantity,
                )?;
            }
            let required_cycle_duration = cycle_duration(&engine.config, item.role);
            let slot = find_slot_mut(&mut engine.bodies, &item.body, &item.slot)?;
            if slot.reserved_by != Some(item.sequence) || slot.development.is_some() {
                return Err(CoreError::InvalidConstructionReservation(item.sequence));
            }
            slot.development = Some(DevelopmentState {
                definition: DevelopmentDefinition {
                    id: item.development_id,
                    role: item.role,
                    condition: DevelopmentCondition::Functional,
                    extractor_deposit: item.extractor_deposit,
                },
                cycle: ProductionCycle::default(),
                required_cycle_duration,
            });
            slot.reserved_by = None;
        }
    }

    // Retention sees Batteries completed above.
    let capacity = energy_capacity(engine)?;
    let available = stocks.quantity(&energy_id);
    if available > capacity {
        let overflow = available - capacity;
        stocks.set(energy_id, capacity);
        engine.overflow.last_tick_retention = overflow;
        record_overflow(engine, EnergyOverflowCause::Retention, overflow)?;
    }
    engine.time.tick = engine.time.tick.checked_add(1).ok_or(CoreError::Overflow)?;
    Ok(())
}

struct ValidatedWorldDefinition {
    resources: Vec<ResourceDefinition>,
    locations: Vec<LocationDefinition>,
    origin: OriginCommunityDefinition,
    systems: BTreeMap<ContentId, SystemState>,
    deposits: Vec<ResourceDepositDefinition>,
    sites: Vec<ReclaimableSiteDefinition>,
    topology: Topology,
}

fn validate_and_normalize(
    mut definition: WorldDefinition,
) -> Result<ValidatedWorldDefinition, CoreError> {
    definition
        .resources
        .sort_by(|left, right| left.id.cmp(&right.id));
    definition
        .locations
        .sort_by(|left, right| left.id.cmp(&right.id));
    definition
        .systems
        .sort_by(|left, right| left.location.cmp(&right.location));
    definition
        .deposits
        .sort_by(|left, right| left.id.cmp(&right.id));
    definition
        .sites
        .sort_by(|left, right| left.id.cmp(&right.id));

    reject_duplicate_ids(
        definition.resources.iter().map(|resource| &resource.id),
        CoreError::DuplicateResourceId,
    )?;
    reject_duplicate_ids(
        definition.locations.iter().map(|location| &location.id),
        CoreError::DuplicateLocationId,
    )?;
    reject_duplicate_ids(
        definition.systems.iter().map(|system| &system.location),
        CoreError::DuplicateSystemLocation,
    )?;
    reject_duplicate_ids(
        definition.deposits.iter().map(|deposit| &deposit.id),
        CoreError::DuplicateDepositId,
    )?;
    reject_duplicate_ids(
        definition.sites.iter().map(|site| &site.id),
        CoreError::DuplicateSiteId,
    )?;

    let resource_ids = definition
        .resources
        .iter()
        .map(|resource| resource.id.clone())
        .collect::<BTreeSet<_>>();
    let positions = definition
        .locations
        .iter()
        .map(|location| {
            if !location.position.is_finite() {
                Err(CoreError::NonFinitePosition(location.id.clone()))
            } else {
                Ok((location.id.clone(), location.position))
            }
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;

    if !positions.contains_key(&definition.origin.location) {
        return Err(CoreError::UnknownOriginLocation(
            definition.origin.location.clone(),
        ));
    }
    for system in &definition.systems {
        if !positions.contains_key(&system.location) {
            return Err(CoreError::UnknownSystemLocation(system.location.clone()));
        }
        for resource in system.stocks.quantities.keys() {
            if !resource_ids.contains(resource) {
                return Err(CoreError::UnknownSystemStockResource {
                    location: system.location.clone(),
                    resource: resource.clone(),
                });
            }
        }
    }

    for deposit in &definition.deposits {
        if deposit.quantity == 0 {
            return Err(CoreError::ZeroDepositQuantity(deposit.id.clone()));
        }
        if !positions.contains_key(&deposit.location) {
            return Err(CoreError::UnknownDepositLocation {
                deposit: deposit.id.clone(),
                location: deposit.location.clone(),
            });
        }
        if !resource_ids.contains(&deposit.resource) {
            return Err(CoreError::UnknownDepositResource {
                deposit: deposit.id.clone(),
                resource: deposit.resource.clone(),
            });
        }
    }
    for site in &definition.sites {
        if !positions.contains_key(&site.location) {
            return Err(CoreError::UnknownSiteLocation {
                site: site.id.clone(),
                location: site.location.clone(),
            });
        }
    }

    let systems = definition
        .systems
        .into_iter()
        .map(|system| {
            let location = system.location;
            let resource_engine = system
                .resource_engine
                .map(|engine| {
                    validate_resource_engine_definition(
                        location.clone(),
                        engine,
                        &definition.resources,
                        &definition.deposits,
                        &system.stocks,
                    )
                })
                .transpose()?;
            Ok((
                location,
                SystemState {
                    stocks: system.stocks,
                    resource_engine,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, CoreError>>()?;

    let mut edges = definition
        .topology
        .edges
        .into_iter()
        .map(TopologyEdge::canonicalized)
        .collect::<Vec<_>>();
    edges.sort();
    let mut snapshots = Vec::with_capacity(edges.len());
    let mut adjacency = positions
        .keys()
        .cloned()
        .map(|id| (id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    let mut previous = None::<TopologyEdge>;
    for edge in edges {
        if edge.from == edge.to {
            return Err(CoreError::TopologySelfEdge(edge.from));
        }
        if previous.as_ref() == Some(&edge) {
            return Err(CoreError::DuplicateTopologyEdge {
                from: edge.from,
                to: edge.to,
            });
        }
        let Some(from_position) = positions.get(&edge.from) else {
            return Err(CoreError::UnknownTopologyEndpoint(edge.from));
        };
        let Some(to_position) = positions.get(&edge.to) else {
            return Err(CoreError::UnknownTopologyEndpoint(edge.to));
        };
        let distance = from_position.distance(*to_position);
        if !distance.is_finite() {
            return Err(CoreError::NonFiniteTopologyDistance {
                from: edge.from,
                to: edge.to,
            });
        }
        adjacency
            .get_mut(&edge.from)
            .expect("edge endpoint was validated")
            .push((edge.to.clone(), distance));
        adjacency
            .get_mut(&edge.to)
            .expect("edge endpoint was validated")
            .push((edge.from.clone(), distance));
        snapshots.push(TopologyEdgeSnapshot {
            from: edge.from.clone(),
            to: edge.to.clone(),
            distance,
        });
        previous = Some(edge);
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_by(|left, right| left.0.cmp(&right.0));
    }

    Ok(ValidatedWorldDefinition {
        resources: definition.resources,
        locations: definition.locations,
        origin: definition.origin,
        systems,
        deposits: definition.deposits,
        sites: definition.sites,
        topology: Topology {
            edges: snapshots,
            adjacency,
        },
    })
}

fn reject_duplicate_ids<'a>(
    ids: impl IntoIterator<Item = &'a ContentId>,
    error: impl Fn(ContentId) -> CoreError,
) -> Result<(), CoreError> {
    let mut previous = None;
    for id in ids {
        if previous == Some(id) {
            return Err(error(id.clone()));
        }
        previous = Some(id);
    }
    Ok(())
}

/// Exact successful resource movement totals, grouped by stable resource ID.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceFlowLedger {
    pub moved: BTreeMap<ContentId, u64>,
}

impl ResourceFlowLedger {
    #[must_use]
    pub fn quantity_moved(&self, resource: &ContentId) -> u64 {
        self.moved.get(resource).copied().unwrap_or(0)
    }
}

/// Moves a nonzero physical quantity atomically. Every source, destination,
/// and ledger result is checked before any of the three values is changed.
pub fn transfer_resource(
    source: &mut ResourceStore,
    destination: &mut ResourceStore,
    ledger: &mut ResourceFlowLedger,
    resource: &ContentId,
    quantity: u64,
) -> Result<(), CoreError> {
    if quantity == 0 {
        return Err(CoreError::ZeroResourceTransfer);
    }
    let source_before = source.quantity(resource);
    let destination_before = destination.quantity(resource);
    let ledger_before = ledger.quantity_moved(resource);

    let source_after =
        source_before
            .checked_sub(quantity)
            .ok_or_else(|| CoreError::InsufficientResource {
                resource: resource.clone(),
                available: source_before,
                requested: quantity,
            })?;
    let destination_after = destination_before
        .checked_add(quantity)
        .ok_or(CoreError::Overflow)?;
    let ledger_after = ledger_before
        .checked_add(quantity)
        .ok_or(CoreError::Overflow)?;

    source.set(resource.clone(), source_after);
    destination.set(resource.clone(), destination_after);
    ledger.moved.insert(resource.clone(), ledger_after);
    Ok(())
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CoreError {
    #[error("invalid content id: {0}")]
    InvalidId(String),
    #[error("checked arithmetic overflow")]
    Overflow,
    #[error("duplicate resource id: {0}")]
    DuplicateResourceId(ContentId),
    #[error("duplicate location id: {0}")]
    DuplicateLocationId(ContentId),
    #[error("duplicate system definition for location: {0}")]
    DuplicateSystemLocation(ContentId),
    #[error("resource system references unknown location: {0}")]
    UnknownSystemLocation(ContentId),
    #[error("system {location} stocks reference unknown resource: {resource}")]
    UnknownSystemStockResource {
        location: ContentId,
        resource: ContentId,
    },
    #[error("duplicate resource deposit id: {0}")]
    DuplicateDepositId(ContentId),
    #[error("duplicate reclaimable site id: {0}")]
    DuplicateSiteId(ContentId),
    #[error("location has a non-finite position: {0}")]
    NonFinitePosition(ContentId),
    #[error("origin references unknown location: {0}")]
    UnknownOriginLocation(ContentId),
    #[error("resource deposit quantity must be nonzero: {0}")]
    ZeroDepositQuantity(ContentId),
    #[error("deposit {deposit} references unknown location: {location}")]
    UnknownDepositLocation {
        deposit: ContentId,
        location: ContentId,
    },
    #[error("deposit {deposit} references unknown resource: {resource}")]
    UnknownDepositResource {
        deposit: ContentId,
        resource: ContentId,
    },
    #[error("site {site} references unknown location: {location}")]
    UnknownSiteLocation {
        site: ContentId,
        location: ContentId,
    },
    #[error("topology self-edge at location: {0}")]
    TopologySelfEdge(ContentId),
    #[error("duplicate topology edge: {from} -- {to}")]
    DuplicateTopologyEdge { from: ContentId, to: ContentId },
    #[error("topology edge references unknown endpoint: {0}")]
    UnknownTopologyEndpoint(ContentId),
    #[error("topology edge has non-finite derived distance: {from} -- {to}")]
    NonFiniteTopologyDistance { from: ContentId, to: ContentId },
    #[error("unknown location: {0}")]
    UnknownLocation(ContentId),
    #[error("unknown resource system: {0}")]
    UnknownSystem(ContentId),
    #[error("resource transfers must move a nonzero quantity")]
    ZeroResourceTransfer,
    #[error("resource transfer references unknown resource: {0}")]
    UnknownTransferResource(ContentId),
    #[error("insufficient {resource}: available {available}, requested {requested}")]
    InsufficientResource {
        resource: ContentId,
        available: u64,
        requested: u64,
    },
    #[error("resource engine prerequisites are missing")]
    MissingResourceEnginePrerequisite,
    #[error("resource engine references unknown resource: {0}")]
    UnknownEngineResource(ContentId),
    #[error("invalid resource engine config: {0}")]
    InvalidResourceEngineConfig(String),
    #[error("invalid {role:?} construction recipe: {reason}")]
    InvalidConstructionRecipe {
        role: DevelopmentRole,
        reason: String,
    },
    #[error("duplicate body id: {0}")]
    DuplicateBodyId(ContentId),
    #[error("duplicate slot id {slot} on body {body}")]
    DuplicateSlotId { body: ContentId, slot: ContentId },
    #[error("duplicate development id: {0}")]
    DuplicateDevelopmentId(ContentId),
    #[error("unknown body: {0}")]
    UnknownBody(ContentId),
    #[error("unknown slot {slot} on body {body}")]
    UnknownDevelopmentSlot { body: ContentId, slot: ContentId },
    #[error("development slot {body}/{slot} is occupied or reserved")]
    DevelopmentSlotUnavailable { body: ContentId, slot: ContentId },
    #[error("Extractor construction requires a deposit assignment")]
    ExtractorDepositRequired,
    #[error("only an Extractor may have a deposit assignment")]
    UnexpectedExtractorDeposit,
    #[error("unknown Extractor deposit: {0}")]
    UnknownExtractorDeposit(ContentId),
    #[error("deposit is not a compatible same-system Ore deposit: {0}")]
    IncompatibleExtractorDeposit(ContentId),
    #[error("deposit already has a queued or installed Extractor: {0}")]
    ExtractorDepositAlreadyAssigned(ContentId),
    #[error("available Energy {available} exceeds capacity {capacity}")]
    EnergyAboveCapacity { available: u64, capacity: u64 },
    #[error("unknown construction sequence: {0}")]
    UnknownConstructionSequence(u64),
    #[error("construction sequence has already begun and cannot be cancelled: {0}")]
    ConstructionAlreadyBegun(u64),
    #[error("invalid slot reservation for construction sequence: {0}")]
    InvalidConstructionReservation(u64),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> ContentId {
        ContentId::new(value).unwrap()
    }

    fn store(values: &[(&str, u64)]) -> ResourceStore {
        values
            .iter()
            .map(|(resource, quantity)| (id(resource), *quantity))
            .collect()
    }

    fn location(value: &str, name: &str, x: f64, y: f64) -> LocationDefinition {
        LocationDefinition {
            id: id(value),
            name: name.into(),
            position: Position3 { x, y, z: 0.0 },
        }
    }

    fn three_location_definition() -> WorldDefinition {
        WorldDefinition {
            resources: vec![
                ResourceDefinition {
                    id: id("core:ore"),
                    name: "Ore".into(),
                },
                ResourceDefinition {
                    id: id(ENERGY_ID),
                    name: "Energy".into(),
                },
            ],
            locations: vec![
                location("core:site_frontier", "Site Frontier", 3.0, 4.0),
                location("core:origin", "Origin", 0.0, 0.0),
                location("core:deposit_frontier", "Deposit Frontier", 6.0, 8.0),
            ],
            origin: OriginCommunityDefinition {
                id: id("core:first_community"),
                location: id("core:origin"),
                population: 100,
            },
            systems: vec![
                SystemDefinition {
                    location: id("core:origin"),
                    stocks: store(&[(ENERGY_ID, 40), ("core:ore", 3)]),
                    resource_engine: None,
                },
                SystemDefinition {
                    location: id("core:site_frontier"),
                    stocks: ResourceStore::new(),
                    resource_engine: None,
                },
                SystemDefinition {
                    location: id("core:deposit_frontier"),
                    stocks: ResourceStore::new(),
                    resource_engine: None,
                },
            ],
            deposits: vec![ResourceDepositDefinition {
                id: id("core:ore_deposit"),
                location: id("core:deposit_frontier"),
                resource: id("core:ore"),
                quantity: 70,
            }],
            sites: vec![ReclaimableSiteDefinition {
                id: id("core:derelict"),
                location: id("core:site_frontier"),
            }],
            topology: TopologyDefinition {
                edges: vec![
                    TopologyEdge::new(id("core:deposit_frontier"), id("core:site_frontier")),
                    TopologyEdge::new(id("core:site_frontier"), id("core:origin")),
                ],
            },
        }
    }

    #[test]
    fn content_id_validation_and_display_are_stable() {
        let value = ContentId::new("core:system_01").unwrap();
        assert_eq!(value.as_str(), "core:system_01");
        assert_eq!(value.to_string(), "core:system_01");
        assert_eq!(
            ContentId::new("missing_namespace"),
            Err(CoreError::InvalidId("missing_namespace".into()))
        );
        assert_eq!(
            ContentId::new("Core:origin"),
            Err(CoreError::InvalidId("Core:origin".into()))
        );
    }

    #[test]
    fn three_location_snapshot_is_normalized_and_frontier_is_neutral() {
        let state = WorldState::new(three_location_definition()).unwrap();
        let snapshot = state.snapshot();

        assert_eq!(snapshot.locations.len(), 3);
        assert_eq!(snapshot.locations[0].id, id("core:deposit_frontier"));
        assert_eq!(snapshot.locations[1].id, id("core:origin"));
        assert_eq!(snapshot.locations[2].id, id("core:site_frontier"));
        assert!(snapshot.locations[0].community.is_none());
        assert!(!snapshot.locations[0].is_origin);
        assert!(snapshot.locations[2].community.is_none());
        assert!(!snapshot.locations[2].is_origin);
        assert_eq!(
            snapshot.locations[1].community.as_ref().unwrap().population,
            100
        );
        assert!(snapshot.locations[1].is_origin);
        assert_eq!(snapshot.origin.population, 100);
        assert_eq!(
            state
                .system_stocks(&id("core:origin"))
                .unwrap()
                .quantity(&id(ENERGY_ID)),
            40
        );
        assert_eq!(snapshot.deposits.len(), 1);
        assert_eq!(snapshot.sites.len(), 1);
        assert_eq!(
            snapshot
                .topology
                .iter()
                .map(|edge| edge.distance)
                .collect::<Vec<_>>(),
            vec![5.0, 5.0]
        );

        let path = state
            .shortest_path(&id("core:origin"), &id("core:deposit_frontier"))
            .unwrap()
            .unwrap();
        assert_eq!(
            path.locations,
            vec![
                id("core:origin"),
                id("core:site_frontier"),
                id("core:deposit_frontier")
            ]
        );
        assert_eq!(path.distance, 10.0);
    }

    #[test]
    fn input_permutations_produce_equal_snapshots() {
        let definition = three_location_definition();
        let mut permuted = definition.clone();
        permuted.resources.reverse();
        permuted.locations.reverse();
        permuted.systems.reverse();
        permuted.deposits.reverse();
        permuted.sites.reverse();
        permuted.topology.edges.reverse();
        for edge in &mut permuted.topology.edges {
            std::mem::swap(&mut edge.from, &mut edge.to);
        }

        assert_eq!(
            WorldState::new(definition).unwrap().snapshot(),
            WorldState::new(permuted).unwrap().snapshot()
        );
    }

    #[test]
    fn empty_and_disconnected_topology_are_valid() {
        let mut definition = three_location_definition();
        definition.topology.edges.clear();
        let state = WorldState::new(definition).unwrap();
        assert!(state.snapshot().topology.is_empty());
        assert_eq!(
            state
                .topology_distance(&id("core:origin"), &id("core:site_frontier"))
                .unwrap(),
            None
        );

        let mut definition = three_location_definition();
        definition.topology.edges.pop();
        let state = WorldState::new(definition).unwrap();
        assert_eq!(
            state
                .topology_distance(&id("core:origin"), &id("core:deposit_frontier"))
                .unwrap(),
            None
        );
    }

    #[test]
    fn topology_rejects_self_duplicate_and_unknown_edges() {
        let mut self_edge = three_location_definition();
        self_edge.topology.edges = vec![TopologyEdge::new(id("core:origin"), id("core:origin"))];
        assert!(matches!(
            WorldState::new(self_edge),
            Err(CoreError::TopologySelfEdge(_))
        ));

        let mut duplicate = three_location_definition();
        duplicate.topology.edges = vec![
            TopologyEdge::new(id("core:origin"), id("core:site_frontier")),
            TopologyEdge::new(id("core:site_frontier"), id("core:origin")),
        ];
        assert!(matches!(
            WorldState::new(duplicate),
            Err(CoreError::DuplicateTopologyEdge { .. })
        ));

        let mut unknown = three_location_definition();
        unknown.topology.edges = vec![TopologyEdge::new(id("core:origin"), id("core:unknown"))];
        assert_eq!(
            WorldState::new(unknown).err(),
            Some(CoreError::UnknownTopologyEndpoint(id("core:unknown")))
        );
    }

    #[test]
    fn definitions_reject_duplicates_nonfinite_zero_and_unknown_references() {
        let mut duplicate = three_location_definition();
        duplicate.resources.push(duplicate.resources[0].clone());
        assert!(matches!(
            WorldState::new(duplicate),
            Err(CoreError::DuplicateResourceId(_))
        ));

        let mut nonfinite = three_location_definition();
        nonfinite.locations[0].position.x = f64::NAN;
        assert!(matches!(
            WorldState::new(nonfinite),
            Err(CoreError::NonFinitePosition(_))
        ));

        let mut zero_population = three_location_definition();
        zero_population.origin.population = 0;
        assert!(WorldState::new(zero_population).is_ok());

        let mut zero_deposit = three_location_definition();
        zero_deposit.deposits[0].quantity = 0;
        assert!(matches!(
            WorldState::new(zero_deposit),
            Err(CoreError::ZeroDepositQuantity(_))
        ));

        let mut unknown_stock = three_location_definition();
        unknown_stock.systems[0].stocks.set(id("core:unknown"), 1);
        assert!(matches!(
            WorldState::new(unknown_stock),
            Err(CoreError::UnknownSystemStockResource { .. })
        ));

        let mut unknown_deposit_resource = three_location_definition();
        unknown_deposit_resource.deposits[0].resource = id("core:unknown");
        assert!(matches!(
            WorldState::new(unknown_deposit_resource),
            Err(CoreError::UnknownDepositResource { .. })
        ));

        let mut unknown_site_location = three_location_definition();
        unknown_site_location.sites[0].location = id("core:unknown");
        assert!(matches!(
            WorldState::new(unknown_site_location),
            Err(CoreError::UnknownSiteLocation { .. })
        ));
    }

    fn recipe(cost: &[(&str, u64)], required_work: u64) -> ConstructionRecipe {
        ConstructionRecipe {
            cost: store(cost),
            required_work,
        }
    }

    fn stage4_definition(population: u64) -> WorldDefinition {
        WorldDefinition {
            resources: vec![
                ResourceDefinition {
                    id: id(ENERGY_ID),
                    name: "Energy".into(),
                },
                ResourceDefinition {
                    id: id("core:ore"),
                    name: "Ore".into(),
                },
                ResourceDefinition {
                    id: id("core:alloy"),
                    name: "Alloy".into(),
                },
            ],
            locations: vec![location("core:origin", "Origin", 0.0, 0.0)],
            origin: OriginCommunityDefinition {
                id: id("core:first_community"),
                location: id("core:origin"),
                population,
            },
            systems: vec![SystemDefinition {
                location: id("core:origin"),
                stocks: store(&[(ENERGY_ID, 10), ("core:ore", 10), ("core:alloy", 0)]),
                resource_engine: Some(stage4_engine()),
            }],
            deposits: vec![ResourceDepositDefinition {
                id: id("core:ore_deposit"),
                location: id("core:origin"),
                resource: id("core:ore"),
                quantity: 200,
            }],
            sites: vec![],
            topology: TopologyDefinition::default(),
        }
    }

    fn stage4_engine() -> ResourceEngineDefinition {
        let slots = (0..6)
            .map(|index| DevelopmentSlotDefinition {
                id: id(&format!("core:slot_{index}")),
                development: (index == 0).then(|| DevelopmentDefinition {
                    id: id("core:initial_collector"),
                    role: DevelopmentRole::Collector,
                    condition: DevelopmentCondition::Functional,
                    extractor_deposit: None,
                }),
            })
            .collect();
        ResourceEngineDefinition {
            collector_energy_profile: [40, 40, 30, 20, 10, 10, 20, 30, 40, 40],
            bodies: vec![BodyDefinition {
                id: id("core:origin_body"),
                name: "Origin Body".into(),
                slots,
            }],
            config: ResourceEngineConfig {
                energy_resource: id(ENERGY_ID),
                ore_resource: id("core:ore"),
                alloy_resource: id("core:alloy"),
                life_support_per_population: 10,
                origin_construction_work: 1,
                intrinsic_energy_capacity: 10,
                battery_energy_capacity: 100,
                collector_recipe: recipe(&[(ENERGY_ID, 10), ("core:alloy", 2)], 4),
                battery_recipe: recipe(&[(ENERGY_ID, 10), ("core:alloy", 2)], 4),
                extractor_recipe: recipe(&[(ENERGY_ID, 10), ("core:alloy", 2)], 4),
                refinery_recipe: recipe(&[(ENERGY_ID, 10), ("core:ore", 2)], 4),
                extractor: ExtractorParameters {
                    energy_upkeep: 10,
                    cycle_duration: 1,
                    ore_output: 1,
                },
                refinery: RefineryParameters {
                    energy_upkeep: 10,
                    cycle_duration: 1,
                    ore_input: 2,
                    alloy_output: 1,
                },
            },
        }
    }

    fn stage4_state(population: u64) -> WorldState {
        WorldState::new(stage4_definition(population)).unwrap()
    }

    fn stage4_system_mut(definition: &mut WorldDefinition) -> &mut SystemDefinition {
        definition
            .systems
            .iter_mut()
            .find(|system| system.location == id("core:origin"))
            .unwrap()
    }

    fn stage4_engine_mut(definition: &mut WorldDefinition) -> &mut ResourceEngineDefinition {
        stage4_system_mut(definition)
            .resource_engine
            .as_mut()
            .unwrap()
    }

    fn development(
        value: &str,
        role: DevelopmentRole,
        condition: DevelopmentCondition,
        extractor_deposit: Option<&str>,
    ) -> DevelopmentDefinition {
        DevelopmentDefinition {
            id: id(value),
            role,
            condition,
            extractor_deposit: extractor_deposit.map(id),
        }
    }

    fn development_roles(snapshot: &ResourceEngineSnapshot) -> Vec<DevelopmentRole> {
        snapshot
            .bodies
            .iter()
            .flat_map(|body| &body.slots)
            .filter_map(|slot| {
                slot.development
                    .as_ref()
                    .map(|development| development.definition.role)
            })
            .collect()
    }

    #[test]
    fn stage3_world_snapshots_but_tick_rejects_atomically() {
        let mut state = WorldState::new(three_location_definition()).unwrap();
        let before = state.snapshot();
        assert_eq!(
            state.advance_tick(),
            Err(CoreError::MissingResourceEnginePrerequisite)
        );
        assert_eq!(state.snapshot(), before);
    }

    #[test]
    fn non_origin_system_owns_stocks_and_command_state() {
        let mut definition = stage4_definition(0);
        definition
            .locations
            .push(location("core:frontier", "Frontier", 1.0, 0.0));
        definition.systems.push(SystemDefinition {
            location: id("core:frontier"),
            stocks: store(&[(ENERGY_ID, 10), ("core:ore", 2), ("core:alloy", 0)]),
            resource_engine: Some(stage4_engine()),
        });
        let mut state = WorldState::new(definition).unwrap();

        state
            .enqueue_construction(
                &id("core:frontier"),
                &id("core:origin_body"),
                &id("core:slot_1"),
                DevelopmentRole::Refinery,
                None,
            )
            .unwrap();

        let origin = state.strategic_snapshot(&id("core:origin")).unwrap();
        let frontier = state.strategic_snapshot(&id("core:frontier")).unwrap();
        assert!(origin.construction_queue.is_empty());
        assert_eq!(frontier.location, id("core:frontier"));
        assert_eq!(frontier.stocks.quantity(&id(ENERGY_ID)), 0);
        assert_eq!(frontier.stocks.quantity(&id("core:ore")), 0);
        assert_eq!(frontier.construction_queue.len(), 1);
        assert!(
            state
                .snapshot()
                .locations
                .iter()
                .find(|location| location.id == id("core:frontier"))
                .unwrap()
                .community
                .is_none()
        );

        assert_eq!(state.advance_tick().unwrap().location, id("core:origin"));
        let frontier = state.strategic_snapshot(&id("core:frontier")).unwrap();
        assert_eq!(frontier.time.tick, 0);
        assert_eq!(frontier.construction_queue[0].work_applied, 0);
    }

    #[test]
    fn constructed_development_ids_are_deterministic_and_unique_across_systems() {
        let mut definition = stage4_definition(0);
        definition
            .locations
            .push(location("core:frontier", "Frontier", 1.0, 0.0));
        definition.systems.push(SystemDefinition {
            location: id("core:frontier"),
            stocks: store(&[(ENERGY_ID, 10), ("core:ore", 10), ("core:alloy", 0)]),
            resource_engine: Some(stage4_engine()),
        });
        let mut state = WorldState::new(definition).unwrap();
        for system in ["core:origin", "core:frontier"] {
            state
                .enqueue_construction(
                    &id(system),
                    &id("core:origin_body"),
                    &id("core:slot_1"),
                    DevelopmentRole::Refinery,
                    None,
                )
                .unwrap();
        }

        let origin_id = state
            .strategic_snapshot(&id("core:origin"))
            .unwrap()
            .construction_queue[0]
            .development_id
            .clone();
        let frontier_id = state
            .strategic_snapshot(&id("core:frontier"))
            .unwrap()
            .construction_queue[0]
            .development_id
            .clone();
        assert_eq!(origin_id, id("core:origin:development_0"));
        assert_eq!(frontier_id, id("core:frontier:development_0"));
        assert_ne!(origin_id, frontier_id);
    }

    #[test]
    fn authored_development_id_collision_rejects_enqueue_atomically() {
        let mut definition = stage4_definition(0);
        stage4_engine_mut(&mut definition).bodies[0].slots[0]
            .development
            .as_mut()
            .unwrap()
            .id = id("core:origin:development_0");
        let mut state = WorldState::new(definition).unwrap();
        let system = id("core:origin");
        let before = state.strategic_snapshot(&system).unwrap();

        assert_eq!(
            state.enqueue_construction(
                &system,
                &id("core:origin_body"),
                &id("core:slot_1"),
                DevelopmentRole::Refinery,
                None,
            ),
            Err(CoreError::DuplicateDevelopmentId(id(
                "core:origin:development_0"
            )))
        );
        assert_eq!(state.strategic_snapshot(&system).unwrap(), before);
    }

    #[test]
    fn resource_engine_definition_order_does_not_change_state() {
        let mut permuted = stage4_definition(0);
        let permuted_engine = stage4_engine_mut(&mut permuted);
        permuted_engine.bodies.reverse();
        for body in &mut permuted_engine.bodies {
            body.slots.reverse();
        }
        let mut first = WorldState::new(stage4_definition(0)).unwrap();
        let mut second = WorldState::new(permuted).unwrap();
        assert_eq!(
            first.strategic_snapshot(&id("core:origin")).unwrap(),
            second.strategic_snapshot(&id("core:origin")).unwrap()
        );
        assert_eq!(
            first.advance_tick().unwrap(),
            second.advance_tick().unwrap()
        );
    }

    #[test]
    fn zero_population_origin_bootstraps_exactly_one_work() {
        let mut state = stage4_state(0);
        state
            .enqueue_construction(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:slot_1"),
                DevelopmentRole::Refinery,
                None,
            )
            .unwrap();
        let snapshot = state.advance_tick().unwrap();
        assert_eq!(snapshot.life_support.supported_population, 0);
        assert_eq!(snapshot.life_support.construction_work, 1);
        assert_eq!(snapshot.construction_queue[0].work_applied, 1);
        let reserved_id = snapshot.construction_queue[0].development_id.clone();
        let mut completed = snapshot;
        for _ in 0..3 {
            completed = state.advance_tick().unwrap();
        }
        assert_eq!(
            completed.bodies[0].slots[1]
                .development
                .as_ref()
                .unwrap()
                .definition
                .id,
            reserved_id
        );
    }

    #[test]
    fn origin_construction_work_uses_the_authored_value() {
        let mut definition = stage4_definition(0);
        stage4_engine_mut(&mut definition)
            .config
            .origin_construction_work = 3;
        let mut state = WorldState::new(definition).unwrap();
        assert_eq!(
            state.advance_tick().unwrap().life_support.construction_work,
            3
        );
    }

    #[test]
    fn supported_and_underserved_population_drive_construction_work() {
        let mut definition = stage4_definition(2);
        stage4_system_mut(&mut definition)
            .stocks
            .set(id(ENERGY_ID), 0);
        stage4_engine_mut(&mut definition).collector_energy_profile = [15; 10];
        let mut state = WorldState::new(definition).unwrap();
        let snapshot = state.advance_tick().unwrap();
        assert_eq!(snapshot.life_support.paid_energy, 15);
        assert_eq!(snapshot.life_support.unpaid_energy, 5);
        assert_eq!(snapshot.life_support.supported_population, 1);
        assert_eq!(snapshot.life_support.underserved_population, 1);
        assert_eq!(snapshot.life_support.construction_work, 2);
    }

    #[test]
    fn fifo_work_overflows_to_the_next_item_in_the_same_tick() {
        let mut definition = stage4_definition(2);
        stage4_system_mut(&mut definition).stocks = store(&[(ENERGY_ID, 30), ("core:ore", 14)]);
        let engine = stage4_engine_mut(&mut definition);
        engine.config.intrinsic_energy_capacity = 30;
        engine.collector_energy_profile = [20; 10];
        let mut state = WorldState::new(definition).unwrap();
        for slot in ["core:slot_1", "core:slot_2"] {
            state
                .enqueue_construction(
                    &id("core:origin"),
                    &id("core:origin_body"),
                    &id(slot),
                    DevelopmentRole::Refinery,
                    None,
                )
                .unwrap();
        }
        let first = state.advance_tick().unwrap();
        assert_eq!(first.construction_queue[0].work_applied, 3);
        let second = state.advance_tick().unwrap();
        assert_eq!(development_roles(&second).len(), 2);
        assert_eq!(second.construction_queue[0].sequence, 1);
        assert_eq!(second.construction_queue[0].work_applied, 2);
    }

    #[test]
    fn cancellation_refund_overflow_is_atomic_and_explicit() {
        let mut state = stage4_state(0);
        let sequence = state
            .enqueue_construction(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:slot_1"),
                DevelopmentRole::Refinery,
                None,
            )
            .unwrap();
        let mut source = store(&[(ENERGY_ID, 10)]);
        let mut ledger = ResourceFlowLedger::default();
        state
            .transfer_resource_to_system(
                &id("core:origin"),
                &mut source,
                &mut ledger,
                &id(ENERGY_ID),
                10,
            )
            .unwrap();
        state
            .cancel_construction(&id("core:origin"), sequence)
            .unwrap();
        let snapshot = state.strategic_snapshot(&id("core:origin")).unwrap();
        assert_eq!(snapshot.stocks.quantity(&id(ENERGY_ID)), 10);
        assert_eq!(snapshot.stocks.quantity(&id("core:ore")), 10);
        assert!(snapshot.construction_queue.is_empty());
        assert_eq!(snapshot.energy_overflow.cumulative, 10);
        assert_eq!(
            snapshot.energy_overflow.evidence.last().unwrap().cause,
            EnergyOverflowCause::CancellationRefund
        );
    }

    #[test]
    fn cancellation_refund_at_max_capacity_does_not_overflow_arithmetic() {
        let mut definition = stage4_definition(0);
        stage4_engine_mut(&mut definition)
            .config
            .intrinsic_energy_capacity = u64::MAX;
        let mut state = WorldState::new(definition).unwrap();
        let system = id("core:origin");
        let sequence = state
            .enqueue_construction(
                &system,
                &id("core:origin_body"),
                &id("core:slot_1"),
                DevelopmentRole::Refinery,
                None,
            )
            .unwrap();
        let mut source = store(&[(ENERGY_ID, u64::MAX)]);
        state
            .transfer_resource_to_system(
                &system,
                &mut source,
                &mut ResourceFlowLedger::default(),
                &id(ENERGY_ID),
                u64::MAX,
            )
            .unwrap();

        state.cancel_construction(&system, sequence).unwrap();
        let snapshot = state.strategic_snapshot(&system).unwrap();
        assert_eq!(snapshot.stocks.quantity(&id(ENERGY_ID)), u64::MAX);
        assert_eq!(snapshot.energy_overflow.cumulative, 10);
        assert_eq!(
            snapshot.energy_overflow.evidence.last().unwrap(),
            &EnergyOverflowEvidence {
                tick: 0,
                cause: EnergyOverflowCause::CancellationRefund,
                quantity: 10,
            }
        );
    }

    #[test]
    fn refinery_cycle_commits_input_and_pauses_without_energy() {
        let mut definition = stage4_definition(0);
        stage4_system_mut(&mut definition).stocks = store(&[(ENERGY_ID, 10), ("core:ore", 2)]);
        let engine = stage4_engine_mut(&mut definition);
        engine.collector_energy_profile = [0; 10];
        engine.config.refinery.cycle_duration = 2;
        engine.bodies[0].slots[1].development = Some(DevelopmentDefinition {
            id: id("core:initial_refinery"),
            role: DevelopmentRole::Refinery,
            condition: DevelopmentCondition::Functional,
            extractor_deposit: None,
        });
        let mut state = WorldState::new(definition).unwrap();
        let first = state.advance_tick().unwrap();
        let first_cycle = first.bodies[0].slots[1]
            .development
            .as_ref()
            .unwrap()
            .cycle
            .clone();
        assert_eq!(first_cycle.progress, 1);
        assert_eq!(first_cycle.committed_inputs.quantity(&id("core:ore")), 2);
        let paused = state.advance_tick().unwrap();
        assert_eq!(
            paused.bodies[0].slots[1]
                .development
                .as_ref()
                .unwrap()
                .cycle,
            first_cycle
        );
        let mut source = store(&[(ENERGY_ID, 10)]);
        state
            .transfer_resource_to_system(
                &id("core:origin"),
                &mut source,
                &mut ResourceFlowLedger::default(),
                &id(ENERGY_ID),
                10,
            )
            .unwrap();
        let completed = state.advance_tick().unwrap();
        assert_eq!(completed.stocks.quantity(&id("core:alloy")), 1);
        assert_eq!(
            completed.bodies[0].slots[1]
                .development
                .as_ref()
                .unwrap()
                .cycle
                .progress,
            0
        );
    }

    #[test]
    fn damaged_and_ruined_developments_have_no_stage4_consequences() {
        let mut definition = stage4_definition(0);
        definition.deposits.push(ResourceDepositDefinition {
            id: id("core:other_ore_deposit"),
            location: id("core:origin"),
            resource: id("core:ore"),
            quantity: 200,
        });
        let engine = stage4_engine_mut(&mut definition);
        engine.collector_energy_profile = [40; 10];
        engine.bodies[0].slots = [
            (
                DevelopmentRole::Collector,
                DevelopmentCondition::Damaged,
                None,
            ),
            (
                DevelopmentRole::Collector,
                DevelopmentCondition::Ruined,
                None,
            ),
            (
                DevelopmentRole::Battery,
                DevelopmentCondition::Damaged,
                None,
            ),
            (DevelopmentRole::Battery, DevelopmentCondition::Ruined, None),
            (
                DevelopmentRole::Extractor,
                DevelopmentCondition::Damaged,
                Some("core:ore_deposit"),
            ),
            (
                DevelopmentRole::Extractor,
                DevelopmentCondition::Ruined,
                Some("core:other_ore_deposit"),
            ),
            (
                DevelopmentRole::Refinery,
                DevelopmentCondition::Damaged,
                None,
            ),
            (
                DevelopmentRole::Refinery,
                DevelopmentCondition::Ruined,
                None,
            ),
        ]
        .into_iter()
        .enumerate()
        .map(
            |(index, (role, condition, deposit))| DevelopmentSlotDefinition {
                id: id(&format!("core:slot_{index}")),
                development: Some(development(
                    &format!("core:disabled_{index}"),
                    role,
                    condition,
                    deposit,
                )),
            },
        )
        .collect();

        let mut state = WorldState::new(definition).unwrap();
        let snapshot = state.advance_tick().unwrap();

        assert_eq!(snapshot.energy_capacity, 10);
        assert_eq!(
            snapshot.stocks,
            store(&[(ENERGY_ID, 10), ("core:ore", 10), ("core:alloy", 0)])
        );
        assert!(
            snapshot
                .deposits
                .iter()
                .all(|deposit| deposit.quantity == 200)
        );
        for resource in [ENERGY_ID, "core:ore", "core:alloy"] {
            assert_eq!(snapshot.accounting.produced.quantity(&id(resource)), 0);
            assert_eq!(
                snapshot.accounting.operation_spent.quantity(&id(resource)),
                0
            );
        }
        assert!(
            snapshot
                .bodies
                .iter()
                .flat_map(|body| &body.slots)
                .all(|slot| slot.development.as_ref().unwrap().cycle == ProductionCycle::default())
        );
    }

    #[test]
    fn refineries_keep_independent_cycles_and_contend_in_body_slot_order() {
        let mut definition = stage4_definition(0);
        stage4_system_mut(&mut definition).stocks = store(&[(ENERGY_ID, 20), ("core:ore", 4)]);
        let engine = stage4_engine_mut(&mut definition);
        engine.collector_energy_profile = [0; 10];
        engine.config.intrinsic_energy_capacity = 20;
        engine.config.refinery.cycle_duration = 2;
        engine.bodies = vec![
            BodyDefinition {
                id: id("core:z_body"),
                name: "Z".into(),
                slots: vec![DevelopmentSlotDefinition {
                    id: id("core:a_slot"),
                    development: Some(development(
                        "core:z_refinery",
                        DevelopmentRole::Refinery,
                        DevelopmentCondition::Functional,
                        None,
                    )),
                }],
            },
            BodyDefinition {
                id: id("core:a_body"),
                name: "A".into(),
                slots: vec![
                    DevelopmentSlotDefinition {
                        id: id("core:b_slot"),
                        development: Some(development(
                            "core:second_refinery",
                            DevelopmentRole::Refinery,
                            DevelopmentCondition::Functional,
                            None,
                        )),
                    },
                    DevelopmentSlotDefinition {
                        id: id("core:a_slot"),
                        development: Some(development(
                            "core:first_refinery",
                            DevelopmentRole::Refinery,
                            DevelopmentCondition::Functional,
                            None,
                        )),
                    },
                ],
            },
        ];
        let mut state = WorldState::new(definition).unwrap();

        let first = state.advance_tick().unwrap();
        assert_eq!(first.bodies[0].id, id("core:a_body"));
        assert_eq!(first.bodies[0].slots[0].id, id("core:a_slot"));
        for slot in &first.bodies[0].slots {
            let cycle = &slot.development.as_ref().unwrap().cycle;
            assert_eq!(cycle.progress, 1);
            assert_eq!(cycle.committed_inputs.quantity(&id("core:ore")), 2);
        }
        assert_eq!(
            first.bodies[1].slots[0].development.as_ref().unwrap().cycle,
            ProductionCycle::default()
        );

        let mut source = store(&[(ENERGY_ID, 10)]);
        state
            .transfer_resource_to_system(
                &id("core:origin"),
                &mut source,
                &mut ResourceFlowLedger::default(),
                &id(ENERGY_ID),
                10,
            )
            .unwrap();
        let second = state.advance_tick().unwrap();
        assert_eq!(second.stocks.quantity(&id("core:alloy")), 1);
        assert_eq!(
            second.bodies[0].slots[0]
                .development
                .as_ref()
                .unwrap()
                .cycle,
            ProductionCycle::default()
        );
        let waiting = &second.bodies[0].slots[1]
            .development
            .as_ref()
            .unwrap()
            .cycle;
        assert_eq!(waiting.progress, 1);
        assert_eq!(waiting.committed_inputs.quantity(&id("core:ore")), 2);
    }

    #[test]
    fn completion_timing_applies_battery_retention_before_other_new_operation() {
        let mut definition = stage4_definition(0);
        stage4_system_mut(&mut definition).stocks =
            store(&[(ENERGY_ID, 40), ("core:ore", 12), ("core:alloy", 6)]);
        let engine = stage4_engine_mut(&mut definition);
        engine.collector_energy_profile = [80; 10];
        engine.config.intrinsic_energy_capacity = 40;
        engine.config.origin_construction_work = 4;
        for recipe in [
            &mut engine.config.collector_recipe,
            &mut engine.config.battery_recipe,
            &mut engine.config.extractor_recipe,
            &mut engine.config.refinery_recipe,
        ] {
            recipe.required_work = 1;
        }
        let mut state = WorldState::new(definition).unwrap();
        let system = id("core:origin");
        let body = id("core:origin_body");
        for (slot, role, deposit) in [
            ("core:slot_1", DevelopmentRole::Battery, None),
            ("core:slot_2", DevelopmentRole::Collector, None),
            (
                "core:slot_3",
                DevelopmentRole::Extractor,
                Some(id("core:ore_deposit")),
            ),
            ("core:slot_4", DevelopmentRole::Refinery, None),
        ] {
            state
                .enqueue_construction(&system, &body, &id(slot), role, deposit.as_ref())
                .unwrap();
        }

        let completed = state.advance_tick().unwrap();
        assert!(completed.construction_queue.is_empty());
        assert_eq!(completed.energy_capacity, 140);
        assert_eq!(completed.stocks.quantity(&id(ENERGY_ID)), 80);
        assert_eq!(completed.stocks.quantity(&id("core:ore")), 10);
        assert_eq!(completed.stocks.quantity(&id("core:alloy")), 0);
        assert_eq!(completed.deposits[0].quantity, 200);
        assert_eq!(completed.accounting.operation_spent, ResourceStore::new());

        let operating = state.advance_tick().unwrap();
        assert_eq!(operating.stocks.quantity(&id(ENERGY_ID)), 140);
        assert_eq!(operating.stocks.quantity(&id("core:ore")), 9);
        assert_eq!(operating.stocks.quantity(&id("core:alloy")), 1);
        assert_eq!(operating.deposits[0].quantity, 199);
    }

    #[test]
    fn transfer_to_system_rejects_unknown_resource_atomically() {
        let mut state = stage4_state(0);
        let system = id("core:origin");
        let resource = id("core:unknown");
        let mut source = ResourceStore::from_iter([(resource.clone(), 1)]);
        let mut ledger = ResourceFlowLedger::default();
        let before = (state.snapshot(), source.clone(), ledger.clone());

        assert_eq!(
            state.transfer_resource_to_system(&system, &mut source, &mut ledger, &resource, 1,),
            Err(CoreError::UnknownTransferResource(resource))
        );
        assert_eq!((state.snapshot(), source, ledger), before);
    }

    #[test]
    fn incoming_energy_at_capacity_reconciles_retained_and_overflow() {
        let mut state = stage4_state(0);
        let mut source = store(&[(ENERGY_ID, 9)]);
        let mut ledger = ResourceFlowLedger::default();
        state
            .transfer_resource_to_system(
                &id("core:origin"),
                &mut source,
                &mut ledger,
                &id(ENERGY_ID),
                9,
            )
            .unwrap();
        let snapshot = state.strategic_snapshot(&id("core:origin")).unwrap();
        assert_eq!(source.quantity(&id(ENERGY_ID)), 0);
        assert_eq!(snapshot.stocks.quantity(&id(ENERGY_ID)), 10);
        assert_eq!(snapshot.energy_overflow.cumulative, 9);
        assert_eq!(ledger.quantity_moved(&id(ENERGY_ID)), 9);
    }

    #[test]
    fn extractor_reservation_cancellation_release_and_begun_rejection_are_atomic() {
        let mut definition = stage4_definition(0);
        stage4_system_mut(&mut definition).stocks =
            store(&[(ENERGY_ID, 30), ("core:ore", 10), ("core:alloy", 4)]);
        stage4_engine_mut(&mut definition)
            .config
            .intrinsic_energy_capacity = 30;
        let mut state = WorldState::new(definition).unwrap();
        let system = id("core:origin");
        let deposit = id("core:ore_deposit");
        let first = state
            .enqueue_construction(
                &system,
                &id("core:origin_body"),
                &id("core:slot_1"),
                DevelopmentRole::Extractor,
                Some(&deposit),
            )
            .unwrap();
        let before_rejection = state.strategic_snapshot(&system).unwrap();
        assert_eq!(
            state.enqueue_construction(
                &system,
                &id("core:origin_body"),
                &id("core:slot_2"),
                DevelopmentRole::Extractor,
                Some(&deposit),
            ),
            Err(CoreError::ExtractorDepositAlreadyAssigned(deposit.clone()))
        );
        assert_eq!(state.strategic_snapshot(&system).unwrap(), before_rejection);

        state.cancel_construction(&system, first).unwrap();
        let second = state
            .enqueue_construction(
                &system,
                &id("core:origin_body"),
                &id("core:slot_2"),
                DevelopmentRole::Extractor,
                Some(&deposit),
            )
            .unwrap();
        state.advance_tick().unwrap();
        let before_begun_cancel = state.strategic_snapshot(&system).unwrap();
        assert_eq!(
            state.cancel_construction(&system, second),
            Err(CoreError::ConstructionAlreadyBegun(second))
        );
        assert_eq!(
            state.strategic_snapshot(&system).unwrap(),
            before_begun_cancel
        );
    }

    #[test]
    fn above_capacity_authored_engine_is_rejected_during_world_construction() {
        let mut definition = stage4_definition(0);
        stage4_system_mut(&mut definition)
            .stocks
            .set(id(ENERGY_ID), 11);
        assert!(matches!(
            WorldState::new(definition),
            Err(CoreError::EnergyAboveCapacity {
                available: 11,
                capacity: 10,
            })
        ));
    }

    #[test]
    fn non_unit_extractor_output_is_rejected() {
        let mut definition = stage4_definition(0);
        stage4_engine_mut(&mut definition)
            .config
            .extractor
            .ore_output = 2;
        assert_eq!(
            WorldState::new(definition).err(),
            Some(CoreError::InvalidResourceEngineConfig(
                "Extractor ore output must equal 1".into()
            ))
        );
    }

    #[test]
    fn rejected_enqueue_and_sequence_overflow_leave_everything_unchanged() {
        let mut state = stage4_state(0);
        let system = id("core:origin");
        let before = state.strategic_snapshot(&system).unwrap();
        assert_eq!(
            state.enqueue_construction(
                &system,
                &id("core:origin_body"),
                &id("core:slot_0"),
                DevelopmentRole::Battery,
                None,
            ),
            Err(CoreError::DevelopmentSlotUnavailable {
                body: id("core:origin_body"),
                slot: id("core:slot_0"),
            })
        );
        assert_eq!(state.strategic_snapshot(&system).unwrap(), before);

        state
            .systems
            .get_mut(&system)
            .unwrap()
            .resource_engine
            .as_mut()
            .unwrap()
            .next_construction_sequence = u64::MAX;
        let before = state.strategic_snapshot(&system).unwrap();
        assert_eq!(
            state.enqueue_construction(
                &system,
                &id("core:origin_body"),
                &id("core:slot_1"),
                DevelopmentRole::Refinery,
                None,
            ),
            Err(CoreError::Overflow)
        );
        assert_eq!(state.strategic_snapshot(&system).unwrap(), before);
    }

    #[test]
    fn exact_twenty_tick_bootstrap_matches_the_approved_fixture() {
        let mut state = stage4_state(0);
        let system = id("core:origin");
        let body = id("core:origin_body");
        state
            .enqueue_construction(
                &system,
                &body,
                &id("core:slot_1"),
                DevelopmentRole::Refinery,
                None,
            )
            .unwrap();
        let initial = state.strategic_snapshot(&system).unwrap();
        assert_eq!(
            (
                initial.stocks.quantity(&id(ENERGY_ID)),
                initial.stocks.quantity(&id("core:ore")),
                initial.stocks.quantity(&id("core:alloy"))
            ),
            (0, 8, 0)
        );

        let mut checkpoints = BTreeMap::new();
        for tick in 1..=20 {
            let snapshot = state.advance_tick().unwrap();
            if [4, 8, 12, 16, 20].contains(&tick) {
                checkpoints.insert(tick, snapshot.clone());
            }
            if tick == 8 {
                state
                    .enqueue_construction(
                        &system,
                        &body,
                        &id("core:slot_2"),
                        DevelopmentRole::Battery,
                        None,
                    )
                    .unwrap();
                let after = state.strategic_snapshot(&system).unwrap();
                assert_eq!(
                    (
                        after.stocks.quantity(&id(ENERGY_ID)),
                        after.stocks.quantity(&id("core:ore")),
                        after.stocks.quantity(&id("core:alloy"))
                    ),
                    (0, 0, 2)
                );
            }
            if tick == 12 {
                state
                    .enqueue_construction(
                        &system,
                        &body,
                        &id("core:slot_3"),
                        DevelopmentRole::Extractor,
                        Some(&id("core:ore_deposit")),
                    )
                    .unwrap();
                let after = state.strategic_snapshot(&system).unwrap();
                assert_eq!(
                    (
                        after.stocks.quantity(&id(ENERGY_ID)),
                        after.stocks.quantity(&id("core:ore")),
                        after.stocks.quantity(&id("core:alloy"))
                    ),
                    (40, 0, 0)
                );
            }
        }

        let expected = [
            (4, (10, 8, 0), 200, 120),
            (8, (10, 0, 4), 200, 150),
            (12, (50, 0, 2), 200, 260),
            (16, (110, 0, 0), 200, 260),
            (20, (110, 0, 2), 196, 330),
        ];
        for (tick, stocks, deposit, overflow) in expected {
            let snapshot = &checkpoints[&tick];
            assert_eq!(
                (
                    snapshot.stocks.quantity(&id(ENERGY_ID)),
                    snapshot.stocks.quantity(&id("core:ore")),
                    snapshot.stocks.quantity(&id("core:alloy"))
                ),
                stocks
            );
            assert_eq!(snapshot.deposits[0].quantity, deposit);
            assert_eq!(snapshot.energy_overflow.cumulative, overflow);
        }
        let final_snapshot = &checkpoints[&20];
        assert_eq!(
            development_roles(final_snapshot),
            vec![
                DevelopmentRole::Collector,
                DevelopmentRole::Refinery,
                DevelopmentRole::Battery,
                DevelopmentRole::Extractor
            ]
        );
        assert_eq!(
            (
                final_snapshot
                    .accounting
                    .construction_spent
                    .quantity(&id(ENERGY_ID)),
                final_snapshot
                    .accounting
                    .construction_spent
                    .quantity(&id("core:ore")),
                final_snapshot
                    .accounting
                    .construction_spent
                    .quantity(&id("core:alloy"))
            ),
            (30, 2, 4)
        );
        assert_eq!(
            final_snapshot
                .accounting
                .operation_spent
                .quantity(&id(ENERGY_ID)),
            100
        );
        assert_eq!(
            final_snapshot
                .accounting
                .operation_spent
                .quantity(&id("core:ore")),
            12
        );
    }

    #[test]
    fn energy_transfer_reconciles_exactly() {
        let resource = id(ENERGY_ID);
        let mut source = store(&[(ENERGY_ID, 9)]);
        let mut destination = store(&[(ENERGY_ID, 4)]);
        let mut ledger = ResourceFlowLedger::default();
        let before =
            u128::from(source.quantity(&resource)) + u128::from(destination.quantity(&resource));

        transfer_resource(&mut source, &mut destination, &mut ledger, &resource, 5).unwrap();

        assert_eq!(source.quantity(&resource), 4);
        assert_eq!(destination.quantity(&resource), 9);
        assert_eq!(ledger.quantity_moved(&resource), 5);
        assert_ne!(ledger.quantity_moved(&resource), 0);
        assert_eq!(
            u128::from(source.quantity(&resource)) + u128::from(destination.quantity(&resource)),
            before
        );
    }

    fn assert_rejected_transfer_is_atomic(
        source: &mut ResourceStore,
        destination: &mut ResourceStore,
        ledger: &mut ResourceFlowLedger,
        quantity: u64,
        expected: CoreError,
    ) {
        let resource = id("core:ore");
        let before = (source.clone(), destination.clone(), ledger.clone());
        assert_eq!(
            transfer_resource(source, destination, ledger, &resource, quantity),
            Err(expected)
        );
        assert_eq!(
            (source.clone(), destination.clone(), ledger.clone()),
            before
        );
    }

    #[test]
    fn resource_transfer_rejections_are_atomic_on_every_path() {
        let resource = id("core:ore");

        let mut source = store(&[("core:ore", 3)]);
        let mut destination = store(&[("core:ore", 1)]);
        let mut ledger = ResourceFlowLedger::default();
        assert_rejected_transfer_is_atomic(
            &mut source,
            &mut destination,
            &mut ledger,
            4,
            CoreError::InsufficientResource {
                resource: resource.clone(),
                available: 3,
                requested: 4,
            },
        );

        let mut source = store(&[("core:ore", 3)]);
        let mut destination = store(&[("core:ore", u64::MAX)]);
        let mut ledger = ResourceFlowLedger::default();
        assert_rejected_transfer_is_atomic(
            &mut source,
            &mut destination,
            &mut ledger,
            1,
            CoreError::Overflow,
        );

        let mut source = store(&[("core:ore", 3)]);
        let mut destination = store(&[("core:ore", 1)]);
        let mut ledger = ResourceFlowLedger {
            moved: BTreeMap::from([(resource.clone(), u64::MAX)]),
        };
        assert_rejected_transfer_is_atomic(
            &mut source,
            &mut destination,
            &mut ledger,
            1,
            CoreError::Overflow,
        );

        let mut source = store(&[("core:ore", 3)]);
        let mut destination = store(&[("core:ore", 1)]);
        let mut ledger = ResourceFlowLedger::default();
        assert_rejected_transfer_is_atomic(
            &mut source,
            &mut destination,
            &mut ledger,
            0,
            CoreError::ZeroResourceTransfer,
        );
    }
}
