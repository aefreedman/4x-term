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

/// Checked signed energy quantity retained for physical accounting.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Energy(pub i64);

impl Energy {
    pub const ZERO: Self = Self(0);

    pub fn checked_add(self, other: Self) -> Result<Self, CoreError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(CoreError::Overflow)
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, CoreError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(CoreError::Overflow)
    }

    pub fn checked_mul(self, quantity: u64) -> Result<Self, CoreError> {
        let quantity = i64::try_from(quantity).map_err(|_| CoreError::Overflow)?;
        self.0
            .checked_mul(quantity)
            .map(Self)
            .ok_or(CoreError::Overflow)
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

/// Physical quantities owned by a community rather than by geography.
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
    pub stocks: ResourceStore,
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

#[derive(Clone, Debug, PartialEq)]
pub struct WorldDefinition {
    pub resources: Vec<ResourceDefinition>,
    pub locations: Vec<LocationDefinition>,
    pub origin: OriginCommunityDefinition,
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
pub struct ResourceDeposit {
    pub location: ContentId,
    pub resource: ContentId,
    pub quantity: u64,
}

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

#[derive(Resource, Clone, Debug, Default, PartialEq)]
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
    pub stocks: ResourceStore,
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
    pub deposits: Vec<ResourceDepositDefinition>,
    pub sites: Vec<ReclaimableSiteDefinition>,
    pub topology: Vec<TopologyEdgeSnapshot>,
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

        for deposit in &definition.deposits {
            world.spawn((
                StableId(deposit.id.clone()),
                ResourceDeposit {
                    location: deposit.location.clone(),
                    resource: deposit.resource.clone(),
                    quantity: deposit.quantity,
                },
            ));
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
            definition.origin.stocks.clone(),
        ));
        world.insert_resource(definition.topology.clone());

        Ok(Self {
            world,
            resources: definition.resources,
            locations: definition.locations,
            deposits: definition.deposits,
            sites: definition.sites,
            topology: definition.topology,
            location_entities,
            origin_entity,
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
                            stocks: self
                                .world
                                .get::<ResourceStore>(entity)
                                .expect("community resource store is constructed atomically")
                                .clone(),
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

        WorldSnapshot {
            resources: self.resources.clone(),
            locations,
            origin,
            deposits: self.deposits.clone(),
            sites: self.sites.clone(),
            topology: self.topology.edges.clone(),
        }
    }

    #[must_use]
    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    #[must_use]
    pub fn origin_stocks(&self) -> &ResourceStore {
        self.world
            .get::<ResourceStore>(self.origin_entity)
            .expect("validated origin has stocks")
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

struct ValidatedWorldDefinition {
    resources: Vec<ResourceDefinition>,
    locations: Vec<LocationDefinition>,
    origin: OriginCommunityDefinition,
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

    if definition.origin.population == 0 {
        return Err(CoreError::ZeroOriginPopulation);
    }
    if !positions.contains_key(&definition.origin.location) {
        return Err(CoreError::UnknownOriginLocation(
            definition.origin.location.clone(),
        ));
    }
    for resource in definition.origin.stocks.quantities.keys() {
        if !resource_ids.contains(resource) {
            return Err(CoreError::UnknownStockResource {
                origin: definition.origin.id.clone(),
                resource: resource.clone(),
            });
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
    #[error("duplicate resource deposit id: {0}")]
    DuplicateDepositId(ContentId),
    #[error("duplicate reclaimable site id: {0}")]
    DuplicateSiteId(ContentId),
    #[error("location has a non-finite position: {0}")]
    NonFinitePosition(ContentId),
    #[error("origin population must be nonzero")]
    ZeroOriginPopulation,
    #[error("origin references unknown location: {0}")]
    UnknownOriginLocation(ContentId),
    #[error("origin {origin} stocks reference unknown resource: {resource}")]
    UnknownStockResource {
        origin: ContentId,
        resource: ContentId,
    },
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
    #[error("resource transfers must move a nonzero quantity")]
    ZeroResourceTransfer,
    #[error("insufficient {resource}: available {available}, requested {requested}")]
    InsufficientResource {
        resource: ContentId,
        available: u64,
        requested: u64,
    },
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
                stocks: store(&[(ENERGY_ID, 40), ("core:ore", 3)]),
            },
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
    fn energy_arithmetic_is_checked() {
        assert_eq!(Energy(7).checked_add(Energy(5)), Ok(Energy(12)));
        assert_eq!(Energy(7).checked_sub(Energy(5)), Ok(Energy(2)));
        assert_eq!(Energy(7).checked_mul(5), Ok(Energy(35)));
        assert_eq!(
            Energy(i64::MAX).checked_add(Energy(1)),
            Err(CoreError::Overflow)
        );
        assert_eq!(
            Energy(i64::MIN).checked_sub(Energy(1)),
            Err(CoreError::Overflow)
        );
        assert_eq!(Energy(i64::MAX).checked_mul(2), Err(CoreError::Overflow));
        assert_eq!(Energy(1).checked_mul(u64::MAX), Err(CoreError::Overflow));
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
        assert_eq!(snapshot.origin.stocks.quantity(&id(ENERGY_ID)), 40);
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
        assert_eq!(
            WorldState::new(zero_population).err(),
            Some(CoreError::ZeroOriginPopulation)
        );

        let mut zero_deposit = three_location_definition();
        zero_deposit.deposits[0].quantity = 0;
        assert!(matches!(
            WorldState::new(zero_deposit),
            Err(CoreError::ZeroDepositQuantity(_))
        ));

        let mut unknown_stock = three_location_definition();
        unknown_stock.origin.stocks.set(id("core:unknown"), 1);
        assert!(matches!(
            WorldState::new(unknown_stock),
            Err(CoreError::UnknownStockResource { .. })
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

    #[test]
    fn resource_transfer_succeeds_and_conserves_total() {
        let resource = id("core:ore");
        let mut source = store(&[("core:ore", 9)]);
        let mut destination = store(&[("core:ore", 4)]);
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
