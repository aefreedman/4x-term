use crate::{ContentId, CoreError, FixedRate, ProjectId, ReservationOwner, SimulationTime};
use std::collections::BTreeMap;

pub const ENERGY_ID: &str = "core:energy";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceDefinition {
    pub id: ContentId,
    pub name: String,
    pub naturally_deposit_bearing: bool,
}

/// Checked physical quantities used for stocks and commitments.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DevelopmentCondition {
    Functional,
    Damaged,
    Ruined,
}

/// Frozen Stage 4b development catalog. Habitat and Shipyard behavior is supplied by leaf hooks.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DevelopmentRole {
    Collector,
    Battery,
    Extractor,
    Refinery,
    Habitat,
    Shipyard,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyResourceTarget {
    pub body: ContentId,
    pub resource: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentDefinition {
    pub id: ContentId,
    pub role: DevelopmentRole,
    pub condition: DevelopmentCondition,
    pub extractor_target: Option<BodyResourceTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentSlotDefinition {
    pub id: ContentId,
    pub development: Option<DevelopmentDefinition>,
}

/// Authored/generated body input. Initial developments seed runtime state but are not map facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyDefinition {
    pub id: ContentId,
    pub name: String,
    pub eccentricity_hundredths: u16,
    pub initial_resources: ResourceStore,
    pub slots: Vec<DevelopmentSlotDefinition>,
}

/// Normalized immutable body and slot map facts in semantic authored/generated order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyMapDefinition {
    pub id: ContentId,
    pub name: String,
    pub eccentricity_hundredths: u16,
    pub initial_resources: ResourceStore,
    pub slots: Vec<ContentId>,
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
    pub output: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefineryParameters {
    pub energy_upkeep: u64,
    pub cycle_duration: u64,
    pub input: u64,
    pub output: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeProjectTuning {
    pub material_commitment: ResourceStore,
    pub duration_ticks: u64,
    pub energy_per_progress_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpeditionProjectTuning {
    pub hull_material_commitment: ResourceStore,
    pub founding_stocks: ResourceStore,
    pub duration_ticks: u64,
    pub energy_per_progress_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShipTravelTuning {
    pub maximum_jump_quanta: u64,
    pub speed_quanta_per_tick: u64,
    pub energy_per_quantum: FixedRate,
}

/// Inclusive authored quantity ranges used by scouting summaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RichnessThresholds {
    pub poor_minimum: u64,
    pub poor_maximum: u64,
    pub normal_minimum: u64,
    pub normal_maximum: u64,
    pub rich_minimum: u64,
}

/// Required world-level tuning shared by every system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldTuning {
    pub energy_resource: ContentId,
    pub ore_resource: ContentId,
    pub alloy_resource: ContentId,
    pub seasonal_shape: [u64; 10],
    pub seasonal_baseline_average: u64,
    pub life_support_per_population: u64,
    pub origin_construction_work: u64,
    pub intrinsic_energy_capacity: u64,
    pub battery_energy_capacity: u64,
    pub habitat_population_energy: u64,
    pub coordinate_quanta_per_map_unit: u64,
    pub collector_recipe: ConstructionRecipe,
    pub battery_recipe: ConstructionRecipe,
    pub extractor_recipe: ConstructionRecipe,
    pub refinery_recipe: ConstructionRecipe,
    pub habitat_recipe: ConstructionRecipe,
    pub shipyard_recipe: ConstructionRecipe,
    pub extractor: ExtractorParameters,
    pub refinery: RefineryParameters,
    pub probe_project: ProbeProjectTuning,
    pub expedition_project: ExpeditionProjectTuning,
    pub probe_travel: ShipTravelTuning,
    pub expedition_travel: ShipTravelTuning,
    pub probe_reveal_radius_quanta: u64,
    pub communication_delay_per_quantum: FixedRate,
    pub resource_richness: BTreeMap<ContentId, RichnessThresholds>,
}

impl WorldTuning {
    /// Complete expedition enqueue commitment: hull, deployable Collector, and founding stocks.
    pub fn expedition_enqueue_commitment(&self) -> Result<ResourceStore, CoreError> {
        let mut commitment = self.expedition_project.hull_material_commitment.clone();
        for store in [
            &self.collector_recipe.cost,
            &self.expedition_project.founding_stocks,
        ] {
            for (resource, quantity) in &store.quantities {
                add(&mut commitment, resource, *quantity)?;
            }
        }
        Ok(commitment)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProductionCycle {
    pub progress: u64,
    pub committed_inputs: ResourceStore,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentState {
    pub definition: DevelopmentDefinition,
    /// Runtime operational control. Disabled developments retain all queued and
    /// in-progress state but contribute no production, capacity, or support.
    pub enabled: bool,
    pub cycle: ProductionCycle,
    pub habitat: Option<crate::HabitatState>,
    pub shipyard: Option<crate::ShipyardState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentSlotState {
    pub id: ContentId,
    pub development: Option<DevelopmentState>,
    pub reserved_by: Option<ReservationOwner>,
}

/// Mutable body runtime keyed to one immutable `BodyMapDefinition`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyState {
    pub id: ContentId,
    pub remaining_resources: ResourceStore,
    pub slots: Vec<DevelopmentSlotState>,
}

/// Privileged combined map/runtime body state used by deterministic debug snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodySnapshot {
    pub id: ContentId,
    pub name: String,
    pub eccentricity_hundredths: u16,
    pub initial_resources: ResourceStore,
    pub remaining_resources: ResourceStore,
    pub slots: Vec<DevelopmentSlotState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructionItem {
    pub id: ProjectId,
    pub development_id: ContentId,
    pub body: ContentId,
    pub slot: ContentId,
    pub role: DevelopmentRole,
    pub extractor_target: Option<BodyResourceTarget>,
    pub required_work: u64,
    pub work_applied: u64,
    pub committed_resources: ResourceStore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnergyOverflowCause {
    Retention,
    DevelopmentOperationalToggle,
    Transfer,
    CancellationRefund,
    ShipProjectCancellationRefund,
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
    /// Cumulative transfers from available stock into Shipyard commitments.
    pub ship_project_committed: ResourceStore,
    /// Cumulative complete cancellation transfers out of unstarted commitments.
    pub ship_project_refunded: ResourceStore,
    /// Command-time travel Energy expenditure.
    pub travel_spent: ResourceStore,
    /// Founding stocks physically received by successful target systems.
    pub founding_received: ResourceStore,
    /// Founding stocks physically lost with failed expeditions.
    pub expedition_lost: ResourceStore,
}

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
    let source_after = source
        .quantity(resource)
        .checked_sub(quantity)
        .ok_or_else(|| CoreError::InsufficientResource {
            resource: resource.clone(),
            available: source.quantity(resource),
            requested: quantity,
        })?;
    let destination_after = destination
        .quantity(resource)
        .checked_add(quantity)
        .ok_or(CoreError::Overflow)?;
    let ledger_after = ledger
        .quantity_moved(resource)
        .checked_add(quantity)
        .ok_or(CoreError::Overflow)?;
    source.set(resource.clone(), source_after);
    destination.set(resource.clone(), destination_after);
    ledger.moved.insert(resource.clone(), ledger_after);
    Ok(())
}

pub(crate) fn add(
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

pub(crate) fn sub(
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

pub(crate) fn record_overflow(
    accounting: &mut EnergyOverflowAccounting,
    time: SimulationTime,
    cause: EnergyOverflowCause,
    quantity: u64,
) -> Result<(), CoreError> {
    accounting.cumulative = accounting
        .cumulative
        .checked_add(quantity)
        .ok_or(CoreError::Overflow)?;
    accounting.evidence.push(EnergyOverflowEvidence {
        tick: time.tick,
        cause,
        quantity,
    });
    Ok(())
}
