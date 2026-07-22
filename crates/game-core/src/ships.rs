use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ShipProjectKind {
    Probe,
    Expedition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShipProjectIds {
    pub project_id: ProjectId,
    pub ship_id: ShipId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpeditionPayload {
    pub founding_stocks: ResourceStore,
    pub collector_id: ContentId,
    pub habitat_id: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShipyardProject {
    pub id: ProjectId,
    pub ship_id: ShipId,
    pub kind: ShipProjectKind,
    pub committed_resources: ResourceStore,
    pub construction_expenditure: ResourceStore,
    pub expedition_payload: Option<ExpeditionPayload>,
    pub progress: u64,
    pub required_progress: u64,
    pub energy_per_progress_tick: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShipyardState {
    pub queue: Vec<ShipyardProject>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompletedAsset {
    Probe {
        ship_id: ShipId,
        available_at_tick: u64,
    },
    Expedition {
        ship_id: ShipId,
        payload: ExpeditionPayload,
        available_at_tick: u64,
    },
}

impl CompletedAsset {
    #[must_use]
    pub fn ship_id(&self) -> &ShipId {
        match self {
            Self::Probe { ship_id, .. } | Self::Expedition { ship_id, .. } => ship_id,
        }
    }

    #[must_use]
    pub fn available_at_tick(&self) -> u64 {
        match self {
            Self::Probe {
                available_at_tick, ..
            }
            | Self::Expedition {
                available_at_tick, ..
            } => *available_at_tick,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlotCoordinate {
    pub body: ContentId,
    pub slot: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpeditionReservations {
    pub habitat: SlotCoordinate,
    pub collector: SlotCoordinate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FoundingLossReason {
    InsufficientSlots,
}

/// Minimal player-safe state for a launched probe whose observations are not all received.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeReportStatus {
    AwaitingReport,
}

/// Read-only probe launch availability. Labels and explanatory copy remain adapter-owned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeLaunchAssessment {
    pub source: ContentId,
    pub ship_id: ShipId,
    pub target: ContentId,
    pub requested_jump_limit: u64,
    pub minimum_jump_limit: u64,
    pub maximum_jump_limit: u64,
    pub target_knowledge: KnowledgeLevel,
    pub asset_ready: bool,
    pub travel_energy: Option<u64>,
    pub route: Option<RedactedRoute>,
    pub limiting_reason: Option<CoreError>,
}

impl ProbeLaunchAssessment {
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.limiting_reason.is_none()
    }
}

/// Read-only expedition launch availability and complete core-owned commitments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpeditionLaunchAssessment {
    pub source: ContentId,
    pub ship_id: ShipId,
    pub target: ContentId,
    pub reservations: Option<ExpeditionReservations>,
    pub complete_commitment: ResourceStore,
    pub resident_population_required: u64,
    pub resident_population_available: u64,
    pub resident_population_ready: bool,
    pub target_knowledge: KnowledgeLevel,
    pub asset_ready: bool,
    pub travel_energy: Option<u64>,
    pub route: Option<RedactedRoute>,
    pub limiting_reason: Option<CoreError>,
}

impl ExpeditionLaunchAssessment {
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.limiting_reason.is_none()
    }
}

/// Origin-facing mission state. Physical arrival never changes this state directly;
/// only receipt of the final transmission resolves `AwaitingOutcome`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MissionState {
    AwaitingOutcome {
        target: ContentId,
    },
    Founded {
        ship_id: ShipId,
        target: ContentId,
        community_id: ContentId,
        habitat_id: ContentId,
        collector_id: ContentId,
    },
    FoundingLost {
        ship_id: ShipId,
        target: ContentId,
        population_id: PopulationId,
        collector_id: ContentId,
        founding_stocks: ResourceStore,
        reason: FoundingLossReason,
    },
}

impl MissionState {
    #[must_use]
    pub fn target(&self) -> &ContentId {
        match self {
            Self::AwaitingOutcome { target }
            | Self::Founded { target, .. }
            | Self::FoundingLost { target, .. } => target,
        }
    }

    #[must_use]
    pub fn resolved_ship_id(&self) -> Option<&ShipId> {
        match self {
            Self::AwaitingOutcome { .. } => None,
            Self::Founded { ship_id, .. } | Self::FoundingLost { ship_id, .. } => Some(ship_id),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransitKind {
    Probe {
        jump_limit: u64,
    },
    Expedition {
        payload: ExpeditionPayload,
        population_id: PopulationId,
        reservations: Option<ExpeditionReservations>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitRecord {
    pub ship_id: ShipId,
    pub source: ContentId,
    pub target: ContentId,
    pub route: Route,
    /// Index of the leg currently being traversed.
    pub next_leg_index: usize,
    pub remaining_leg_ticks: u64,
    pub reached_stops: BTreeSet<ContentId>,
    pub paid_travel_energy: u64,
    pub observer_counters: ObserverCounters,
    pub kind: TransitKind,
}

/// Aggregate mutable state needed by phase 6 for one system.
pub(crate) struct ShipyardPhaseContext<'a> {
    pub time: SimulationTime,
    pub bodies: &'a mut [BodyState],
    pub stocks: &'a mut ResourceStore,
    pub completed_assets: &'a mut Vec<CompletedAsset>,
    pub accounting: &'a mut ResourceAccounting,
    pub tuning: &'a WorldTuning,
}

/// Advances at most the head project of every functional Shipyard, in stable
/// body/slot order. A head that cannot pay its complete Energy step pauses without
/// spending or progress; it does not block another Shipyard's independent queue.
pub(crate) fn progress_shipyards(context: ShipyardPhaseContext<'_>) -> Result<(), CoreError> {
    let coordinates = context
        .bodies
        .iter()
        .enumerate()
        .flat_map(|(body_index, body)| {
            body.slots
                .iter()
                .enumerate()
                .filter_map(move |(slot_index, slot)| {
                    slot.development
                        .as_ref()
                        .is_some_and(|development| {
                            development.definition.role == DevelopmentRole::Shipyard
                                && development.definition.condition
                                    == DevelopmentCondition::Functional
                                && development.enabled
                                && development.shipyard.is_some()
                        })
                        .then_some((body_index, slot_index))
                })
        })
        .collect::<Vec<_>>();

    for (body_index, slot_index) in coordinates {
        let project = context.bodies[body_index].slots[slot_index]
            .development
            .as_ref()
            .and_then(|development| development.shipyard.as_ref())
            .and_then(|shipyard| shipyard.queue.first())
            .cloned();
        let Some(project) = project else {
            continue;
        };
        if context.stocks.quantity(&context.tuning.energy_resource)
            < project.energy_per_progress_tick
        {
            continue;
        }

        let next_progress = project.progress.checked_add(1).ok_or(CoreError::Overflow)?;
        sub(
            context.stocks,
            &context.tuning.energy_resource,
            project.energy_per_progress_tick,
        )?;
        add(
            &mut context.accounting.operation_spent,
            &context.tuning.energy_resource,
            project.energy_per_progress_tick,
        )?;
        let queue = &mut context.bodies[body_index].slots[slot_index]
            .development
            .as_mut()
            .and_then(|development| development.shipyard.as_mut())
            .expect("functional Shipyard coordinate remains present")
            .queue;
        queue[0].progress = next_progress;
        if next_progress != project.required_progress {
            continue;
        }

        let completed = queue.remove(0);
        add_store(
            &mut context.accounting.construction_spent,
            &completed.construction_expenditure,
        )?;
        let available_at_tick = context
            .time
            .tick
            .checked_add(1)
            .ok_or(CoreError::Overflow)?;
        let asset = match completed.kind {
            ShipProjectKind::Probe => CompletedAsset::Probe {
                ship_id: completed.ship_id,
                available_at_tick,
            },
            ShipProjectKind::Expedition => CompletedAsset::Expedition {
                ship_id: completed.ship_id,
                payload: completed
                    .expedition_payload
                    .ok_or_else(|| CoreError::InvalidShipProject(completed.id.clone()))?,
                available_at_tick,
            },
        };
        context.completed_assets.push(asset);
        context
            .completed_assets
            .sort_by(|left, right| left.ship_id().cmp(right.ship_id()));
    }
    Ok(())
}

struct ProbeLaunchPlan {
    candidate: WorldState,
    route: Route,
}

struct ExpeditionLaunchPlan {
    candidate: WorldState,
    route: Route,
}

impl WorldState {
    /// Atomically commits one complete probe or expedition package to a specific
    /// functional Shipyard's independent FIFO queue.
    pub fn enqueue_ship_project(
        &mut self,
        system_id: &ContentId,
        shipyard_body: &ContentId,
        shipyard_slot: &ContentId,
        kind: ShipProjectKind,
    ) -> Result<ShipProjectIds, CoreError> {
        self.ensure_commandable(system_id)?;
        let current = self
            .systems
            .get(system_id)
            .ok_or_else(|| CoreError::UnknownSystem(system_id.clone()))?;
        let mut system = current.clone();
        ensure_functional_shipyard(&system.bodies, shipyard_body, shipyard_slot)?;

        let project_sequence = system.counters.next_project_sequence;
        let ship_sequence = system.counters.next_ship_sequence;
        let project_id = ProjectId::new(system_id.clone(), project_sequence);
        let ship_id = ShipId::new(system_id.clone(), ship_sequence);
        let next_project_sequence = project_sequence.checked_add(1).ok_or(CoreError::Overflow)?;
        let next_ship_sequence = ship_sequence.checked_add(1).ok_or(CoreError::Overflow)?;

        let (
            committed_resources,
            construction_expenditure,
            expedition_payload,
            required_progress,
            energy_per_progress_tick,
        ) = match kind {
            ShipProjectKind::Probe => (
                self.tuning.probe_project.material_commitment.clone(),
                self.tuning.probe_project.material_commitment.clone(),
                None,
                self.tuning.probe_project.duration_ticks,
                self.tuning.probe_project.energy_per_progress_tick,
            ),
            ShipProjectKind::Expedition => {
                let collector_id = expedition_development_id(&ship_id, "collector")?;
                let habitat_id = expedition_development_id(&ship_id, "habitat")?;
                if development_id_exists(&self.systems, &collector_id)
                    || development_id_exists(&self.systems, &habitat_id)
                {
                    return Err(CoreError::DuplicateDevelopmentId(
                        if development_id_exists(&self.systems, &collector_id) {
                            collector_id
                        } else {
                            habitat_id
                        },
                    ));
                }
                let mut construction_expenditure = self
                    .tuning
                    .expedition_project
                    .hull_material_commitment
                    .clone();
                add_store(
                    &mut construction_expenditure,
                    &self.tuning.collector_recipe.cost,
                )?;
                (
                    self.tuning.expedition_enqueue_commitment()?,
                    construction_expenditure,
                    Some(ExpeditionPayload {
                        founding_stocks: self.tuning.expedition_project.founding_stocks.clone(),
                        collector_id,
                        habitat_id,
                    }),
                    self.tuning.expedition_project.duration_ticks,
                    self.tuning.expedition_project.energy_per_progress_tick,
                )
            }
        };

        for (resource, quantity) in &committed_resources.quantities {
            sub(&mut system.stocks, resource, *quantity)?;
        }
        add_store(
            &mut system.accounting.ship_project_committed,
            &committed_resources,
        )?;
        let shipyard = functional_shipyard_mut(&mut system.bodies, shipyard_body, shipyard_slot)?;
        shipyard.queue.push(ShipyardProject {
            id: project_id.clone(),
            ship_id: ship_id.clone(),
            kind,
            committed_resources,
            construction_expenditure,
            expedition_payload,
            progress: 0,
            required_progress,
            energy_per_progress_tick,
        });
        system.counters.next_project_sequence = next_project_sequence;
        system.counters.next_ship_sequence = next_ship_sequence;
        *self.systems.get_mut(system_id).expect("system existed") = system;
        Ok(ShipProjectIds {
            project_id,
            ship_id,
        })
    }

    /// Cancels an unstarted ship project and returns its complete commitment.
    /// Energy that no longer fits is recorded as cancellation overflow.
    pub fn cancel_ship_project(&mut self, project_id: &ProjectId) -> Result<(), CoreError> {
        self.ensure_commandable(&project_id.system)?;
        let current = self
            .systems
            .get(&project_id.system)
            .ok_or_else(|| CoreError::UnknownSystem(project_id.system.clone()))?;
        let mut system = current.clone();
        let coordinate = find_ship_project(&system.bodies, project_id)
            .ok_or_else(|| CoreError::UnknownProject(project_id.clone()))?;
        let project = system.bodies[coordinate.0].slots[coordinate.1]
            .development
            .as_ref()
            .and_then(|development| development.shipyard.as_ref())
            .expect("project coordinate has Shipyard")
            .queue[coordinate.2]
            .clone();
        if project.progress != 0 {
            return Err(CoreError::ShipProjectAlreadyBegun(project_id.clone()));
        }

        let capacity = energy_capacity(&system, &self.tuning)?;
        for (resource, quantity) in &project.committed_resources.quantities {
            if resource == &self.tuning.energy_resource {
                let retained = capacity
                    .saturating_sub(system.stocks.quantity(resource))
                    .min(*quantity);
                add(&mut system.stocks, resource, retained)?;
                let overflow = quantity - retained;
                if overflow != 0 {
                    record_overflow(
                        &mut system.overflow,
                        self.time,
                        EnergyOverflowCause::ShipProjectCancellationRefund,
                        overflow,
                    )?;
                }
            } else {
                add(&mut system.stocks, resource, *quantity)?;
            }
        }
        add_store(
            &mut system.accounting.ship_project_refunded,
            &project.committed_resources,
        )?;
        system.bodies[coordinate.0].slots[coordinate.1]
            .development
            .as_mut()
            .and_then(|development| development.shipyard.as_mut())
            .expect("project coordinate has Shipyard")
            .queue
            .remove(coordinate.2);
        *self
            .systems
            .get_mut(&project_id.system)
            .expect("system existed") = system;
        Ok(())
    }

    /// Assesses probe launch without mutating authoritative state. Commit invokes
    /// the same private launch plan again, so stale state is revalidated.
    #[must_use]
    pub fn assess_probe_launch(
        &self,
        source: &ContentId,
        ship_id: &ShipId,
        target: &ContentId,
        desired_jump_limit: u64,
    ) -> ProbeLaunchAssessment {
        let source_commandable = self.ensure_commandable(source).is_ok();
        let asset_ready = source_commandable
            && self.systems.get(source).is_some_and(|system| {
                has_operational_shipyard(system)
                    && completed_asset_index(&system.completed_assets, ship_id).is_some_and(
                        |index| {
                            matches!(
                                &system.completed_assets[index],
                                CompletedAsset::Probe { available_at_tick, .. }
                                    if self.time.tick >= *available_at_tick
                            )
                        },
                    )
            });
        let route = (source_commandable
            && validate_target_knowledge(&self.knowledge, source, target).is_ok()
            && desired_jump_limit != 0
            && desired_jump_limit <= self.tuning.probe_travel.maximum_jump_quanta)
            .then(|| self.route(source, target, desired_jump_limit))
            .transpose()
            .ok()
            .flatten();
        let travel_energy = route.as_ref().and_then(|route| {
            route
                .checked_energy(self.tuning.probe_travel.energy_per_quantum)
                .ok()
        });
        let player_route = route.as_ref().map(|route| {
            route.player_route(
                &self.knowledge.identified_systems(),
                &BTreeSet::from([source.clone()]),
            )
        });
        ProbeLaunchAssessment {
            source: source.clone(),
            ship_id: ship_id.clone(),
            target: target.clone(),
            requested_jump_limit: desired_jump_limit,
            minimum_jump_limit: 1,
            maximum_jump_limit: self.tuning.probe_travel.maximum_jump_quanta,
            target_knowledge: self.knowledge.level(target),
            asset_ready,
            travel_energy,
            route: player_route,
            limiting_reason: self
                .plan_probe_launch(source, ship_id, target, desired_jump_limit)
                .err(),
        }
    }

    /// Launches a completed probe using an explicit jump limit no larger than the
    /// authored maximum. Route and complete travel Energy are committed atomically.
    pub fn launch_probe(
        &mut self,
        source: &ContentId,
        ship_id: &ShipId,
        target: &ContentId,
        desired_jump_limit: u64,
    ) -> Result<RedactedRoute, CoreError> {
        let plan = self.plan_probe_launch(source, ship_id, target, desired_jump_limit)?;
        let player_route = plan.route.player_route(
            &plan.candidate.knowledge.identified_systems(),
            &BTreeSet::from([source.clone()]),
        );
        *self = plan.candidate;
        Ok(player_route)
    }

    fn plan_probe_launch(
        &self,
        source: &ContentId,
        ship_id: &ShipId,
        target: &ContentId,
        desired_jump_limit: u64,
    ) -> Result<ProbeLaunchPlan, CoreError> {
        let mut candidate = self.clone_full();
        let route = candidate.launch_probe_inner(source, ship_id, target, desired_jump_limit)?;
        candidate.validate_runtime_integrity()?;
        Ok(ProbeLaunchPlan { candidate, route })
    }

    /// Assesses expedition launch without exposing population identities or
    /// authoritative target slots beyond the explicit reservation draft.
    #[must_use]
    pub fn assess_expedition_launch(
        &self,
        source: &ContentId,
        ship_id: &ShipId,
        target: &ContentId,
        reservations: Option<ExpeditionReservations>,
    ) -> ExpeditionLaunchAssessment {
        let source_commandable = self.ensure_commandable(source).is_ok();
        let target_knowledge = self.knowledge.level(target);
        let resident_population_available = if source_commandable {
            self.populations
                .system_population(&self.communities, source)
        } else {
            0
        };
        let resident_population_ready = source_commandable
            && self.systems.get(source).is_some_and(|system| {
                select_departing_population(system, &self.populations, &self.communities, source)
                    .is_ok()
            });
        let asset_ready = source_commandable
            && self.systems.get(source).is_some_and(|system| {
                has_operational_shipyard(system)
                    && completed_asset_index(&system.completed_assets, ship_id).is_some_and(
                        |index| {
                            matches!(
                                &system.completed_assets[index],
                                CompletedAsset::Expedition { available_at_tick, .. }
                                    if self.time.tick >= *available_at_tick
                            )
                        },
                    )
            });
        let route = (source_commandable
            && validate_target_knowledge(&self.knowledge, source, target).is_ok())
        .then(|| {
            self.route(
                source,
                target,
                self.tuning.expedition_travel.maximum_jump_quanta,
            )
        })
        .transpose()
        .ok()
        .flatten();
        let travel_energy = route.as_ref().and_then(|route| {
            route
                .checked_energy(self.tuning.expedition_travel.energy_per_quantum)
                .ok()
        });
        let player_route = route.as_ref().map(|route| {
            route.player_route(
                &self.knowledge.identified_systems(),
                &BTreeSet::from([source.clone()]),
            )
        });
        ExpeditionLaunchAssessment {
            source: source.clone(),
            ship_id: ship_id.clone(),
            target: target.clone(),
            reservations: reservations.clone(),
            complete_commitment: self
                .tuning
                .expedition_enqueue_commitment()
                .expect("validated tuning has a representable expedition commitment"),
            resident_population_required: 1,
            resident_population_available,
            resident_population_ready,
            target_knowledge,
            asset_ready,
            travel_energy,
            route: player_route,
            limiting_reason: self
                .plan_expedition_launch(source, ship_id, target, reservations)
                .err(),
        }
    }

    /// Launches a completed expedition. Complete target knowledge requires two
    /// named typed reservations; summary knowledge requires an unreserved launch.
    pub fn launch_expedition(
        &mut self,
        source: &ContentId,
        ship_id: &ShipId,
        target: &ContentId,
        reservations: Option<ExpeditionReservations>,
    ) -> Result<RedactedRoute, CoreError> {
        let plan = self.plan_expedition_launch(source, ship_id, target, reservations)?;
        let player_route = plan.route.player_route(
            &plan.candidate.knowledge.identified_systems(),
            &BTreeSet::from([source.clone()]),
        );
        *self = plan.candidate;
        Ok(player_route)
    }

    fn plan_expedition_launch(
        &self,
        source: &ContentId,
        ship_id: &ShipId,
        target: &ContentId,
        reservations: Option<ExpeditionReservations>,
    ) -> Result<ExpeditionLaunchPlan, CoreError> {
        let mut candidate = self.clone_full();
        let route = candidate.launch_expedition_inner(source, ship_id, target, reservations)?;
        candidate.validate_runtime_integrity()?;
        Ok(ExpeditionLaunchPlan { candidate, route })
    }

    fn launch_probe_inner(
        &mut self,
        source: &ContentId,
        ship_id: &ShipId,
        target: &ContentId,
        desired_jump_limit: u64,
    ) -> Result<Route, CoreError> {
        self.ensure_commandable(source)?;
        validate_target_knowledge(&self.knowledge, source, target)?;
        if desired_jump_limit == 0
            || desired_jump_limit > self.tuning.probe_travel.maximum_jump_quanta
        {
            return Err(CoreError::InvalidProbeJumpLimit {
                requested: desired_jump_limit,
                maximum: self.tuning.probe_travel.maximum_jump_quanta,
            });
        }
        let route = self.route(source, target, desired_jump_limit)?;
        let travel_energy = route.checked_energy(self.tuning.probe_travel.energy_per_quantum)?;
        let system = self
            .systems
            .get_mut(source)
            .ok_or_else(|| CoreError::UnknownSystem(source.clone()))?;
        if !has_operational_shipyard(system) {
            return Err(CoreError::NoOperationalShipyard(source.clone()));
        }
        let asset_index = completed_asset_index(&system.completed_assets, ship_id)
            .ok_or_else(|| CoreError::UnknownCompletedShip(ship_id.clone()))?;
        match &system.completed_assets[asset_index] {
            CompletedAsset::Probe {
                available_at_tick, ..
            } => ensure_asset_ready(self.time, *available_at_tick, ship_id)?,
            CompletedAsset::Expedition { .. } => {
                return Err(CoreError::WrongCompletedShipKind(ship_id.clone()));
            }
        }
        sub(
            &mut system.stocks,
            &self.tuning.energy_resource,
            travel_energy,
        )?;
        add(
            &mut system.accounting.travel_spent,
            &self.tuning.energy_resource,
            travel_energy,
        )?;
        system.completed_assets.remove(asset_index);
        let remaining_leg_ticks =
            leg_duration(&route, 0, self.tuning.probe_travel.speed_quanta_per_tick)?;
        self.transit.push(TransitRecord {
            ship_id: ship_id.clone(),
            source: source.clone(),
            target: target.clone(),
            route: route.clone(),
            next_leg_index: 0,
            remaining_leg_ticks,
            reached_stops: BTreeSet::from([source.clone()]),
            paid_travel_energy: travel_energy,
            observer_counters: ObserverCounters::default(),
            kind: TransitKind::Probe {
                jump_limit: desired_jump_limit,
            },
        });
        self.transit
            .sort_by(|left, right| left.ship_id.cmp(&right.ship_id));
        Ok(route)
    }

    fn launch_expedition_inner(
        &mut self,
        source: &ContentId,
        ship_id: &ShipId,
        target: &ContentId,
        reservations: Option<ExpeditionReservations>,
    ) -> Result<Route, CoreError> {
        self.ensure_commandable(source)?;
        let knowledge_level = validate_target_knowledge(&self.knowledge, source, target)?;
        let reservations = match (knowledge_level, reservations) {
            (KnowledgeLevel::Complete, Some(reservations)) => {
                validate_known_reservations(&self.knowledge, target, &reservations)?;
                Some(reservations)
            }
            (KnowledgeLevel::Complete, None) => {
                return Err(CoreError::CompleteKnowledgeRequiresReservations(
                    target.clone(),
                ));
            }
            (KnowledgeLevel::IdentifiedSummary, None) => None,
            (KnowledgeLevel::IdentifiedSummary, Some(_)) => {
                return Err(CoreError::SummaryKnowledgeCannotReserve(target.clone()));
            }
            _ => return Err(CoreError::SystemNotTargetable(target.clone())),
        };
        let route = self.route(
            source,
            target,
            self.tuning.expedition_travel.maximum_jump_quanta,
        )?;
        let travel_energy =
            route.checked_energy(self.tuning.expedition_travel.energy_per_quantum)?;

        let source_system = self
            .systems
            .get(source)
            .ok_or_else(|| CoreError::UnknownSystem(source.clone()))?;
        if !has_operational_shipyard(source_system) {
            return Err(CoreError::NoOperationalShipyard(source.clone()));
        }
        let asset_index = completed_asset_index(&source_system.completed_assets, ship_id)
            .ok_or_else(|| CoreError::UnknownCompletedShip(ship_id.clone()))?;
        let (payload, available_at_tick) = match &source_system.completed_assets[asset_index] {
            CompletedAsset::Expedition {
                payload,
                available_at_tick,
                ..
            } => (payload.clone(), *available_at_tick),
            CompletedAsset::Probe { .. } => {
                return Err(CoreError::WrongCompletedShipKind(ship_id.clone()));
            }
        };
        ensure_asset_ready(self.time, available_at_tick, ship_id)?;
        if source_system.stocks.quantity(&self.tuning.energy_resource) < travel_energy {
            return Err(CoreError::InsufficientResource {
                resource: self.tuning.energy_resource.clone(),
                available: source_system.stocks.quantity(&self.tuning.energy_resource),
                requested: travel_energy,
            });
        }
        let (population_id, source_community_id, source_habitat_id) = select_departing_population(
            source_system,
            &self.populations,
            &self.communities,
            source,
        )?;

        if let Some(reservations) = &reservations {
            let target_system = self
                .systems
                .get_mut(target)
                .ok_or_else(|| CoreError::UnknownSystem(target.clone()))?;
            reserve_expedition_slots(target_system, reservations, ship_id)?;
        }

        let token = self
            .populations
            .tokens
            .get_mut(&population_id)
            .ok_or_else(|| CoreError::UnknownTransitPopulation(population_id.clone()))?;
        token.state = PopulationState::InTransit {
            ship_id: ship_id.clone(),
        };
        self.population_accounting.record(
            self.time,
            population_id.clone(),
            PopulationTransition::EnteredTransit {
                ship_id: ship_id.clone(),
                source_community_id,
                source_habitat_id: source_habitat_id.clone(),
            },
        )?;
        let source_system = self
            .systems
            .get_mut(source)
            .expect("validated source system exists");
        vacate_habitat(&mut source_system.bodies, &source_habitat_id)?;
        sub(
            &mut source_system.stocks,
            &self.tuning.energy_resource,
            travel_energy,
        )?;
        add(
            &mut source_system.accounting.travel_spent,
            &self.tuning.energy_resource,
            travel_energy,
        )?;
        source_system.completed_assets.remove(asset_index);
        self.knowledge
            .register_mission(ship_id.clone(), target.clone())
            .map_err(|error| CoreError::KnowledgeIntegration(error.to_string()))?;

        let remaining_leg_ticks = leg_duration(
            &route,
            0,
            self.tuning.expedition_travel.speed_quanta_per_tick,
        )?;
        self.transit.push(TransitRecord {
            ship_id: ship_id.clone(),
            source: source.clone(),
            target: target.clone(),
            route: route.clone(),
            next_leg_index: 0,
            remaining_leg_ticks,
            reached_stops: BTreeSet::from([source.clone()]),
            paid_travel_energy: travel_energy,
            observer_counters: ObserverCounters::default(),
            kind: TransitKind::Expedition {
                payload,
                population_id,
                reservations,
            },
        });
        self.transit
            .sort_by(|left, right| left.ship_id.cmp(&right.ship_id));
        Ok(route)
    }

    fn route(
        &self,
        source: &ContentId,
        target: &ContentId,
        jump_limit: u64,
    ) -> Result<Route, CoreError> {
        if source == target {
            return Err(CoreError::ShipTargetMustBeDistinct(source.clone()));
        }
        let nodes = self
            .locations
            .iter()
            .map(|location| RouteNode {
                system: location.id.clone(),
                position: location.position,
            })
            .collect::<Vec<_>>();
        let route = shortest_route(&nodes, source, target, jump_limit)?.ok_or_else(|| {
            CoreError::NoShipRoute {
                from_system: source.clone(),
                target: target.clone(),
                jump_limit,
            }
        })?;
        if route.legs.is_empty() {
            return Err(CoreError::ShipTargetMustBeDistinct(source.clone()));
        }
        Ok(route)
    }
}

/// Aggregate context required for movement, arrival, population transfer, observations, and reports.
pub(crate) struct MovementPhaseContext<'a> {
    pub time: SimulationTime,
    pub origin_system: &'a ContentId,
    pub locations: &'a [LocationDefinition],
    pub map_systems: &'a BTreeMap<ContentId, SystemMapDefinition>,
    pub systems: &'a mut BTreeMap<ContentId, SystemState>,
    pub communities: &'a mut BTreeMap<ContentId, CommunityDefinition>,
    pub populations: &'a mut PopulationRegistry,
    pub population_accounting: &'a mut PopulationAccounting,
    pub ships: &'a mut Vec<TransitRecord>,
    pub knowledge: &'a mut KnowledgeState,
    pub tuning: &'a WorldTuning,
}

/// Phase 9 movement. Ships are processed in stable ship-ID order, each advances
/// at most one tick of its current leg, and every reached stop observes in that
/// same phase. Final expedition settlement/loss occurs before its observation.
pub(crate) fn move_ships(mut context: MovementPhaseContext<'_>) -> Result<(), CoreError> {
    context
        .ships
        .sort_by(|left, right| left.ship_id.cmp(&right.ship_id));
    let mut index = 0;
    while index < context.ships.len() {
        let mut ship = context.ships[index].clone();
        if ship.remaining_leg_ticks != 0 {
            ship.remaining_leg_ticks -= 1;
        }
        if ship.remaining_leg_ticks != 0 {
            context.ships[index] = ship;
            index += 1;
            continue;
        }

        let stop = ship.route.legs[ship.next_leg_index].to.clone();
        let final_stop = ship.next_leg_index + 1 == ship.route.legs.len();
        let outcome = if final_stop && matches!(ship.kind, TransitKind::Expedition { .. }) {
            Some(resolve_expedition_arrival(&mut context, &ship)?)
        } else {
            None
        };
        observe_stop(&mut context, &mut ship, &stop, outcome)?;
        ship.reached_stops.insert(stop);

        if final_stop {
            context.ships.remove(index);
            continue;
        }
        ship.next_leg_index += 1;
        ship.remaining_leg_ticks = leg_duration(
            &ship.route,
            ship.next_leg_index,
            match ship.kind {
                TransitKind::Probe { .. } => context.tuning.probe_travel.speed_quanta_per_tick,
                TransitKind::Expedition { .. } => {
                    context.tuning.expedition_travel.speed_quanta_per_tick
                }
            },
        )?;
        context.ships[index] = ship;
        index += 1;
    }
    Ok(())
}

fn resolve_expedition_arrival(
    context: &mut MovementPhaseContext<'_>,
    ship: &TransitRecord,
) -> Result<MissionState, CoreError> {
    let TransitKind::Expedition {
        payload,
        population_id,
        reservations,
    } = &ship.kind
    else {
        return Err(CoreError::WrongCompletedShipKind(ship.ship_id.clone()));
    };
    let target_system = context
        .systems
        .get(&ship.target)
        .ok_or_else(|| CoreError::UnknownSystem(ship.target.clone()))?;
    let selected = if let Some(reservations) = reservations {
        validate_authoritative_reservations(target_system, reservations, &ship.ship_id, true)?;
        Some((reservations.habitat.clone(), reservations.collector.clone()))
    } else {
        let available = available_slots(target_system);
        (available.len() >= 2).then(|| (available[0].clone(), available[1].clone()))
    };

    let Some((habitat_slot, collector_slot)) = selected else {
        let token = context
            .populations
            .tokens
            .get(population_id)
            .ok_or_else(|| CoreError::UnknownTransitPopulation(population_id.clone()))?;
        if token.state
            != (PopulationState::InTransit {
                ship_id: ship.ship_id.clone(),
            })
        {
            return Err(CoreError::InvalidTransitPopulation(population_id.clone()));
        }
        context.populations.tokens.remove(population_id);
        context.population_accounting.record(
            context.time,
            population_id.clone(),
            PopulationTransition::ExpeditionLost {
                ship_id: ship.ship_id.clone(),
            },
        )?;
        let source_system = context
            .systems
            .get_mut(&ship.source)
            .ok_or_else(|| CoreError::UnknownSystem(ship.source.clone()))?;
        add_store(
            &mut source_system.accounting.expedition_lost,
            &payload.founding_stocks,
        )?;
        return Ok(MissionState::FoundingLost {
            ship_id: ship.ship_id.clone(),
            target: ship.target.clone(),
            population_id: population_id.clone(),
            collector_id: payload.collector_id.clone(),
            founding_stocks: payload.founding_stocks.clone(),
            reason: FoundingLossReason::InsufficientSlots,
        });
    };

    if development_id_exists(context.systems, &payload.habitat_id)
        || development_id_exists(context.systems, &payload.collector_id)
    {
        return Err(CoreError::DuplicateDevelopmentId(
            if development_id_exists(context.systems, &payload.habitat_id) {
                payload.habitat_id.clone()
            } else {
                payload.collector_id.clone()
            },
        ));
    }
    let token = context
        .populations
        .tokens
        .get(population_id)
        .ok_or_else(|| CoreError::UnknownTransitPopulation(population_id.clone()))?;
    if token.state
        != (PopulationState::InTransit {
            ship_id: ship.ship_id.clone(),
        })
    {
        return Err(CoreError::InvalidTransitPopulation(population_id.clone()));
    }

    let community_id = if let Some(community) = context
        .communities
        .values()
        .find(|community| community.system == ship.target)
    {
        community.id.clone()
    } else {
        let community_id = founded_community_id(&ship.target)?;
        if context.communities.contains_key(&community_id) {
            return Err(CoreError::DuplicateCommunityId(community_id));
        }
        context.communities.insert(
            community_id.clone(),
            CommunityDefinition {
                id: community_id.clone(),
                system: ship.target.clone(),
            },
        );
        community_id
    };

    let target_system = context
        .systems
        .get_mut(&ship.target)
        .expect("validated target exists");
    install_arrival_development(
        target_system,
        &habitat_slot,
        &payload.habitat_id,
        DevelopmentRole::Habitat,
        &ship.ship_id,
        reservations.is_some(),
    )?;
    install_arrival_development(
        target_system,
        &collector_slot,
        &payload.collector_id,
        DevelopmentRole::Collector,
        &ship.ship_id,
        reservations.is_some(),
    )?;
    add_store(&mut target_system.stocks, &payload.founding_stocks)?;
    add_store(
        &mut target_system.accounting.founding_received,
        &payload.founding_stocks,
    )?;
    target_system.player_founded = true;

    context
        .populations
        .tokens
        .get_mut(population_id)
        .expect("validated population exists")
        .state = PopulationState::Resident {
        community_id: community_id.clone(),
        habitat_id: payload.habitat_id.clone(),
    };
    context.population_accounting.record(
        context.time,
        population_id.clone(),
        PopulationTransition::ResidenceTransferred {
            ship_id: ship.ship_id.clone(),
            target_community_id: community_id.clone(),
            target_habitat_id: payload.habitat_id.clone(),
        },
    )?;

    Ok(MissionState::Founded {
        ship_id: ship.ship_id.clone(),
        target: ship.target.clone(),
        community_id,
        habitat_id: payload.habitat_id.clone(),
        collector_id: payload.collector_id.clone(),
    })
}

fn observe_stop(
    context: &mut MovementPhaseContext<'_>,
    ship: &mut TransitRecord,
    stop: &ContentId,
    outcome: Option<MissionState>,
) -> Result<(), CoreError> {
    let system = context
        .systems
        .get(stop)
        .ok_or_else(|| CoreError::UnknownSystem(stop.clone()))?;
    let map = context
        .map_systems
        .get(stop)
        .ok_or_else(|| CoreError::UnknownSystem(stop.clone()))?;
    let inhabited = context
        .populations
        .system_population(context.communities, stop)
        != 0;
    let material_resources = context
        .tuning
        .resource_richness
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let stop_position = location_position(context.locations, stop)?;
    let mut observations = vec![complete_system_observation(
        map,
        stop_position,
        &system.bodies,
        &material_resources,
        inhabited,
    )?];
    if matches!(ship.kind, TransitKind::Probe { .. }) {
        for location in context.locations {
            if &location.id != stop
                && stop_position.checked_within_jump(
                    location.position,
                    context.tuning.probe_reveal_radius_quanta,
                )?
            {
                observations.push(anonymous_existence_observation(location.id.clone()));
            }
        }
    }

    let sequence = ship.observer_counters.next_transmission_sequence;
    ship.observer_counters.next_transmission_sequence =
        sequence.checked_add(1).ok_or(CoreError::Overflow)?;
    let transmission = PendingTransmission::scheduled_observations(
        TransmissionId {
            observer: ObserverId::Ship(ship.ship_id.clone()),
            sequence,
        },
        context.time.tick,
        stop_position,
        location_position(context.locations, context.origin_system)?,
        context.tuning.communication_delay_per_quantum,
        observations,
    )
    .map_err(|error| CoreError::KnowledgeIntegration(error.to_string()))?;
    match outcome {
        Some(outcome) => {
            context
                .knowledge
                .submit_mission_transmission(context.time.tick, transmission, outcome)
        }
        None => context
            .knowledge
            .submit_transmission(context.time.tick, transmission),
    }
    .map_err(|error| CoreError::KnowledgeIntegration(error.to_string()))?;
    Ok(())
}

fn validate_target_knowledge(
    knowledge: &KnowledgeState,
    source: &ContentId,
    target: &ContentId,
) -> Result<KnowledgeLevel, CoreError> {
    if source == target {
        return Err(CoreError::ShipTargetMustBeDistinct(source.clone()));
    }
    let level = knowledge.level(target);
    if matches!(
        level,
        KnowledgeLevel::IdentifiedSummary | KnowledgeLevel::Complete
    ) {
        Ok(level)
    } else {
        Err(CoreError::SystemNotTargetable(target.clone()))
    }
}

fn validate_known_reservations(
    knowledge: &KnowledgeState,
    target: &ContentId,
    reservations: &ExpeditionReservations,
) -> Result<(), CoreError> {
    if reservations.habitat == reservations.collector {
        return Err(CoreError::InvalidExpeditionReservation(
            "Habitat and Collector reservations must be distinct".into(),
        ));
    }
    for coordinate in [&reservations.habitat, &reservations.collector] {
        let Some(system) = knowledge.systems.get(target) else {
            return Err(CoreError::IncompleteTargetSlotKnowledge(target.clone()));
        };
        let known = system.facts.get(&FactKey::SlotOrder {
            body: coordinate.body.clone(),
        });
        if !known.is_some_and(|fact| {
            fact.detail == FactDetail::Complete
                && matches!(&fact.value, FactValue::ContentIds(slots) if slots.contains(&coordinate.slot))
        }) {
            return Err(CoreError::IncompleteTargetSlotKnowledge(target.clone()));
        }
    }
    Ok(())
}

fn validate_authoritative_reservations(
    system: &SystemState,
    reservations: &ExpeditionReservations,
    ship_id: &ShipId,
    require_owned_reservation: bool,
) -> Result<(), CoreError> {
    if reservations.habitat == reservations.collector {
        return Err(CoreError::InvalidExpeditionReservation(
            "Habitat and Collector reservations must be distinct".into(),
        ));
    }
    for coordinate in [&reservations.habitat, &reservations.collector] {
        let slot = find_slot(&system.bodies, &coordinate.body, &coordinate.slot)?;
        let expected = ReservationOwner::Expedition(ship_id.clone());
        let valid = if require_owned_reservation {
            slot.development.is_none() && slot.reserved_by == Some(expected)
        } else {
            slot.development.is_none() && slot.reserved_by.is_none()
        };
        if !valid {
            return Err(CoreError::InvalidExpeditionReservation(format!(
                "slot {}/{} is not available for ship {:?}",
                coordinate.body, coordinate.slot, ship_id
            )));
        }
    }
    Ok(())
}

fn reserve_expedition_slots(
    system: &mut SystemState,
    reservations: &ExpeditionReservations,
    ship_id: &ShipId,
) -> Result<(), CoreError> {
    validate_authoritative_reservations(system, reservations, ship_id, false)?;
    let owner = Some(ReservationOwner::Expedition(ship_id.clone()));
    find_slot_mut(
        &mut system.bodies,
        &reservations.habitat.body,
        &reservations.habitat.slot,
    )?
    .reserved_by = owner.clone();
    find_slot_mut(
        &mut system.bodies,
        &reservations.collector.body,
        &reservations.collector.slot,
    )?
    .reserved_by = owner;
    Ok(())
}

fn available_slots(system: &SystemState) -> Vec<SlotCoordinate> {
    system
        .bodies
        .iter()
        .flat_map(|body| {
            body.slots
                .iter()
                .filter(|slot| slot.development.is_none() && slot.reserved_by.is_none())
                .map(|slot| SlotCoordinate {
                    body: body.id.clone(),
                    slot: slot.id.clone(),
                })
        })
        .collect()
}

fn install_arrival_development(
    system: &mut SystemState,
    coordinate: &SlotCoordinate,
    development_id: &ContentId,
    role: DevelopmentRole,
    ship_id: &ShipId,
    reserved: bool,
) -> Result<(), CoreError> {
    let slot = find_slot_mut(&mut system.bodies, &coordinate.body, &coordinate.slot)?;
    let expected_reservation = reserved.then(|| ReservationOwner::Expedition(ship_id.clone()));
    if slot.development.is_some() || slot.reserved_by != expected_reservation {
        return Err(CoreError::InvalidExpeditionReservation(format!(
            "arrival slot {}/{} no longer matches ship {:?}",
            coordinate.body, coordinate.slot, ship_id
        )));
    }
    let definition = DevelopmentDefinition {
        id: development_id.clone(),
        role,
        condition: DevelopmentCondition::Functional,
        extractor_target: None,
    };
    slot.development = Some(DevelopmentState {
        enabled: true,
        habitat: (role == DevelopmentRole::Habitat).then(HabitatState::default),
        shipyard: None,
        definition,
        cycle: ProductionCycle::default(),
    });
    slot.reserved_by = None;
    Ok(())
}

fn select_departing_population(
    system: &SystemState,
    populations: &PopulationRegistry,
    communities: &BTreeMap<ContentId, CommunityDefinition>,
    source: &ContentId,
) -> Result<(PopulationId, ContentId, ContentId), CoreError> {
    for slot in system.bodies.iter().flat_map(|body| &body.slots) {
        let Some(development) = &slot.development else {
            continue;
        };
        if development.definition.role != DevelopmentRole::Habitat
            || development.definition.condition != DevelopmentCondition::Functional
        {
            continue;
        }
        let habitat_id = &development.definition.id;
        let Some(token) = populations.habitat_occupant(habitat_id) else {
            continue;
        };
        let PopulationState::Resident {
            community_id,
            habitat_id,
        } = &token.state
        else {
            continue;
        };
        if communities
            .get(community_id)
            .is_some_and(|community| &community.system == source)
        {
            return Ok((token.id.clone(), community_id.clone(), habitat_id.clone()));
        }
    }
    Err(CoreError::NoResidentPopulation(source.clone()))
}

fn vacate_habitat(bodies: &mut [BodyState], habitat_id: &ContentId) -> Result<(), CoreError> {
    let habitat = bodies
        .iter_mut()
        .flat_map(|body| &mut body.slots)
        .filter_map(|slot| slot.development.as_mut())
        .find(|development| {
            &development.definition.id == habitat_id
                && development.definition.role == DevelopmentRole::Habitat
                && development.definition.condition == DevelopmentCondition::Functional
        })
        .and_then(|development| development.habitat.as_mut())
        .ok_or_else(|| CoreError::UnknownPopulationHabitat(habitat_id.clone()))?;
    habitat.generation_progress = 0;
    habitat.ready_since_tick = None;
    Ok(())
}

fn has_operational_shipyard(system: &SystemState) -> bool {
    system
        .bodies
        .iter()
        .flat_map(|body| &body.slots)
        .any(|slot| {
            slot.development.as_ref().is_some_and(|development| {
                development.definition.role == DevelopmentRole::Shipyard
                    && development.definition.condition == DevelopmentCondition::Functional
                    && development.enabled
                    && development.shipyard.is_some()
            })
        })
}

fn ensure_functional_shipyard(
    bodies: &[BodyState],
    body: &ContentId,
    slot: &ContentId,
) -> Result<(), CoreError> {
    let development = find_slot(bodies, body, slot)?
        .development
        .as_ref()
        .filter(|development| {
            development.definition.role == DevelopmentRole::Shipyard
                && development.definition.condition == DevelopmentCondition::Functional
                && development.enabled
                && development.shipyard.is_some()
        })
        .ok_or_else(|| CoreError::NotFunctionalShipyard {
            body: body.clone(),
            slot: slot.clone(),
        })?;
    debug_assert!(development.shipyard.is_some());
    Ok(())
}

fn functional_shipyard_mut<'a>(
    bodies: &'a mut [BodyState],
    body: &ContentId,
    slot: &ContentId,
) -> Result<&'a mut ShipyardState, CoreError> {
    find_slot_mut(bodies, body, slot)?
        .development
        .as_mut()
        .filter(|development| {
            development.definition.role == DevelopmentRole::Shipyard
                && development.definition.condition == DevelopmentCondition::Functional
                && development.enabled
        })
        .and_then(|development| development.shipyard.as_mut())
        .ok_or_else(|| CoreError::NotFunctionalShipyard {
            body: body.clone(),
            slot: slot.clone(),
        })
}

fn find_ship_project(
    bodies: &[BodyState],
    project_id: &ProjectId,
) -> Option<(usize, usize, usize)> {
    bodies
        .iter()
        .enumerate()
        .flat_map(|(body_index, body)| {
            body.slots
                .iter()
                .enumerate()
                .filter_map(move |(slot_index, slot)| {
                    slot.development
                        .as_ref()
                        .and_then(|development| development.shipyard.as_ref())
                        .map(|shipyard| (body_index, slot_index, shipyard))
                })
        })
        .find_map(|(body_index, slot_index, shipyard)| {
            shipyard
                .queue
                .iter()
                .position(|project| &project.id == project_id)
                .map(|project_index| (body_index, slot_index, project_index))
        })
}

fn completed_asset_index(assets: &[CompletedAsset], ship_id: &ShipId) -> Option<usize> {
    assets.iter().position(|asset| asset.ship_id() == ship_id)
}

fn ensure_asset_ready(
    time: SimulationTime,
    available_at_tick: u64,
    ship_id: &ShipId,
) -> Result<(), CoreError> {
    if time.tick < available_at_tick {
        Err(CoreError::CompletedShipNotReady(ship_id.clone()))
    } else {
        Ok(())
    }
}

fn leg_duration(route: &Route, leg_index: usize, speed: u64) -> Result<u64, CoreError> {
    let speed = NonZeroU64::new(speed)
        .ok_or_else(|| CoreError::InvalidTuning("ship speed must be nonzero".into()))?;
    FixedRate::new(1, speed).checked_ceil(route.legs[leg_index].distance)
}

fn location_position(
    locations: &[LocationDefinition],
    system: &ContentId,
) -> Result<Position3, CoreError> {
    locations
        .iter()
        .find(|location| &location.id == system)
        .map(|location| location.position)
        .ok_or_else(|| CoreError::UnknownSystemLocation(system.clone()))
}

fn expedition_development_id(ship_id: &ShipId, kind: &str) -> Result<ContentId, CoreError> {
    ContentId::new(format!(
        "{}_expedition_{}_{}",
        ship_id.system.as_str(),
        ship_id.sequence,
        kind
    ))
}

fn founded_community_id(system: &ContentId) -> Result<ContentId, CoreError> {
    ContentId::new(format!("{}_community", system.as_str()))
}

fn development_id_exists(
    systems: &BTreeMap<ContentId, SystemState>,
    development_id: &ContentId,
) -> bool {
    systems.values().any(|system| {
        system
            .bodies
            .iter()
            .flat_map(|body| &body.slots)
            .any(|slot| {
                slot.development
                    .as_ref()
                    .is_some_and(|development| &development.definition.id == development_id)
            })
    })
}

fn add_store(destination: &mut ResourceStore, source: &ResourceStore) -> Result<(), CoreError> {
    for (resource, quantity) in &source.quantities {
        add(destination, resource, *quantity)?;
    }
    Ok(())
}
