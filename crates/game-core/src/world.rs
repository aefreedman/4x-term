use crate::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocationDefinition {
    pub id: ContentId,
    pub name: String,
    pub position: Position3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReclaimableSiteDefinition {
    pub id: ContentId,
    pub location: ContentId,
}

/// Immutable map facts plus initial runtime ownership for one always-present system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemDefinition {
    pub location: ContentId,
    pub stellar_strength_hundredths: u16,
    pub bodies: Vec<BodyDefinition>,
    pub stocks: ResourceStore,
    pub player_founded: bool,
    pub command_unlock_received: bool,
}

/// Normalized immutable system map facts. Body and slot vectors retain semantic source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemMapDefinition {
    pub location: ContentId,
    pub stellar_strength_hundredths: u16,
    pub bodies: Vec<BodyMapDefinition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Commandability {
    Neutral,
    Origin,
    AwaitingFoundingOutcome,
    Commandable,
    Depopulated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldDefinition {
    pub resources: Vec<ResourceDefinition>,
    pub locations: Vec<LocationDefinition>,
    pub origin_system: ContentId,
    pub origin_community: ContentId,
    pub communities: Vec<CommunityDefinition>,
    pub population_tokens: Vec<PopulationToken>,
    pub systems: Vec<SystemDefinition>,
    pub sites: Vec<ReclaimableSiteDefinition>,
    pub tuning: WorldTuning,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemState {
    pub stocks: ResourceStore,
    pub bodies: Vec<BodyState>,
    pub construction_queue: Vec<ConstructionItem>,
    pub counters: SystemCounters,
    pub life_support: LifeSupportEvidence,
    pub overflow: EnergyOverflowAccounting,
    pub accounting: ResourceAccounting,
    pub player_founded: bool,
    pub command_unlock_received: bool,
    pub completed_assets: Vec<CompletedAsset>,
}

/// Player-safe resident population evidence for one commandable local system.
/// Population token identities and transit state are intentionally omitted.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LocalPopulationSnapshot {
    pub population_count: u64,
    pub occupied_habitat_slots: Vec<SlotCoordinate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemSnapshot {
    pub location: ContentId,
    pub stellar_strength_hundredths: u16,
    pub stocks: ResourceStore,
    pub bodies: Vec<BodySnapshot>,
    pub construction_queue: Vec<ConstructionItem>,
    pub counters: SystemCounters,
    pub life_support: LifeSupportEvidence,
    pub energy_capacity: u64,
    pub energy_headroom: u64,
    pub energy_overflow: EnergyOverflowAccounting,
    pub accounting: ResourceAccounting,
    pub player_founded: bool,
    pub command_unlock_received: bool,
    pub commandability: Commandability,
    pub completed_assets: Vec<CompletedAsset>,
    pub local_population: LocalPopulationSnapshot,
}

impl SystemSnapshot {
    pub fn initial_resource_total(&self, resource: &ContentId) -> Result<u64, CoreError> {
        self.bodies.iter().try_fold(0_u64, |total, body| {
            total
                .checked_add(body.initial_resources.quantity(resource))
                .ok_or(CoreError::Overflow)
        })
    }

    pub fn remaining_resource_total(&self, resource: &ContentId) -> Result<u64, CoreError> {
        self.bodies.iter().try_fold(0_u64, |total, body| {
            total
                .checked_add(body.remaining_resources.quantity(resource))
                .ok_or(CoreError::Overflow)
        })
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunitySnapshot {
    pub id: ContentId,
    pub system: ContentId,
    pub population: u64,
}

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldSnapshot {
    pub time: SimulationTime,
    pub resources: Vec<ResourceDefinition>,
    pub locations: Vec<LocationDefinition>,
    pub map_systems: Vec<SystemMapDefinition>,
    pub origin_system: ContentId,
    pub origin_community: ContentId,
    pub communities: Vec<CommunitySnapshot>,
    pub populations: PopulationRegistry,
    pub population_accounting: PopulationAccounting,
    pub systems: Vec<SystemSnapshot>,
    pub sites: Vec<ReclaimableSiteDefinition>,
    pub tuning: WorldTuning,
    pub transit: Vec<TransitRecord>,
    pub knowledge: KnowledgeState,
}

/// Read-only construction availability and exact core-owned commitment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructionAssessment {
    pub system: ContentId,
    pub body: ContentId,
    pub slot: ContentId,
    pub role: DevelopmentRole,
    pub extractor_resource: Option<ContentId>,
    pub cost: ResourceStore,
    pub required_work: u64,
    pub limiting_reason: Option<CoreError>,
}

impl ConstructionAssessment {
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.limiting_reason.is_none()
    }
}

/// Read-only operational toggle availability for any installed development.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentOperationalAssessment {
    pub system: ContentId,
    pub body: ContentId,
    pub slot: ContentId,
    pub enabled: bool,
    pub limiting_reason: Option<CoreError>,
}

impl DevelopmentOperationalAssessment {
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.limiting_reason.is_none()
    }
}

/// Read-only Habitat generation toggle availability. Presentation labels remain adapter-owned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HabitatToggleAssessment {
    pub system: ContentId,
    pub body: ContentId,
    pub slot: ContentId,
    pub enabled: bool,
    pub limiting_reason: Option<CoreError>,
}

impl HabitatToggleAssessment {
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.limiting_reason.is_none()
    }
}

struct ConstructionPlan {
    system: SystemState,
    project_id: ProjectId,
}

struct DevelopmentOperationalPlan {
    system: SystemState,
}

struct HabitatTogglePlan {
    system: SystemState,
}

/// One identified system in the player-facing projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerSystemView {
    pub system: ContentId,
    /// Stable opaque map-visual assignment key; it carries no system facts.
    pub map_visual_key: u64,
    pub knowledge: SystemKnowledge,
    /// Authoritative local state exists only for origin or a founded system whose outcome arrived.
    pub local_state: Option<SystemSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrontierFogPoint {
    pub position: Position3,
    pub map_visual_key: u64,
}

/// Knowledge-filtered player projection. It exposes only redacted active physical
/// routes and has no global population registry, pending outcome, or global accounting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerWorldView {
    pub time: SimulationTime,
    /// Zero-based phase of the core's ten-phase seasonal collector cycle.
    pub seasonal_phase: u8,
    pub systems: Vec<PlayerSystemView>,
    pub anonymous_indication_count: usize,
    /// Anonymous map texture points for systems whose identity is not player-visible.
    /// Adapters may render these as fog but must not associate them with system rows.
    pub frontier_fog: Vec<FrontierFogPoint>,
    pub missions: BTreeMap<ShipId, MissionState>,
    /// Probe missions with at least one active leg or report not yet received.
    pub probe_reports: BTreeMap<ShipId, ProbeReportStatus>,
    /// Active physical routes, recomputed so each hidden stop is named only once reached.
    pub active_routes: BTreeMap<ShipId, RedactedRoute>,
}

