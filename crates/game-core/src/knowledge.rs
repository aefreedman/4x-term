use crate::{
    BodyState, ContentId, CoreError, FixedRate, MissionState, ObserverId, Position3, RouteNode,
    ShipId, SystemMapDefinition, TransmissionId,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum KnowledgeLevel {
    Unknown,
    Anonymous,
    IdentifiedSummary,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FactDetail {
    Anonymous,
    IdentifiedSummary,
    Complete,
}

impl From<FactDetail> for KnowledgeLevel {
    fn from(value: FactDetail) -> Self {
        match value {
            FactDetail::Anonymous => Self::Anonymous,
            FactDetail::IdentifiedSummary => Self::IdentifiedSummary,
            FactDetail::Complete => Self::Complete,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceRichness {
    Poor,
    Normal,
    Rich,
}

/// Stable field keys keep unrelated facts from replacing one another.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FactKey {
    Existence,
    Position,
    BodyCount,
    SystemStrength,
    BodySlotCount {
        body_index: u64,
    },
    ResourceRichness {
        resource: ContentId,
    },
    BodyOrder,
    BodyName {
        body: ContentId,
    },
    BodyEccentricity {
        body: ContentId,
    },
    SlotOrder {
        body: ContentId,
    },
    InitialBodyResource {
        body: ContentId,
        resource: ContentId,
    },
    RemainingBodyResource {
        body: ContentId,
        resource: ContentId,
    },
    Inhabited,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FactValue {
    Present,
    Position(Position3),
    Unsigned(u64),
    Boolean(bool),
    Text(String),
    ContentIds(Vec<ContentId>),
    Richness(ResourceRichness),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedFact {
    pub system: ContentId,
    pub key: FactKey,
    pub value: FactValue,
    pub detail: FactDetail,
}

/// One system observation containing independently keyed facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub system: ContentId,
    pub facts: BTreeMap<FactKey, (FactValue, FactDetail)>,
}

impl Observation {
    pub fn into_facts(self) -> Vec<ObservedFact> {
        self.facts
            .into_iter()
            .map(|(key, (value, detail))| ObservedFact {
                system: self.system.clone(),
                key,
                value,
                detail,
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeFact {
    pub value: FactValue,
    pub detail: FactDetail,
    pub tick_observed: u64,
    pub observer: ObserverId,
    pub tick_received: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemKnowledge {
    pub system: ContentId,
    pub level: KnowledgeLevel,
    pub facts: BTreeMap<FactKey, KnowledgeFact>,
}

impl SystemKnowledge {
    fn unknown(system: ContentId) -> Self {
        Self {
            system,
            level: KnowledgeLevel::Unknown,
            facts: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingTransmission {
    pub id: TransmissionId,
    pub tick_observed: u64,
    pub tick_received: u64,
    pub facts: Vec<ObservedFact>,
}

impl PendingTransmission {
    /// Schedules receipt from direct observed-system distance, never route distance.
    pub fn scheduled(
        id: TransmissionId,
        tick_observed: u64,
        observed_position: Position3,
        origin_position: Position3,
        communication_rate: FixedRate,
        facts: Vec<ObservedFact>,
    ) -> Result<Self, KnowledgeError> {
        let distance = observed_position.checked_ceil_distance(origin_position)?;
        let delay = communication_rate.checked_ceil(distance)?;
        let tick_received = tick_observed
            .checked_add(delay)
            .ok_or(CoreError::Overflow)?;
        Ok(Self {
            id,
            tick_observed,
            tick_received,
            facts,
        })
    }

    pub fn scheduled_observations(
        id: TransmissionId,
        tick_observed: u64,
        observed_position: Position3,
        origin_position: Position3,
        communication_rate: FixedRate,
        observations: Vec<Observation>,
    ) -> Result<Self, KnowledgeError> {
        let facts = observations
            .into_iter()
            .flat_map(Observation::into_facts)
            .collect();
        Self::scheduled(
            id,
            tick_observed,
            observed_position,
            origin_position,
            communication_rate,
            facts,
        )
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct KnowledgeState {
    pub systems: BTreeMap<ContentId, SystemKnowledge>,
    pub pending_transmissions: BTreeMap<TransmissionId, PendingTransmission>,
    /// Receipt IDs are deduplication state, not retained report contents.
    pub received_transmissions: BTreeSet<TransmissionId>,
    /// Player-facing expedition state. Physical arrival cannot resolve this map.
    pub mission_states: BTreeMap<ShipId, MissionState>,
    /// Typed outcomes travel with final transmissions but remain hidden until receipt.
    pub(crate) pending_mission_outcomes: BTreeMap<TransmissionId, MissionState>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum KnowledgeError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("duplicate system in knowledge input: {0}")]
    DuplicateSystem(ContentId),
    #[error("unknown origin system: {0}")]
    UnknownOrigin(ContentId),
    #[error("duplicate pending transmission")]
    DuplicateTransmission,
    #[error("transmission receipt precedes observation")]
    ReceiptBeforeObservation,
    #[error("transmission is already overdue when submitted")]
    OverdueTransmission,
    #[error("fact has the wrong value or detail for key {0:?}")]
    InvalidFact(FactKey),
    #[error("immutable fact contradicts known value for {system}, key {key:?}")]
    ImmutableContradiction { system: ContentId, key: FactKey },
    #[error("equally ranked facts disagree for {system}, key {key:?}")]
    AmbiguousFact { system: ContentId, key: FactKey },
    #[error("mission already exists for ship: {0:?}")]
    DuplicateMission(ShipId),
    #[error("mission is not awaiting this outcome: {0:?}")]
    MissionNotAwaiting(ShipId),
    #[error("invalid mission outcome transmission: {0:?}")]
    InvalidMissionOutcome(ShipId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitialKnowledgeSystem {
    pub system: ContentId,
    pub position: Position3,
    pub summary: InitialSystemSummary,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InitialSystemSummary {
    pub body_count: u64,
    pub stellar_strength_hundredths: u16,
    /// Slot counts are in stable body order; body identities remain unrevealed.
    pub body_slot_counts: Vec<u64>,
    pub resource_richness: BTreeMap<ContentId, ResourceRichness>,
}

impl KnowledgeState {
    #[must_use]
    pub fn level(&self, system: &ContentId) -> KnowledgeLevel {
        self.systems
            .get(system)
            .map_or(KnowledgeLevel::Unknown, |knowledge| knowledge.level)
    }

    #[must_use]
    pub fn identified_systems(&self) -> BTreeSet<ContentId> {
        self.systems
            .iter()
            .filter(|(_, knowledge)| knowledge.level >= KnowledgeLevel::IdentifiedSummary)
            .map(|(system, _)| system.clone())
            .collect()
    }

    #[must_use]
    pub fn mission_state(&self, ship_id: &ShipId) -> Option<&MissionState> {
        self.mission_states.get(ship_id)
    }

    /// Registers the origin-facing state at expedition launch. The state remains
    /// awaiting even after physical arrival until the final report is received.
    pub fn register_mission(
        &mut self,
        ship_id: ShipId,
        target: ContentId,
    ) -> Result<(), KnowledgeError> {
        if self.mission_states.contains_key(&ship_id) {
            return Err(KnowledgeError::DuplicateMission(ship_id));
        }
        self.mission_states
            .insert(ship_id, MissionState::AwaitingOutcome { target });
        Ok(())
    }

    /// Submits the final expedition observation and its hidden typed outcome as
    /// one atomic transmission. A zero-delay report resolves immediately.
    pub fn submit_mission_transmission(
        &mut self,
        current_tick: u64,
        transmission: PendingTransmission,
        outcome: MissionState,
    ) -> Result<bool, KnowledgeError> {
        let mut candidate = self.clone();
        candidate.validate_mission_outcome(&transmission, &outcome)?;
        if candidate
            .pending_mission_outcomes
            .insert(transmission.id.clone(), outcome)
            .is_some()
        {
            return Err(KnowledgeError::DuplicateTransmission);
        }
        let submitted = candidate.submit_transmission(current_tick, transmission)?;
        *self = candidate;
        Ok(submitted)
    }

    /// Submits a report during a movement phase. Zero-delay reports merge immediately.
    pub fn submit_transmission(
        &mut self,
        current_tick: u64,
        transmission: PendingTransmission,
    ) -> Result<bool, KnowledgeError> {
        if transmission.tick_received < transmission.tick_observed {
            return Err(KnowledgeError::ReceiptBeforeObservation);
        }
        if self.received_transmissions.contains(&transmission.id) {
            return Ok(false);
        }
        if transmission.tick_received < current_tick {
            return Err(KnowledgeError::OverdueTransmission);
        }
        if transmission.tick_received == current_tick {
            return self.receive_transmission(transmission);
        }
        if self.pending_transmissions.contains_key(&transmission.id) {
            return Err(KnowledgeError::DuplicateTransmission);
        }
        self.pending_transmissions
            .insert(transmission.id.clone(), transmission);
        Ok(true)
    }

    /// Receives every report due by this tick. The due batch commits only if every report is valid.
    pub fn receive_due(&mut self, current_tick: u64) -> Result<usize, KnowledgeError> {
        let due = self
            .pending_transmissions
            .values()
            .filter(|transmission| transmission.tick_received <= current_tick)
            .cloned()
            .collect::<Vec<_>>();
        let mut candidate = self.clone();
        let mut received = 0;
        for transmission in due {
            if candidate.receive_transmission(transmission)? {
                received += 1;
            }
        }
        *self = candidate;
        Ok(received)
    }

    /// Validates and merges one complete transmission atomically. Duplicate receipt is a no-op.
    pub fn receive_transmission(
        &mut self,
        transmission: PendingTransmission,
    ) -> Result<bool, KnowledgeError> {
        if transmission.tick_received < transmission.tick_observed {
            return Err(KnowledgeError::ReceiptBeforeObservation);
        }
        if self.received_transmissions.contains(&transmission.id) {
            return Ok(false);
        }

        let mut candidate = self.clone();
        for fact in &transmission.facts {
            validate_fact(fact)?;
            candidate.merge_fact(
                fact,
                transmission.tick_observed,
                transmission.tick_received,
                &transmission.id.observer,
            )?;
        }
        candidate.pending_transmissions.remove(&transmission.id);
        let outcome = candidate.pending_mission_outcomes.remove(&transmission.id);
        candidate
            .received_transmissions
            .insert(transmission.id.clone());
        if let Some(outcome) = outcome {
            candidate.apply_mission_outcome(outcome)?;
        }
        *self = candidate;
        Ok(true)
    }

    fn validate_mission_outcome(
        &self,
        transmission: &PendingTransmission,
        outcome: &MissionState,
    ) -> Result<(), KnowledgeError> {
        let Some(ship_id) = outcome.resolved_ship_id() else {
            return Err(KnowledgeError::InvalidMissionOutcome(ShipId::new(
                outcome.target().clone(),
                0,
            )));
        };
        if transmission.id.observer.ship_id() != Some(ship_id) {
            return Err(KnowledgeError::InvalidMissionOutcome(ship_id.clone()));
        }
        let Some(MissionState::AwaitingOutcome { target }) = self.mission_states.get(ship_id)
        else {
            return Err(KnowledgeError::MissionNotAwaiting(ship_id.clone()));
        };
        if target != outcome.target()
            || !transmission.facts.iter().any(|fact| {
                &fact.system == target
                    && fact.key == FactKey::Inhabited
                    && fact.detail == FactDetail::Complete
                    && matches!(fact.value, FactValue::Boolean(_))
            })
        {
            return Err(KnowledgeError::InvalidMissionOutcome(ship_id.clone()));
        }
        Ok(())
    }

    fn apply_mission_outcome(&mut self, outcome: MissionState) -> Result<(), KnowledgeError> {
        let ship_id = outcome.resolved_ship_id().cloned().ok_or_else(|| {
            KnowledgeError::InvalidMissionOutcome(ShipId::new(outcome.target().clone(), 0))
        })?;
        let Some(MissionState::AwaitingOutcome { target }) = self.mission_states.get(&ship_id)
        else {
            return Err(KnowledgeError::MissionNotAwaiting(ship_id));
        };
        if target != outcome.target() {
            return Err(KnowledgeError::InvalidMissionOutcome(ship_id));
        }
        self.mission_states.insert(ship_id, outcome);
        Ok(())
    }

    fn merge_fact(
        &mut self,
        observed: &ObservedFact,
        tick_observed: u64,
        tick_received: u64,
        observer: &ObserverId,
    ) -> Result<(), KnowledgeError> {
        let system = self
            .systems
            .entry(observed.system.clone())
            .or_insert_with(|| SystemKnowledge::unknown(observed.system.clone()));
        let incoming = KnowledgeFact {
            value: observed.value.clone(),
            detail: observed.detail,
            tick_observed,
            observer: observer.clone(),
            tick_received,
        };

        if let Some(current) = system.facts.get(&observed.key) {
            if is_immutable(&observed.key) && current.value != incoming.value {
                return Err(KnowledgeError::ImmutableContradiction {
                    system: observed.system.clone(),
                    key: observed.key.clone(),
                });
            }
            match compare_facts(&incoming, current) {
                std::cmp::Ordering::Greater => {
                    system.facts.insert(observed.key.clone(), incoming);
                }
                std::cmp::Ordering::Equal if current.value != incoming.value => {
                    return Err(KnowledgeError::AmbiguousFact {
                        system: observed.system.clone(),
                        key: observed.key.clone(),
                    });
                }
                _ => {}
            }
        } else {
            system.facts.insert(observed.key.clone(), incoming);
        }
        system.level = system.level.max(observed.detail.into());
        Ok(())
    }
}

#[must_use]
pub fn anonymous_existence_observation(system: ContentId) -> Observation {
    Observation {
        system,
        facts: BTreeMap::from([(
            FactKey::Existence,
            (FactValue::Present, FactDetail::Anonymous),
        )]),
    }
}

/// Captures the approved exact map and scouting-visible runtime fields at a stop.
pub fn complete_system_observation(
    map: &SystemMapDefinition,
    position: Position3,
    bodies: &[BodyState],
    material_resources: &[ContentId],
    inhabited: bool,
) -> Result<Observation, CoreError> {
    if map.bodies.len() != bodies.len() {
        return Err(CoreError::MapRuntimeMismatch(map.location.clone()));
    }
    let mut facts = BTreeMap::new();
    facts.insert(
        FactKey::Existence,
        (FactValue::Present, FactDetail::Complete),
    );
    facts.insert(
        FactKey::Position,
        (FactValue::Position(position), FactDetail::Complete),
    );
    facts.insert(
        FactKey::SystemStrength,
        (
            FactValue::Unsigned(u64::from(map.stellar_strength_hundredths)),
            FactDetail::Complete,
        ),
    );
    facts.insert(
        FactKey::BodyOrder,
        (
            FactValue::ContentIds(map.bodies.iter().map(|body| body.id.clone()).collect()),
            FactDetail::Complete,
        ),
    );
    facts.insert(
        FactKey::Inhabited,
        (FactValue::Boolean(inhabited), FactDetail::Complete),
    );
    for (body_map, body_state) in map.bodies.iter().zip(bodies) {
        if body_map.id != body_state.id
            || body_map.slots.len() != body_state.slots.len()
            || !body_map
                .slots
                .iter()
                .zip(&body_state.slots)
                .all(|(slot_map, slot_state)| slot_map == &slot_state.id)
        {
            return Err(CoreError::MapRuntimeMismatch(map.location.clone()));
        }
        facts.insert(
            FactKey::BodyName {
                body: body_map.id.clone(),
            },
            (FactValue::Text(body_map.name.clone()), FactDetail::Complete),
        );
        facts.insert(
            FactKey::BodyEccentricity {
                body: body_map.id.clone(),
            },
            (
                FactValue::Unsigned(u64::from(body_map.eccentricity_hundredths)),
                FactDetail::Complete,
            ),
        );
        facts.insert(
            FactKey::SlotOrder {
                body: body_map.id.clone(),
            },
            (
                FactValue::ContentIds(body_map.slots.clone()),
                FactDetail::Complete,
            ),
        );
        for resource in material_resources {
            facts.insert(
                FactKey::InitialBodyResource {
                    body: body_map.id.clone(),
                    resource: resource.clone(),
                },
                (
                    FactValue::Unsigned(body_map.initial_resources.quantity(resource)),
                    FactDetail::Complete,
                ),
            );
            facts.insert(
                FactKey::RemainingBodyResource {
                    body: body_map.id.clone(),
                    resource: resource.clone(),
                },
                (
                    FactValue::Unsigned(body_state.remaining_resources.quantity(resource)),
                    FactDetail::Complete,
                ),
            );
        }
    }
    Ok(Observation {
        system: map.location.clone(),
        facts,
    })
}

/// Creates tick-zero origin knowledge from one-, two-, and three-leg geometric reachability.
pub fn initial_origin_knowledge(
    systems: &[InitialKnowledgeSystem],
    origin: &ContentId,
    probe_maximum_jump: u64,
    observer: ObserverId,
) -> Result<KnowledgeState, KnowledgeError> {
    let mut by_id = BTreeMap::new();
    for system in systems {
        if by_id.insert(system.system.clone(), system).is_some() {
            return Err(KnowledgeError::DuplicateSystem(system.system.clone()));
        }
    }
    if !by_id.contains_key(origin) {
        return Err(KnowledgeError::UnknownOrigin(origin.clone()));
    }

    let mut depth = BTreeMap::from([(origin.clone(), 0_u8)]);
    let mut queue = VecDeque::from([origin.clone()]);
    while let Some(current) = queue.pop_front() {
        let current_depth = depth[&current];
        if current_depth == 3 {
            continue;
        }
        let current_position = by_id[&current].position;
        for (next, next_system) in &by_id {
            if depth.contains_key(next)
                || !current_position
                    .checked_within_jump(next_system.position, probe_maximum_jump)?
            {
                continue;
            }
            depth.insert(next.clone(), current_depth + 1);
            queue.push_back(next.clone());
        }
    }

    let mut state = KnowledgeState::default();
    for system in systems {
        state.systems.insert(
            system.system.clone(),
            SystemKnowledge::unknown(system.system.clone()),
        );
    }
    for (system_id, hops) in depth {
        let facts = if hops <= 1 {
            summary_facts(by_id[&system_id])?
        } else {
            vec![ObservedFact {
                system: system_id.clone(),
                key: FactKey::Existence,
                value: FactValue::Present,
                detail: FactDetail::Anonymous,
            }]
        };
        // Tick-zero generated observations merge directly and do not consume runtime IDs.
        for fact in &facts {
            validate_fact(fact)?;
            state.merge_fact(fact, 0, 0, &observer)?;
        }
    }
    Ok(state)
}

fn summary_facts(system: &InitialKnowledgeSystem) -> Result<Vec<ObservedFact>, KnowledgeError> {
    if u64::try_from(system.summary.body_slot_counts.len()).map_err(|_| CoreError::Overflow)?
        != system.summary.body_count
    {
        return Err(KnowledgeError::InvalidFact(FactKey::BodyCount));
    }
    let mut facts = vec![
        ObservedFact {
            system: system.system.clone(),
            key: FactKey::Existence,
            value: FactValue::Present,
            detail: FactDetail::IdentifiedSummary,
        },
        ObservedFact {
            system: system.system.clone(),
            key: FactKey::BodyCount,
            value: FactValue::Unsigned(system.summary.body_count),
            detail: FactDetail::IdentifiedSummary,
        },
        ObservedFact {
            system: system.system.clone(),
            key: FactKey::SystemStrength,
            value: FactValue::Unsigned(u64::from(system.summary.stellar_strength_hundredths)),
            detail: FactDetail::IdentifiedSummary,
        },
    ];
    for (body_index, slot_count) in system.summary.body_slot_counts.iter().enumerate() {
        facts.push(ObservedFact {
            system: system.system.clone(),
            key: FactKey::BodySlotCount {
                body_index: u64::try_from(body_index).map_err(|_| CoreError::Overflow)?,
            },
            value: FactValue::Unsigned(*slot_count),
            detail: FactDetail::IdentifiedSummary,
        });
    }
    for (resource, richness) in &system.summary.resource_richness {
        facts.push(ObservedFact {
            system: system.system.clone(),
            key: FactKey::ResourceRichness {
                resource: resource.clone(),
            },
            value: FactValue::Richness(*richness),
            detail: FactDetail::IdentifiedSummary,
        });
    }
    Ok(facts)
}

fn compare_facts(left: &KnowledgeFact, right: &KnowledgeFact) -> std::cmp::Ordering {
    left.detail
        .cmp(&right.detail)
        .then_with(|| left.tick_observed.cmp(&right.tick_observed))
        // Lower stable observer ID wins the final tie.
        .then_with(|| right.observer.cmp(&left.observer))
}

fn is_immutable(key: &FactKey) -> bool {
    !matches!(
        key,
        FactKey::RemainingBodyResource { .. } | FactKey::Inhabited
    )
}

fn validate_fact(fact: &ObservedFact) -> Result<(), KnowledgeError> {
    let valid = matches!(
        (&fact.key, &fact.value, fact.detail),
        (FactKey::Existence, FactValue::Present, _)
            | (
                FactKey::Position,
                FactValue::Position(_),
                FactDetail::Complete,
            )
            | (
                FactKey::BodyCount | FactKey::SystemStrength | FactKey::BodySlotCount { .. },
                FactValue::Unsigned(_),
                FactDetail::IdentifiedSummary | FactDetail::Complete,
            )
            | (
                FactKey::ResourceRichness { .. },
                FactValue::Richness(_),
                FactDetail::IdentifiedSummary | FactDetail::Complete,
            )
            | (
                FactKey::BodyOrder | FactKey::SlotOrder { .. },
                FactValue::ContentIds(_),
                FactDetail::Complete,
            )
            | (
                FactKey::BodyName { .. },
                FactValue::Text(_),
                FactDetail::Complete,
            )
            | (
                FactKey::BodyEccentricity { .. }
                    | FactKey::InitialBodyResource { .. }
                    | FactKey::RemainingBodyResource { .. },
                FactValue::Unsigned(_),
                FactDetail::Complete,
            )
            | (
                FactKey::Inhabited,
                FactValue::Boolean(_),
                FactDetail::Complete
            )
    );
    if valid {
        Ok(())
    } else {
        Err(KnowledgeError::InvalidFact(fact.key.clone()))
    }
}

/// Converts map nodes for callers that already own routing positions.
#[must_use]
pub fn knowledge_route_nodes(systems: &[InitialKnowledgeSystem]) -> Vec<RouteNode> {
    systems
        .iter()
        .map(|system| RouteNode {
            system: system.system.clone(),
            position: system.position,
        })
        .collect()
}