#[cfg_attr(
    any(test, feature = "test-support"),
    derive(Clone, Debug, Eq, PartialEq)
)]
pub struct WorldState {
    pub(crate) time: SimulationTime,
    pub(crate) resources: Vec<ResourceDefinition>,
    pub(crate) locations: Vec<LocationDefinition>,
    pub(crate) origin_system: ContentId,
    pub(crate) origin_community: ContentId,
    pub(crate) communities: BTreeMap<ContentId, CommunityDefinition>,
    pub(crate) populations: PopulationRegistry,
    pub(crate) population_accounting: PopulationAccounting,
    pub(crate) map_systems: BTreeMap<ContentId, SystemMapDefinition>,
    pub(crate) systems: BTreeMap<ContentId, SystemState>,
    pub(crate) sites: Vec<ReclaimableSiteDefinition>,
    pub(crate) tuning: WorldTuning,
    pub(crate) transit: Vec<TransitRecord>,
    pub(crate) knowledge: KnowledgeState,
}

impl WorldState {
    pub fn new(definition: WorldDefinition) -> Result<Self, CoreError> {
        validate_and_normalize(definition)
    }

    #[must_use]
    pub fn time(&self) -> SimulationTime {
        self.time
    }

    pub(crate) fn clone_full(&self) -> Self {
        Self {
            time: self.time,
            resources: self.resources.clone(),
            locations: self.locations.clone(),
            origin_system: self.origin_system.clone(),
            origin_community: self.origin_community.clone(),
            communities: self.communities.clone(),
            populations: self.populations.clone(),
            population_accounting: self.population_accounting.clone(),
            map_systems: self.map_systems.clone(),
            systems: self.systems.clone(),
            sites: self.sites.clone(),
            tuning: self.tuning.clone(),
            transit: self.transit.clone(),
            knowledge: self.knowledge.clone(),
        }
    }

    /// Privileged complete state for deterministic engine diagnostics and tests.
    /// Player adapters must use `player_view`, which applies knowledge and mission redaction.
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    #[must_use]
    pub fn debug_snapshot(&self) -> WorldSnapshot {
        WorldSnapshot {
            time: self.time,
            resources: self.resources.clone(),
            locations: self.locations.clone(),
            map_systems: self.map_systems.values().cloned().collect(),
            origin_system: self.origin_system.clone(),
            origin_community: self.origin_community.clone(),
            communities: self
                .communities
                .values()
                .map(|community| CommunitySnapshot {
                    id: community.id.clone(),
                    system: community.system.clone(),
                    population: self.populations.community_population(&community.id),
                })
                .collect(),
            populations: self.populations.clone(),
            population_accounting: self.population_accounting.clone(),
            systems: self
                .systems
                .iter()
                .map(|(location, state)| {
                    snapshot_system(
                        self.map_systems
                            .get(location)
                            .expect("runtime system has immutable map definition"),
                        state,
                        &self.tuning,
                        derive_commandability(
                            location,
                            &self.origin_system,
                            state,
                            self.populations
                                .system_population(&self.communities, location),
                        ),
                        local_population_snapshot(
                            state,
                            &self.populations,
                            &self.communities,
                            location,
                        ),
                    )
                    .expect("validated system remains snapshotable")
                })
                .collect(),
            sites: self.sites.clone(),
            tuning: self.tuning.clone(),
            transit: self.transit.clone(),
            knowledge: self.knowledge.clone(),
        }
    }

    /// Returns the knowledge-filtered state intended for player adapters.
    pub fn player_view(&self) -> Result<PlayerWorldView, CoreError> {
        let mut systems = Vec::new();
        let mut anonymous_indication_count = 0;
        for (system_id, knowledge) in &self.knowledge.systems {
            match knowledge.level {
                KnowledgeLevel::Unknown => {}
                KnowledgeLevel::Anonymous => anonymous_indication_count += 1,
                KnowledgeLevel::IdentifiedSummary | KnowledgeLevel::Complete => {
                    let runtime = self
                        .systems
                        .get(system_id)
                        .ok_or_else(|| CoreError::UnknownSystem(system_id.clone()))?;
                    let mut local_state = (system_id == &self.origin_system
                        || (runtime.player_founded && runtime.command_unlock_received))
                        .then(|| self.system_snapshot(system_id))
                        .transpose()?;
                    if let Some(local_state) = &mut local_state {
                        self.redact_unreceived_expedition_losses(system_id, local_state)?;
                    }
                    systems.push(PlayerSystemView {
                        system: system_id.clone(),
                        map_visual_key: self
                            .locations
                            .iter()
                            .position(|location| &location.id == system_id)
                            .and_then(|index| u64::try_from(index).ok())
                            .expect("known system has an indexed location"),
                        knowledge: knowledge.clone(),
                        local_state,
                    });
                }
            }
        }
        let frontier_fog = self
            .locations
            .iter()
            .enumerate()
            .filter(|(_, location)| {
                self.knowledge
                    .systems
                    .get(&location.id)
                    .is_none_or(|knowledge| {
                        matches!(
                            knowledge.level,
                            KnowledgeLevel::Unknown | KnowledgeLevel::Anonymous
                        )
                    })
            })
            .filter_map(|(index, location)| {
                Some(FrontierFogPoint {
                    position: location.position,
                    map_visual_key: u64::try_from(index).ok()?,
                })
            })
            .collect();
        let identified = self.knowledge.identified_systems();
        let active_routes = self
            .transit
            .iter()
            .map(|transit| {
                (
                    transit.ship_id.clone(),
                    transit
                        .route
                        .player_route(&identified, &transit.reached_stops),
                )
            })
            .collect();
        let mut probe_reports = self
            .transit
            .iter()
            .filter(|transit| matches!(transit.kind, TransitKind::Probe { .. }))
            .map(|transit| (transit.ship_id.clone(), ProbeReportStatus::AwaitingReport))
            .collect::<BTreeMap<_, _>>();
        for transmission in self.knowledge.pending_transmissions.values() {
            let ObserverId::Ship(ship_id) = &transmission.id.observer else {
                continue;
            };
            if !self.knowledge.mission_states.contains_key(ship_id) {
                probe_reports.insert(ship_id.clone(), ProbeReportStatus::AwaitingReport);
            }
        }
        Ok(PlayerWorldView {
            time: self.time,
            seasonal_phase: u8::try_from(self.time.tick % 10)
                .expect("seasonal phase is always below ten"),
            systems,
            anonymous_indication_count,
            frontier_fog,
            missions: self.knowledge.mission_states.clone(),
            probe_reports,
            active_routes,
        })
    }

    fn redact_unreceived_expedition_losses(
        &self,
        system_id: &ContentId,
        local_state: &mut SystemSnapshot,
    ) -> Result<(), CoreError> {
        for outcome in self.knowledge.pending_mission_outcomes.values() {
            if let MissionState::FoundingLost {
                ship_id,
                founding_stocks,
                ..
            } = outcome
                && &ship_id.system == system_id
            {
                for (resource, quantity) in &founding_stocks.quantities {
                    sub(
                        &mut local_state.accounting.expedition_lost,
                        resource,
                        *quantity,
                    )?;
                }
            }
        }
        Ok(())
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn debug_system_snapshot(&self, location: &ContentId) -> Result<SystemSnapshot, CoreError> {
        self.system_snapshot(location)
    }

    fn system_snapshot(&self, location: &ContentId) -> Result<SystemSnapshot, CoreError> {
        let system = self
            .systems
            .get(location)
            .ok_or_else(|| CoreError::UnknownSystem(location.clone()))?;
        snapshot_system(
            self.map_systems
                .get(location)
                .expect("runtime system has immutable map definition"),
            system,
            &self.tuning,
            derive_commandability(
                location,
                &self.origin_system,
                system,
                self.populations
                    .system_population(&self.communities, location),
            ),
            local_population_snapshot(system, &self.populations, &self.communities, location),
        )
    }

    pub fn system_stocks(&self, location: &ContentId) -> Result<&ResourceStore, CoreError> {
        self.ensure_commandable(location)?;
        Ok(&self
            .systems
            .get(location)
            .ok_or_else(|| CoreError::UnknownSystem(location.clone()))?
            .stocks)
    }

    pub fn commandability(&self, system_id: &ContentId) -> Result<Commandability, CoreError> {
        let system = self
            .systems
            .get(system_id)
            .ok_or_else(|| CoreError::UnknownSystem(system_id.clone()))?;
        let physical = derive_commandability(
            system_id,
            &self.origin_system,
            system,
            self.populations
                .system_population(&self.communities, system_id),
        );
        if physical == Commandability::Neutral
            && self.knowledge.mission_states.values().any(|mission| {
                matches!(mission, MissionState::AwaitingOutcome { target } if target == system_id)
            })
        {
            Ok(Commandability::AwaitingFoundingOutcome)
        } else {
            Ok(physical)
        }
    }

    pub(crate) fn ensure_commandable(&self, system_id: &ContentId) -> Result<(), CoreError> {
        if matches!(
            self.commandability(system_id)?,
            Commandability::Origin | Commandability::Commandable
        ) {
            Ok(())
        } else {
            Err(CoreError::SystemNotCommandable(system_id.clone()))
        }
    }

    pub(crate) fn validate_runtime_integrity(&self) -> Result<(), CoreError> {
        if self.map_systems.len() != self.systems.len() {
            return Err(CoreError::MissingPersistentSystem);
        }
        for (system_id, state) in &self.systems {
            let map = self
                .map_systems
                .get(system_id)
                .ok_or_else(|| CoreError::MapRuntimeMismatch(system_id.clone()))?;
            validate_map_runtime_shape(map, state)?;
        }

        let mut expedition_populations = BTreeMap::new();
        let mut expedition_ships = BTreeSet::new();
        for transit in &self.transit {
            if !expedition_ships.insert(transit.ship_id.clone()) {
                return Err(CoreError::InvalidTransitPopulationBijection(format!(
                    "duplicate transit ship {:?}",
                    transit.ship_id
                )));
            }
            if let TransitKind::Expedition { population_id, .. } = &transit.kind
                && expedition_populations
                    .insert(population_id.clone(), transit.ship_id.clone())
                    .is_some()
            {
                return Err(CoreError::InvalidTransitPopulationBijection(format!(
                    "population {population_id:?} is carried by multiple expeditions"
                )));
            }
        }
        for token in self.populations.tokens.values() {
            if let PopulationState::InTransit { ship_id } = &token.state {
                if expedition_populations.get(&token.id) != Some(ship_id) {
                    return Err(CoreError::InvalidTransitPopulationBijection(format!(
                        "token {:?} does not match expedition {:?}",
                        token.id, ship_id
                    )));
                }
                expedition_populations.remove(&token.id);
            }
        }
        if let Some((population_id, ship_id)) = expedition_populations.into_iter().next() {
            return Err(CoreError::InvalidTransitPopulationBijection(format!(
                "expedition {ship_id:?} lacks token {population_id:?}"
            )));
        }
        validate_founded_communities(&self.origin_system, &self.communities, &self.systems)?;
        validate_populations(&self.populations, &self.communities, &self.systems)?;
        validate_population_accounting(&self.populations, &self.population_accounting)
    }

    /// Read-only assessment using the same private plan as the operational command.
    #[must_use]
    pub fn assess_development_operational_toggle(
        &self,
        system_id: &ContentId,
        body_id: &ContentId,
        slot_id: &ContentId,
        enabled: bool,
    ) -> DevelopmentOperationalAssessment {
        DevelopmentOperationalAssessment {
            system: system_id.clone(),
            body: body_id.clone(),
            slot: slot_id.clone(),
            enabled,
            limiting_reason: self
                .plan_development_operational_toggle(system_id, body_id, slot_id, enabled)
                .err(),
        }
    }

    /// Enables or disables any installed development. The command is atomic and
    /// preserves production cycles, Habitat generation, and Shipyard queues.
    pub fn set_development_operational_enabled(
        &mut self,
        system_id: &ContentId,
        body_id: &ContentId,
        slot_id: &ContentId,
        enabled: bool,
    ) -> Result<(), CoreError> {
        let plan =
            self.plan_development_operational_toggle(system_id, body_id, slot_id, enabled)?;
        *self
            .systems
            .get_mut(system_id)
            .expect("planned system remains present") = plan.system;
        Ok(())
    }

    fn plan_development_operational_toggle(
        &self,
        system_id: &ContentId,
        body_id: &ContentId,
        slot_id: &ContentId,
        enabled: bool,
    ) -> Result<DevelopmentOperationalPlan, CoreError> {
        self.ensure_commandable(system_id)?;
        let mut system = self
            .systems
            .get(system_id)
            .ok_or_else(|| CoreError::UnknownSystem(system_id.clone()))?
            .clone();
        let slot = find_slot_mut(&mut system.bodies, body_id, slot_id)?;
        let development =
            slot.development
                .as_mut()
                .ok_or_else(|| CoreError::DevelopmentSlotUnavailable {
                    body: body_id.clone(),
                    slot: slot_id.clone(),
                })?;
        development.enabled = enabled;
        let capacity = energy_capacity(&system, &self.tuning)?;
        let available = system.stocks.quantity(&self.tuning.energy_resource);
        if available > capacity {
            let overflow = available - capacity;
            system
                .stocks
                .set(self.tuning.energy_resource.clone(), capacity);
            record_overflow(
                &mut system.overflow,
                self.time,
                EnergyOverflowCause::DevelopmentOperationalToggle,
                overflow,
            )?;
        }
        Ok(DevelopmentOperationalPlan { system })
    }

    /// Read-only assessment of a Habitat generation toggle.
    #[must_use]
    pub fn assess_habitat_generation_toggle(
        &self,
        system_id: &ContentId,
        body_id: &ContentId,
        slot_id: &ContentId,
        enabled: bool,
    ) -> HabitatToggleAssessment {
        HabitatToggleAssessment {
            system: system_id.clone(),
            body: body_id.clone(),
            slot: slot_id.clone(),
            enabled,
            limiting_reason: self
                .plan_habitat_generation_toggle(system_id, body_id, slot_id, enabled)
                .err(),
        }
    }

    /// Enables or disables automatic generation on an empty, functional Habitat.
    /// The command is atomic and preserves existing progress and readiness.
    pub fn set_habitat_generation_enabled(
        &mut self,
        system_id: &ContentId,
        body_id: &ContentId,
        slot_id: &ContentId,
        enabled: bool,
    ) -> Result<(), CoreError> {
        let plan = self.plan_habitat_generation_toggle(system_id, body_id, slot_id, enabled)?;
        *self
            .systems
            .get_mut(system_id)
            .expect("planned system remains present") = plan.system;
        Ok(())
    }

    fn plan_habitat_generation_toggle(
        &self,
        system_id: &ContentId,
        body_id: &ContentId,
        slot_id: &ContentId,
        enabled: bool,
    ) -> Result<HabitatTogglePlan, CoreError> {
        self.ensure_commandable(system_id)?;
        let mut system = self
            .systems
            .get(system_id)
            .ok_or_else(|| CoreError::UnknownSystem(system_id.clone()))?
            .clone();
        crate::set_habitat_generation_enabled(
            &mut system.bodies,
            &self.populations,
            body_id,
            slot_id,
            enabled,
        )?;
        Ok(HabitatTogglePlan { system })
    }

    /// Applies the delayed successful-founding outcome received by origin knowledge.
    pub(crate) fn unlock_remote_commands(
        &mut self,
        system_id: &ContentId,
    ) -> Result<(), CoreError> {
        if system_id == &self.origin_system {
            return Ok(());
        }
        let system = self
            .systems
            .get_mut(system_id)
            .ok_or_else(|| CoreError::UnknownSystem(system_id.clone()))?;
        if !system.player_founded {
            return Err(CoreError::CannotUnlockNeutralSystem(system_id.clone()));
        }
        system.command_unlock_received = true;
        Ok(())
    }

    /// Read-only assessment using the same private plan as construction commit.
    #[must_use]
    pub fn assess_construction(
        &self,
        system_id: &ContentId,
        body_id: &ContentId,
        slot_id: &ContentId,
        role: DevelopmentRole,
        extractor_resource: Option<&ContentId>,
    ) -> ConstructionAssessment {
        let recipe = recipe_for(&self.tuning, role);
        ConstructionAssessment {
            system: system_id.clone(),
            body: body_id.clone(),
            slot: slot_id.clone(),
            role,
            extractor_resource: extractor_resource.cloned(),
            cost: recipe.cost.clone(),
            required_work: recipe.required_work,
            limiting_reason: self
                .plan_construction(system_id, body_id, slot_id, role, extractor_resource)
                .err(),
        }
    }

    pub fn enqueue_construction(
        &mut self,
        system_id: &ContentId,
        body_id: &ContentId,
        slot_id: &ContentId,
        role: DevelopmentRole,
        extractor_resource: Option<&ContentId>,
    ) -> Result<ProjectId, CoreError> {
        let plan = self.plan_construction(system_id, body_id, slot_id, role, extractor_resource)?;
        let project_id = plan.project_id.clone();
        *self
            .systems
            .get_mut(system_id)
            .expect("planned system remains present") = plan.system;
        Ok(project_id)
    }

    fn plan_construction(
        &self,
        system_id: &ContentId,
        body_id: &ContentId,
        slot_id: &ContentId,
        role: DevelopmentRole,
        extractor_resource: Option<&ContentId>,
    ) -> Result<ConstructionPlan, CoreError> {
        self.ensure_commandable(system_id)?;
        let current = self
            .systems
            .get(system_id)
            .ok_or_else(|| CoreError::UnknownSystem(system_id.clone()))?;
        let mut system = current.clone();
        let sequence = system.counters.next_project_sequence;
        system.counters.next_project_sequence =
            sequence.checked_add(1).ok_or(CoreError::Overflow)?;
        let project_id = ProjectId::new(system_id.clone(), sequence);
        let development_id =
            ContentId::new(format!("{}:development_{sequence}", system_id.as_str()))?;
        let recipe = recipe_for(&self.tuning, role).clone();
        let slot = find_slot(&system.bodies, body_id, slot_id)?;
        if slot.development.is_some() || slot.reserved_by.is_some() {
            return Err(CoreError::DevelopmentSlotUnavailable {
                body: body_id.clone(),
                slot: slot_id.clone(),
            });
        }
        let extractor_target = match (role, extractor_resource) {
            (DevelopmentRole::Extractor, Some(resource)) => {
                let body = self
                    .map_systems
                    .get(system_id)
                    .and_then(|map| map.bodies.iter().find(|body| &body.id == body_id))
                    .ok_or_else(|| CoreError::UnknownBody(body_id.clone()))?;
                if body.initial_resources.quantity(resource) == 0 {
                    return Err(CoreError::IncompatibleExtractorTarget {
                        body: body_id.clone(),
                        resource: resource.clone(),
                    });
                }
                Some(BodyResourceTarget {
                    body: body_id.clone(),
                    resource: resource.clone(),
                })
            }
            (DevelopmentRole::Extractor, None) => return Err(CoreError::ExtractorTargetRequired),
            (_, Some(_)) => return Err(CoreError::UnexpectedExtractorTarget),
            (_, None) => None,
        };
        for (resource, quantity) in &recipe.cost.quantities {
            sub(&mut system.stocks, resource, *quantity)?;
        }
        find_slot_mut(&mut system.bodies, body_id, slot_id)?.reserved_by =
            Some(ReservationOwner::Construction(project_id.clone()));
        system.construction_queue.push(ConstructionItem {
            id: project_id.clone(),
            development_id,
            body: body_id.clone(),
            slot: slot_id.clone(),
            role,
            extractor_target,
            required_work: recipe.required_work,
            work_applied: 0,
            committed_resources: recipe.cost,
        });
        Ok(ConstructionPlan { system, project_id })
    }

    pub fn cancel_construction(&mut self, project_id: &ProjectId) -> Result<(), CoreError> {
        self.ensure_commandable(&project_id.system)?;
        let current = self
            .systems
            .get(&project_id.system)
            .ok_or_else(|| CoreError::UnknownSystem(project_id.system.clone()))?;
        let mut system = current.clone();
        let index = system
            .construction_queue
            .iter()
            .position(|item| &item.id == project_id)
            .ok_or_else(|| CoreError::UnknownProject(project_id.clone()))?;
        let item = system.construction_queue[index].clone();
        if item.work_applied != 0 {
            return Err(CoreError::ConstructionAlreadyBegun(project_id.clone()));
        }
        let capacity = energy_capacity(&system, &self.tuning)?;
        for (resource, quantity) in &item.committed_resources.quantities {
            if resource == &self.tuning.energy_resource {
                let headroom = capacity.saturating_sub(system.stocks.quantity(resource));
                let retained = headroom.min(*quantity);
                add(&mut system.stocks, resource, retained)?;
                let overflow = quantity - retained;
                if overflow != 0 {
                    record_overflow(
                        &mut system.overflow,
                        self.time,
                        EnergyOverflowCause::CancellationRefund,
                        overflow,
                    )?;
                }
            } else {
                add(&mut system.stocks, resource, *quantity)?;
            }
        }
        let slot = find_slot_mut(&mut system.bodies, &item.body, &item.slot)?;
        if slot.reserved_by != Some(ReservationOwner::Construction(project_id.clone())) {
            return Err(CoreError::InvalidConstructionReservation(
                project_id.clone(),
            ));
        }
        slot.reserved_by = None;
        system.construction_queue.remove(index);
        *self
            .systems
            .get_mut(&project_id.system)
            .expect("system existed") = system;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn transfer_resource_to_system(
        &mut self,
        system_id: &ContentId,
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
            .any(|definition| &definition.id == resource)
        {
            return Err(CoreError::UnknownTransferResource(resource.clone()));
        }
        let current = self
            .systems
            .get(system_id)
            .ok_or_else(|| CoreError::UnknownSystem(system_id.clone()))?;
        let mut system = current.clone();
        let mut next_source = source.clone();
        let mut next_ledger = ledger.clone();
        sub(&mut next_source, resource, quantity)?;
        let retained = if resource == &self.tuning.energy_resource {
            quantity.min(
                energy_capacity(&system, &self.tuning)?
                    .saturating_sub(system.stocks.quantity(resource)),
            )
        } else {
            quantity
        };
        add(&mut system.stocks, resource, retained)?;
        let overflow = quantity - retained;
        if overflow != 0 {
            record_overflow(
                &mut system.overflow,
                self.time,
                EnergyOverflowCause::Transfer,
                overflow,
            )?;
        }
        let moved = next_ledger
            .quantity_moved(resource)
            .checked_add(quantity)
            .ok_or(CoreError::Overflow)?;
        next_ledger.moved.insert(resource.clone(), moved);
        *source = next_source;
        *ledger = next_ledger;
        *self.systems.get_mut(system_id).expect("system existed") = system;
        Ok(())
    }
}

pub(crate) fn recipe_for(tuning: &WorldTuning, role: DevelopmentRole) -> &ConstructionRecipe {
    match role {
        DevelopmentRole::Collector => &tuning.collector_recipe,
        DevelopmentRole::Battery => &tuning.battery_recipe,
        DevelopmentRole::Extractor => &tuning.extractor_recipe,
        DevelopmentRole::Refinery => &tuning.refinery_recipe,
        DevelopmentRole::Habitat => &tuning.habitat_recipe,
        DevelopmentRole::Shipyard => &tuning.shipyard_recipe,
    }
}

pub(crate) fn find_slot<'a>(
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

pub(crate) fn find_slot_mut<'a>(
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

pub(crate) fn energy_capacity(
    system: &SystemState,
    tuning: &WorldTuning,
) -> Result<u64, CoreError> {
    let batteries = u64::try_from(
        system
            .bodies
            .iter()
            .flat_map(|body| &body.slots)
            .filter(|slot| {
                slot.development.as_ref().is_some_and(|development| {
                    development.definition.role == DevelopmentRole::Battery
                        && development.definition.condition == DevelopmentCondition::Functional
                        && development.enabled
                })
            })
            .count(),
    )
    .map_err(|_| CoreError::Overflow)?;
    tuning
        .battery_energy_capacity
        .checked_mul(batteries)
        .and_then(|value| tuning.intrinsic_energy_capacity.checked_add(value))
        .ok_or(CoreError::Overflow)
}

fn snapshot_system(
    map: &SystemMapDefinition,
    state: &SystemState,
    tuning: &WorldTuning,
    commandability: Commandability,
    local_population: LocalPopulationSnapshot,
) -> Result<SystemSnapshot, CoreError> {
    let capacity = energy_capacity(state, tuning)?;
    let bodies = combine_body_snapshots(map, state)?;
    Ok(SystemSnapshot {
        location: map.location.clone(),
        stellar_strength_hundredths: map.stellar_strength_hundredths,
        stocks: state.stocks.clone(),
        bodies,
        construction_queue: state.construction_queue.clone(),
        counters: state.counters.clone(),
        life_support: state.life_support.clone(),
        energy_capacity: capacity,
        energy_headroom: capacity.saturating_sub(state.stocks.quantity(&tuning.energy_resource)),
        energy_overflow: state.overflow.clone(),
        accounting: state.accounting.clone(),
        player_founded: state.player_founded,
        command_unlock_received: state.command_unlock_received,
        commandability,
        completed_assets: state.completed_assets.clone(),
        local_population,
    })
}

fn local_population_snapshot(
    system: &SystemState,
    populations: &PopulationRegistry,
    communities: &BTreeMap<ContentId, CommunityDefinition>,
    system_id: &ContentId,
) -> LocalPopulationSnapshot {
    let population_count = populations.system_population(communities, system_id);
    let occupied_habitat_slots = system
        .bodies
        .iter()
        .flat_map(|body| {
            body.slots.iter().filter_map(|slot| {
                let development = slot.development.as_ref()?;
                (development.definition.role == DevelopmentRole::Habitat
                    && development.definition.condition == DevelopmentCondition::Functional
                    && populations
                        .habitat_occupant(&development.definition.id)
                        .is_some())
                .then(|| SlotCoordinate {
                    body: body.id.clone(),
                    slot: slot.id.clone(),
                })
            })
        })
        .collect();
    LocalPopulationSnapshot {
        population_count,
        occupied_habitat_slots,
    }
}

pub(crate) fn combine_body_snapshots(
    map: &SystemMapDefinition,
    state: &SystemState,
) -> Result<Vec<BodySnapshot>, CoreError> {
    validate_map_runtime_shape(map, state)?;
    map.bodies
        .iter()
        .zip(&state.bodies)
        .map(|(body_map, body_state)| {
            Ok(BodySnapshot {
                id: body_map.id.clone(),
                name: body_map.name.clone(),
                eccentricity_hundredths: body_map.eccentricity_hundredths,
                initial_resources: body_map.initial_resources.clone(),
                remaining_resources: body_state.remaining_resources.clone(),
                slots: body_state.slots.clone(),
            })
        })
        .collect()
}

fn validate_map_runtime_shape(
    map: &SystemMapDefinition,
    state: &SystemState,
) -> Result<(), CoreError> {
    let valid = map.bodies.len() == state.bodies.len()
        && map
            .bodies
            .iter()
            .zip(&state.bodies)
            .all(|(body_map, body_state)| {
                body_map.id == body_state.id
                    && body_map.slots.len() == body_state.slots.len()
                    && body_map
                        .slots
                        .iter()
                        .zip(&body_state.slots)
                        .all(|(slot_map, slot_state)| slot_map == &slot_state.id)
            });
    if valid {
        Ok(())
    } else {
        Err(CoreError::MapRuntimeMismatch(map.location.clone()))
    }
}

fn derive_commandability(
    system_id: &ContentId,
    origin_system: &ContentId,
    system: &SystemState,
    population: u64,
) -> Commandability {
    if system_id == origin_system {
        Commandability::Origin
    } else if !system.player_founded {
        Commandability::Neutral
    } else if !system.command_unlock_received {
        Commandability::AwaitingFoundingOutcome
    } else if population == 0 {
        Commandability::Depopulated
    } else {
        Commandability::Commandable
    }
}

pub fn validate_world_tuning(
    tuning: &WorldTuning,
    resource_definitions: &[ResourceDefinition],
) -> Result<(), CoreError> {
    let resources = resource_definitions
        .iter()
        .map(|resource| resource.id.clone())
        .collect::<BTreeSet<_>>();
    if resources.len() != resource_definitions.len() {
        return Err(CoreError::InvalidTuning(
            "resource IDs must be unique before tuning validation".into(),
        ));
    }
    validate_tuning(tuning, &resources)?;
    let energy = resource_definitions
        .iter()
        .find(|resource| resource.id == tuning.energy_resource)
        .expect("engine resource validated");
    if energy.naturally_deposit_bearing {
        return Err(CoreError::InvalidTuning(
            "Energy cannot be naturally deposit-bearing".into(),
        ));
    }
    for resource in resource_definitions
        .iter()
        .filter(|resource| resource.naturally_deposit_bearing)
    {
        if !tuning.resource_richness.contains_key(&resource.id) {
            return Err(CoreError::InvalidTuning(format!(
                "deposit-bearing resource {} requires richness thresholds",
                resource.id
            )));
        }
    }
    Ok(())
}

fn validate_tuning(tuning: &WorldTuning, resources: &BTreeSet<ContentId>) -> Result<(), CoreError> {
    for resource in [
        &tuning.energy_resource,
        &tuning.ore_resource,
        &tuning.alloy_resource,
    ] {
        if !resources.contains(resource) {
            return Err(CoreError::UnknownEngineResource(resource.clone()));
        }
    }
    if tuning.energy_resource.as_str() != ENERGY_ID {
        return Err(CoreError::InvalidTuning(
            "energy_resource must be core:energy".into(),
        ));
    }
    let seasonal_total = tuning
        .seasonal_shape
        .iter()
        .try_fold(0_u64, |total, value| {
            total.checked_add(*value).ok_or(CoreError::Overflow)
        })?;
    if seasonal_total
        != tuning
            .seasonal_baseline_average
            .checked_mul(10)
            .ok_or(CoreError::Overflow)?
    {
        return Err(CoreError::InvalidTuning(
            "seasonal shape total must equal ten times its baseline average".into(),
        ));
    }
    if tuning.seasonal_baseline_average == 0
        || tuning.life_support_per_population == 0
        || tuning.origin_construction_work == 0
        || tuning.intrinsic_energy_capacity == 0
        || tuning.battery_energy_capacity == 0
        || tuning.habitat_population_energy == 0
        || tuning.coordinate_quanta_per_map_unit == 0
        || tuning.extractor.energy_upkeep == 0
        || tuning.extractor.cycle_duration == 0
        || tuning.extractor.output == 0
        || tuning.refinery.energy_upkeep == 0
        || tuning.refinery.cycle_duration == 0
        || tuning.refinery.input == 0
        || tuning.refinery.output == 0
        || tuning.probe_project.duration_ticks == 0
        || tuning.probe_project.energy_per_progress_tick == 0
        || tuning.expedition_project.duration_ticks == 0
        || tuning.expedition_project.energy_per_progress_tick == 0
        || tuning.probe_travel.maximum_jump_quanta == 0
        || tuning.probe_travel.speed_quanta_per_tick == 0
        || tuning.probe_travel.energy_per_quantum.numerator == 0
        || tuning.expedition_travel.maximum_jump_quanta == 0
        || tuning.expedition_travel.speed_quanta_per_tick == 0
        || tuning.expedition_travel.energy_per_quantum.numerator == 0
        || tuning.probe_reveal_radius_quanta == 0
        || tuning.communication_delay_per_quantum.numerator == 0
    {
        return Err(CoreError::InvalidTuning(
            "required tuning values must be nonzero".into(),
        ));
    }
    for role in [
        DevelopmentRole::Collector,
        DevelopmentRole::Battery,
        DevelopmentRole::Extractor,
        DevelopmentRole::Refinery,
        DevelopmentRole::Habitat,
        DevelopmentRole::Shipyard,
    ] {
        let recipe = recipe_for(tuning, role);
        if recipe.required_work == 0 || recipe.cost.quantity(&tuning.energy_resource) == 0 {
            return Err(CoreError::InvalidConstructionRecipe {
                role,
                reason: "Energy cost and required work must be nonzero".into(),
            });
        }
        if recipe
            .cost
            .quantities
            .keys()
            .any(|resource| !resources.contains(resource))
        {
            return Err(CoreError::InvalidConstructionRecipe {
                role,
                reason: "unknown resource".into(),
            });
        }
    }
    for (name, store) in [
        (
            "probe material commitment",
            &tuning.probe_project.material_commitment,
        ),
        (
            "expedition hull material commitment",
            &tuning.expedition_project.hull_material_commitment,
        ),
        (
            "expedition founding stocks",
            &tuning.expedition_project.founding_stocks,
        ),
    ] {
        if store
            .quantities
            .keys()
            .any(|resource| !resources.contains(resource))
        {
            return Err(CoreError::InvalidTuning(format!(
                "{name} references an unknown resource"
            )));
        }
    }
    // Assessments expose this complete commitment as one exact store.
    tuning.expedition_enqueue_commitment()?;
    for (resource, thresholds) in &tuning.resource_richness {
        if !resources.contains(resource) {
            return Err(CoreError::InvalidTuning(format!(
                "richness thresholds reference unknown resource {resource}"
            )));
        }
        if thresholds.poor_minimum == 0
            || thresholds.poor_minimum > thresholds.poor_maximum
            || thresholds.normal_minimum
                != thresholds
                    .poor_maximum
                    .checked_add(1)
                    .ok_or(CoreError::Overflow)?
            || thresholds.normal_minimum > thresholds.normal_maximum
            || thresholds.rich_minimum
                != thresholds
                    .normal_maximum
                    .checked_add(1)
                    .ok_or(CoreError::Overflow)?
        {
            return Err(CoreError::InvalidTuning(format!(
                "richness thresholds for {resource} must be ordered contiguous nonzero ranges"
            )));
        }
    }
    Ok(())
}

fn validate_and_normalize(mut definition: WorldDefinition) -> Result<WorldState, CoreError> {
    definition.resources.sort_by(|a, b| a.id.cmp(&b.id));
    definition.locations.sort_by(|a, b| a.id.cmp(&b.id));
    definition
        .systems
        .sort_by(|a, b| a.location.cmp(&b.location));
    definition.communities.sort_by(|a, b| a.id.cmp(&b.id));
    definition.population_tokens.sort_by(|a, b| a.id.cmp(&b.id));
    definition.sites.sort_by(|a, b| a.id.cmp(&b.id));
    ensure_unique(
        definition.resources.iter().map(|value| &value.id),
        CoreError::DuplicateResourceId,
    )?;
    ensure_unique(
        definition.locations.iter().map(|value| &value.id),
        CoreError::DuplicateLocationId,
    )?;
    ensure_unique(
        definition.systems.iter().map(|value| &value.location),
        CoreError::DuplicateSystemLocation,
    )?;
    ensure_unique(
        definition.communities.iter().map(|value| &value.id),
        CoreError::DuplicateCommunityId,
    )?;
    ensure_unique(
        definition.sites.iter().map(|value| &value.id),
        CoreError::DuplicateSiteId,
    )?;
    let resources = definition
        .resources
        .iter()
        .map(|value| value.id.clone())
        .collect::<BTreeSet<_>>();
    validate_world_tuning(&definition.tuning, &definition.resources)?;
    let locations = definition
        .locations
        .iter()
        .map(|value| value.id.clone())
        .collect::<BTreeSet<_>>();
    if !locations.contains(&definition.origin_system) {
        return Err(CoreError::UnknownOriginSystem(definition.origin_system));
    }
    for site in &definition.sites {
        if !locations.contains(&site.location) {
            return Err(CoreError::UnknownSiteLocation {
                site: site.id.clone(),
                location: site.location.clone(),
            });
        }
    }
    let community_map = definition
        .communities
        .iter()
        .cloned()
        .map(|value| (value.id.clone(), value))
        .collect::<BTreeMap<_, _>>();
    let origin = community_map
        .get(&definition.origin_community)
        .ok_or_else(|| CoreError::UnknownOriginCommunity(definition.origin_community.clone()))?;
    if origin.system != definition.origin_system {
        return Err(CoreError::OriginCommunitySystemMismatch);
    }
    let mut community_systems = BTreeSet::new();
    for community in community_map.values() {
        if !locations.contains(&community.system) {
            return Err(CoreError::UnknownCommunitySystem {
                community: community.id.clone(),
                system: community.system.clone(),
            });
        }
        if !community_systems.insert(community.system.clone()) {
            return Err(CoreError::DuplicateCommunitySystem(
                community.system.clone(),
            ));
        }
    }

    let mut all_bodies = BTreeSet::new();
    let mut all_developments = BTreeSet::new();
    let mut map_systems = BTreeMap::new();
    let mut systems = BTreeMap::new();
    for definition_system in definition.systems {
        if !locations.contains(&definition_system.location) {
            return Err(CoreError::UnknownSystemLocation(definition_system.location));
        }
        if definition_system.stellar_strength_hundredths == 0 {
            return Err(CoreError::InvalidSystemStrength(definition_system.location));
        }
        let mut map_bodies = Vec::new();
        let mut bodies = Vec::new();
        for body in definition_system.bodies {
            if !all_bodies.insert(body.id.clone()) {
                return Err(CoreError::DuplicateBodyId(body.id));
            }
            for (resource, quantity) in &body.initial_resources.quantities {
                let Some(resource_definition) = definition
                    .resources
                    .iter()
                    .find(|candidate| &candidate.id == resource)
                else {
                    return Err(CoreError::UnknownBodyResource {
                        body: body.id.clone(),
                        resource: resource.clone(),
                    });
                };
                if *quantity == 0 || !resource_definition.naturally_deposit_bearing {
                    return Err(CoreError::InvalidBodyResource {
                        body: body.id.clone(),
                        resource: resource.clone(),
                    });
                }
            }
            ensure_unique(body.slots.iter().map(|value| &value.id), |slot| {
                CoreError::DuplicateSlotId {
                    body: body.id.clone(),
                    slot,
                }
            })?;
            let body_id = body.id.clone();
            let initial_resources = body.initial_resources.clone();
            let map_body = BodyMapDefinition {
                id: body.id.clone(),
                name: body.name,
                eccentricity_hundredths: body.eccentricity_hundredths,
                initial_resources: body.initial_resources.clone(),
                slots: body.slots.iter().map(|slot| slot.id.clone()).collect(),
            };
            let mut slots = Vec::new();
            for slot in body.slots {
                let development = slot
                    .development
                    .map(|development| {
                        if !all_developments.insert(development.id.clone()) {
                            return Err(CoreError::DuplicateDevelopmentId(development.id));
                        }
                        validate_development(&development, &body_id, &initial_resources)?;
                        Ok(DevelopmentState {
                            enabled: true,
                            habitat: (development.role == DevelopmentRole::Habitat)
                                .then(HabitatState::default),
                            shipyard: (development.role == DevelopmentRole::Shipyard)
                                .then(ShipyardState::default),
                            definition: development,
                            cycle: ProductionCycle::default(),
                        })
                    })
                    .transpose()?;
                slots.push(DevelopmentSlotState {
                    id: slot.id,
                    development,
                    reserved_by: None,
                });
            }
            map_bodies.push(map_body);
            bodies.push(BodyState {
                id: body.id,
                remaining_resources: body.initial_resources,
                slots,
            });
        }
        for resource in definition_system.stocks.quantities.keys() {
            if !resources.contains(resource) {
                return Err(CoreError::UnknownSystemStockResource {
                    location: definition_system.location.clone(),
                    resource: resource.clone(),
                });
            }
        }
        if definition_system.command_unlock_received && !definition_system.player_founded {
            return Err(CoreError::CannotUnlockNeutralSystem(
                definition_system.location.clone(),
            ));
        }
        let state = SystemState {
            stocks: definition_system.stocks,
            bodies,
            construction_queue: Vec::new(),
            counters: SystemCounters::default(),
            life_support: LifeSupportEvidence::default(),
            overflow: EnergyOverflowAccounting::default(),
            accounting: ResourceAccounting::default(),
            player_founded: definition_system.player_founded,
            command_unlock_received: definition_system.command_unlock_received,
            completed_assets: Vec::new(),
        };
        let capacity = energy_capacity(&state, &definition.tuning)?;
        let available = state.stocks.quantity(&definition.tuning.energy_resource);
        if available > capacity {
            return Err(CoreError::EnergyAboveCapacity {
                available,
                capacity,
            });
        }
        let location = definition_system.location;
        map_systems.insert(
            location.clone(),
            SystemMapDefinition {
                location: location.clone(),
                stellar_strength_hundredths: definition_system.stellar_strength_hundredths,
                bodies: map_bodies,
            },
        );
        systems.insert(location, state);
    }
    if systems.len() != locations.len() {
        return Err(CoreError::MissingPersistentSystem);
    }
    validate_founded_communities(&definition.origin_system, &community_map, &systems)?;
    let mut populations = PopulationRegistry::default();
    for token in definition.population_tokens {
        if matches!(&token.state, PopulationState::InTransit { .. }) {
            return Err(CoreError::InitialPopulationInTransit(token.id));
        }
        let system = systems
            .get_mut(&token.id.system)
            .ok_or_else(|| CoreError::UnknownPopulationBirthSystem(token.id.system.clone()))?;
        let next_sequence = token
            .id
            .sequence
            .checked_add(1)
            .ok_or(CoreError::Overflow)?;
        system.counters.next_population_sequence =
            system.counters.next_population_sequence.max(next_sequence);
        let population_id = token.id.clone();
        if populations
            .tokens
            .insert(population_id.clone(), token)
            .is_some()
        {
            return Err(CoreError::DuplicatePopulationId(population_id));
        }
    }
    validate_populations(&populations, &community_map, &systems)?;
    let initialized = u64::try_from(populations.tokens.len()).map_err(|_| CoreError::Overflow)?;
    let population_accounting = PopulationAccounting {
        initialized,
        ..PopulationAccounting::default()
    };
    validate_population_accounting(&populations, &population_accounting)?;
    let initial_knowledge_systems = initial_knowledge_systems(
        &definition.locations,
        &map_systems,
        &definition.tuning.resource_richness,
    )?;
    let knowledge = initial_origin_knowledge(
        &initial_knowledge_systems,
        &definition.origin_system,
        definition.tuning.probe_travel.maximum_jump_quanta,
        ObserverId::InitialOrigin(definition.origin_system.clone()),
    )
    .map_err(|error| CoreError::KnowledgeIntegration(error.to_string()))?;
    Ok(WorldState {
        time: SimulationTime::default(),
        resources: definition.resources,
        locations: definition.locations,
        origin_system: definition.origin_system,
        origin_community: definition.origin_community,
        communities: community_map,
        populations,
        population_accounting,
        map_systems,
        systems,
        sites: definition.sites,
        tuning: definition.tuning,
        transit: Vec::new(),
        knowledge,
    })
}

fn initial_knowledge_systems(
    locations: &[LocationDefinition],
    systems: &BTreeMap<ContentId, SystemMapDefinition>,
    thresholds: &BTreeMap<ContentId, RichnessThresholds>,
) -> Result<Vec<InitialKnowledgeSystem>, CoreError> {
    let positions = locations
        .iter()
        .map(|location| (location.id.clone(), location.position))
        .collect::<BTreeMap<_, _>>();
    systems
        .iter()
        .map(|(system_id, system)| {
            let mut resource_richness = BTreeMap::new();
            for (resource, ranges) in thresholds {
                let quantity = system.bodies.iter().try_fold(0_u64, |total, body| {
                    total
                        .checked_add(body.initial_resources.quantity(resource))
                        .ok_or(CoreError::Overflow)
                })?;
                if quantity != 0 {
                    let richness = if quantity >= ranges.poor_minimum
                        && quantity <= ranges.poor_maximum
                    {
                        ResourceRichness::Poor
                    } else if quantity >= ranges.normal_minimum && quantity <= ranges.normal_maximum
                    {
                        ResourceRichness::Normal
                    } else if quantity >= ranges.rich_minimum {
                        ResourceRichness::Rich
                    } else {
                        return Err(CoreError::InvalidTuning(format!(
                            "resource quantity {quantity} is outside richness ranges for {resource}"
                        )));
                    };
                    resource_richness.insert(resource.clone(), richness);
                }
            }
            Ok(InitialKnowledgeSystem {
                system: system_id.clone(),
                position: *positions
                    .get(system_id)
                    .ok_or_else(|| CoreError::UnknownSystemLocation(system_id.clone()))?,
                summary: InitialSystemSummary {
                    body_count: u64::try_from(system.bodies.len())
                        .map_err(|_| CoreError::Overflow)?,
                    stellar_strength_hundredths: system.stellar_strength_hundredths,
                    body_slot_counts: system
                        .bodies
                        .iter()
                        .map(|body| {
                            u64::try_from(body.slots.len()).map_err(|_| CoreError::Overflow)
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    resource_richness,
                },
            })
        })
        .collect()
}

fn validate_development(
    development: &DevelopmentDefinition,
    body_id: &ContentId,
    initial_resources: &ResourceStore,
) -> Result<(), CoreError> {
    match (development.role, &development.extractor_target) {
        (DevelopmentRole::Extractor, Some(target))
            if &target.body == body_id && initial_resources.quantity(&target.resource) != 0 =>
        {
            Ok(())
        }
        (DevelopmentRole::Extractor, Some(target)) => Err(CoreError::IncompatibleExtractorTarget {
            body: target.body.clone(),
            resource: target.resource.clone(),
        }),
        (DevelopmentRole::Extractor, None) => Err(CoreError::ExtractorTargetRequired),
        (_, Some(_)) => Err(CoreError::UnexpectedExtractorTarget),
        (_, None) => Ok(()),
    }
}

fn validate_founded_communities(
    origin_system: &ContentId,
    communities: &BTreeMap<ContentId, CommunityDefinition>,
    systems: &BTreeMap<ContentId, SystemState>,
) -> Result<(), CoreError> {
    for (system_id, system) in systems {
        if system_id == origin_system {
            continue;
        }
        let community_count = communities
            .values()
            .filter(|community| &community.system == system_id)
            .count();
        match (system.player_founded, community_count) {
            (true, 0) => {
                return Err(CoreError::FoundedSystemMissingCommunity(system_id.clone()));
            }
            (true, 1) | (false, 0) => {}
            (true, _) => return Err(CoreError::DuplicateCommunitySystem(system_id.clone())),
            (false, _) => return Err(CoreError::NeutralSystemHasCommunity(system_id.clone())),
        }
    }
    Ok(())
}

fn validate_populations(
    populations: &PopulationRegistry,
    communities: &BTreeMap<ContentId, CommunityDefinition>,
    systems: &BTreeMap<ContentId, SystemState>,
) -> Result<(), CoreError> {
    let mut occupied = BTreeSet::new();
    for (population_id, token) in &populations.tokens {
        if population_id != &token.id {
            return Err(CoreError::DuplicatePopulationId(token.id.clone()));
        }
        let birth_system = systems
            .get(&token.id.system)
            .ok_or_else(|| CoreError::UnknownPopulationBirthSystem(token.id.system.clone()))?;
        if token.id.sequence >= birth_system.counters.next_population_sequence {
            return Err(CoreError::PopulationSequenceNotAdvanced(token.id.clone()));
        }
        match &token.state {
            PopulationState::Resident {
                community_id,
                habitat_id,
            } => {
                let community = communities
                    .get(community_id)
                    .ok_or_else(|| CoreError::UnknownPopulationCommunity(community_id.clone()))?;
                let habitat_exists = systems.get(&community.system).is_some_and(|system| {
                    system
                        .bodies
                        .iter()
                        .flat_map(|body| &body.slots)
                        .any(|slot| {
                            slot.development.as_ref().is_some_and(|development| {
                                development.definition.id == *habitat_id
                                    && development.definition.role == DevelopmentRole::Habitat
                                    && development.definition.condition
                                        == DevelopmentCondition::Functional
                            })
                        })
                });
                if !habitat_exists {
                    return Err(CoreError::UnknownPopulationHabitat(habitat_id.clone()));
                }
                if !occupied.insert(habitat_id.clone()) {
                    return Err(CoreError::HabitatMultiplyOccupied(habitat_id.clone()));
                }
            }
            PopulationState::InTransit { .. } => {}
        }
    }
    Ok(())
}

fn validate_population_accounting(
    populations: &PopulationRegistry,
    accounting: &PopulationAccounting,
) -> Result<(), CoreError> {
    let created = accounting
        .initialized
        .checked_add(accounting.generated)
        .ok_or(CoreError::Overflow)?;
    let live = u64::try_from(populations.tokens.len()).map_err(|_| CoreError::Overflow)?;
    if created
        != live
            .checked_add(accounting.removed)
            .ok_or(CoreError::Overflow)?
    {
        return Err(CoreError::PopulationAccountingMismatch);
    }
    Ok(())
}

fn ensure_unique<'a>(
    ids: impl IntoIterator<Item = &'a ContentId>,
    error: impl Fn(ContentId) -> CoreError,
) -> Result<(), CoreError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(error(id.clone()));
        }
    }
    Ok(())
}
