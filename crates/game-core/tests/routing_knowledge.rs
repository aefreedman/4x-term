use game_core::*;
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

fn id(value: &str) -> ContentId {
    ContentId::new(value).unwrap()
}

fn ship(value: &str, sequence: u64) -> ShipId {
    ShipId::new(id(value), sequence)
}

fn node(value: &str, x: i64, y: i64) -> RouteNode {
    RouteNode {
        system: id(value),
        position: Position3::from_quanta(x, y, 0),
    }
}

fn summary_system(value: &str, x: i64) -> InitialKnowledgeSystem {
    InitialKnowledgeSystem {
        system: id(value),
        position: Position3::from_quanta(x, 0, 0),
        summary: InitialSystemSummary {
            body_count: 1,
            stellar_strength_hundredths: 100,
            body_slot_counts: vec![2],
            resource_richness: BTreeMap::from([(id("core:ore"), ResourceRichness::Normal)]),
        },
    }
}

fn transmission(
    observer: ShipId,
    sequence: u64,
    observed: u64,
    received: u64,
    facts: Vec<ObservedFact>,
) -> PendingTransmission {
    PendingTransmission {
        id: TransmissionId {
            observer: ObserverId::Ship(observer),
            sequence,
        },
        tick_observed: observed,
        tick_received: received,
        facts,
    }
}

fn inhabited_fact(system: &ContentId, inhabited: bool) -> ObservedFact {
    ObservedFact {
        system: system.clone(),
        key: FactKey::Inhabited,
        value: FactValue::Boolean(inhabited),
        detail: FactDetail::Complete,
    }
}

#[test]
fn fixed_point_jump_boundary_and_ceiling_arithmetic_are_exact() {
    let origin = Position3::from_quanta(0, 0, 0);
    let boundary = Position3::from_quanta(3, 4, 0);
    let fractional = Position3::from_quanta(1, 1, 0);

    assert_eq!(origin.checked_ceil_distance(boundary), Ok(5));
    assert_eq!(origin.checked_within_jump(boundary, 5), Ok(true));
    assert_eq!(origin.checked_within_jump(boundary, 4), Ok(false));
    assert_eq!(origin.checked_ceil_distance(fractional), Ok(2));

    let rate = FixedRate::new(1, NonZeroU64::new(500).unwrap());
    assert_eq!(rate.checked_ceil(0), Ok(0));
    assert_eq!(rate.checked_ceil(500), Ok(1));
    assert_eq!(rate.checked_ceil(501), Ok(2));
}

#[test]
fn shortest_route_uses_stable_sequence_tie_break_and_redacts_hidden_stops() {
    let nodes = vec![
        node("core:source", 0, 0),
        node("core:beta", 3, -4),
        node("core:alpha", 3, 4),
        node("core:target", 6, 0),
    ];
    let source = id("core:source");
    let target = id("core:target");
    let route = shortest_route(&nodes, &source, &target, 5)
        .unwrap()
        .unwrap();

    assert_eq!(
        route.systems,
        vec![source.clone(), id("core:alpha"), target.clone()]
    );
    assert_eq!(route.total_distance, 10);
    assert_eq!(route.checked_duration(NonZeroU64::new(3).unwrap()), Ok(4));

    let redacted = route.redact(&BTreeSet::from([source, target]), &BTreeSet::new());
    assert_eq!(redacted[1].system, None);
    let reached = route.redact(&BTreeSet::new(), &BTreeSet::from([id("core:alpha")]));
    assert_eq!(reached[1].system, Some(id("core:alpha")));
    assert!(reached[1].reached);
}

#[test]
fn initial_origin_knowledge_uses_one_leg_summaries_and_three_leg_indications() {
    let systems = vec![
        summary_system("core:origin", 0),
        summary_system("core:one", 10),
        summary_system("core:two", 20),
        summary_system("core:three", 30),
        summary_system("core:four", 40),
    ];
    let state = initial_origin_knowledge(
        &systems,
        &id("core:origin"),
        10,
        ObserverId::InitialOrigin(id("core:origin")),
    )
    .unwrap();

    assert_eq!(
        state.level(&id("core:origin")),
        KnowledgeLevel::IdentifiedSummary
    );
    assert_eq!(
        state.level(&id("core:one")),
        KnowledgeLevel::IdentifiedSummary
    );
    assert_eq!(state.level(&id("core:two")), KnowledgeLevel::Anonymous);
    assert_eq!(state.level(&id("core:three")), KnowledgeLevel::Anonymous);
    assert_eq!(state.level(&id("core:four")), KnowledgeLevel::Unknown);
    assert!(!state.identified_systems().contains(&id("core:two")));

    let one = &state.systems[&id("core:one")];
    assert_eq!(
        one.facts[&FactKey::SystemStrength].value,
        FactValue::Unsigned(100)
    );
    assert!(!one.facts.contains_key(&FactKey::BodyOrder));
    assert_eq!(
        one.facts[&FactKey::SystemStrength].observer,
        ObserverId::InitialOrigin(id("core:origin"))
    );
    assert_ne!(
        one.facts[&FactKey::SystemStrength].observer,
        ObserverId::Ship(ship("core:origin", 0)),
        "synthetic tick-zero identity cannot collide with the first ship"
    );
}

#[test]
fn complete_stop_observation_keeps_exact_map_and_dynamic_fields_separate() {
    let system = id("core:remote");
    let body = id("core:body");
    let ore = id("core:ore");
    let map = SystemMapDefinition {
        location: system.clone(),
        stellar_strength_hundredths: 175,
        bodies: vec![BodyMapDefinition {
            id: body.clone(),
            name: "Remote I".into(),
            eccentricity_hundredths: 125,
            initial_resources: [(ore.clone(), 5)].into_iter().collect(),
            slots: vec![id("core:slot")],
        }],
    };
    let bodies = vec![BodyState {
        id: body.clone(),
        remaining_resources: [(ore.clone(), 3)].into_iter().collect(),
        slots: vec![DevelopmentSlotState {
            id: id("core:slot"),
            development: None,
            reserved_by: None,
        }],
    }];
    let position = Position3::from_quanta(7, -2, 11);
    let complete =
        complete_system_observation(&map, position, &bodies, std::slice::from_ref(&ore), true)
            .unwrap();
    let indication = anonymous_existence_observation(id("core:nearby"));
    let report = PendingTransmission::scheduled_observations(
        TransmissionId {
            observer: ObserverId::Ship(ship("core:probe", 0)),
            sequence: 0,
        },
        4,
        Position3::from_quanta(0, 0, 0),
        Position3::from_quanta(0, 0, 0),
        FixedRate::new(1, NonZeroU64::new(500).unwrap()),
        vec![complete, indication],
    )
    .unwrap();
    let mut state = KnowledgeState::default();
    assert_eq!(state.submit_transmission(4, report), Ok(true));

    let facts = &state.systems[&system].facts;
    assert_eq!(state.level(&system), KnowledgeLevel::Complete);
    assert_eq!(
        facts[&FactKey::Position].value,
        FactValue::Position(position)
    );
    assert_eq!(
        facts[&FactKey::BodyOrder].value,
        FactValue::ContentIds(vec![body.clone()])
    );
    assert_eq!(
        facts[&FactKey::BodyEccentricity { body: body.clone() }].value,
        FactValue::Unsigned(125)
    );
    assert_eq!(
        facts[&FactKey::InitialBodyResource {
            body: body.clone(),
            resource: ore.clone(),
        }]
            .value,
        FactValue::Unsigned(5)
    );
    assert_eq!(
        facts[&FactKey::RemainingBodyResource {
            body,
            resource: ore,
        }]
            .value,
        FactValue::Unsigned(3)
    );
    assert_eq!(state.level(&id("core:nearby")), KnowledgeLevel::Anonymous);
}

#[test]
fn communication_delay_supports_same_tick_and_exact_positive_receipt() {
    let system = id("core:remote");
    let local_system = id("core:local");
    let observer = ship("core:origin", 2);
    let rate = FixedRate::new(1, NonZeroU64::new(500).unwrap());
    let mut state = KnowledgeState::default();

    let immediate = PendingTransmission::scheduled(
        TransmissionId {
            observer: ObserverId::Ship(observer.clone()),
            sequence: 0,
        },
        7,
        Position3::from_quanta(0, 0, 0),
        Position3::from_quanta(0, 0, 0),
        rate,
        vec![inhabited_fact(&local_system, true)],
    )
    .unwrap();
    assert_eq!(immediate.tick_received, 7);
    assert_eq!(state.submit_transmission(7, immediate), Ok(true));
    assert_eq!(state.level(&local_system), KnowledgeLevel::Complete);

    let delayed = PendingTransmission::scheduled(
        TransmissionId {
            observer: ObserverId::Ship(observer),
            sequence: 1,
        },
        7,
        Position3::from_quanta(501, 0, 0),
        Position3::from_quanta(0, 0, 0),
        rate,
        vec![inhabited_fact(&system, false)],
    )
    .unwrap();
    assert_eq!(delayed.tick_received, 9);
    assert_eq!(state.submit_transmission(7, delayed), Ok(true));
    assert_eq!(state.receive_due(8), Ok(0));
    assert_eq!(state.receive_due(9), Ok(1));
    assert_eq!(
        state.systems[&system].facts[&FactKey::Inhabited].value,
        FactValue::Boolean(false)
    );
}

#[test]
fn dynamic_fact_merge_is_fresh_monotonic_and_receipt_order_independent() {
    let system = id("core:remote");
    let lower_observer = ship("core:alpha", 5);
    let higher_observer = ship("core:beta", 1);
    let reports = [
        transmission(
            higher_observer,
            0,
            10,
            20,
            vec![inhabited_fact(&system, false)],
        ),
        transmission(
            lower_observer,
            0,
            10,
            21,
            vec![inhabited_fact(&system, true)],
        ),
        transmission(
            ship("core:stale", 0),
            0,
            9,
            30,
            vec![inhabited_fact(&system, false)],
        ),
    ];

    for order in [[0, 1, 2], [2, 1, 0], [1, 0, 2]] {
        let mut state = KnowledgeState::default();
        for index in order {
            state.receive_transmission(reports[index].clone()).unwrap();
        }
        let fact = &state.systems[&system].facts[&FactKey::Inhabited];
        assert_eq!(fact.value, FactValue::Boolean(true));
        assert_eq!(fact.tick_observed, 10);
        assert_eq!(fact.observer, ObserverId::Ship(ship("core:alpha", 5)));
    }
}

#[test]
fn immutable_contradiction_rejects_whole_transmission_and_duplicate_is_idempotent() {
    let system = id("core:remote");
    let observer = ship("core:probe", 0);
    let strength = |value| ObservedFact {
        system: system.clone(),
        key: FactKey::SystemStrength,
        value: FactValue::Unsigned(value),
        detail: FactDetail::Complete,
    };
    let position = |x| ObservedFact {
        system: system.clone(),
        key: FactKey::Position,
        value: FactValue::Position(Position3::from_quanta(x, 2, 3)),
        detail: FactDetail::Complete,
    };
    let mut state = KnowledgeState::default();
    let first = transmission(observer.clone(), 0, 3, 4, vec![strength(100), position(1)]);
    assert_eq!(state.receive_transmission(first.clone()), Ok(true));
    let after_first = state.clone();
    assert_eq!(state.receive_transmission(first), Ok(false));
    assert_eq!(state, after_first);

    let contradictory = transmission(
        observer,
        1,
        5,
        6,
        vec![inhabited_fact(&system, true), position(9)],
    );
    assert!(matches!(
        state.receive_transmission(contradictory),
        Err(KnowledgeError::ImmutableContradiction { .. })
    ));
    assert!(
        !state.systems[&system]
            .facts
            .contains_key(&FactKey::Inhabited)
    );
    assert_eq!(
        state.systems[&system].facts[&FactKey::SystemStrength].value,
        FactValue::Unsigned(100)
    );
    assert_eq!(
        state.systems[&system].facts[&FactKey::Position].value,
        FactValue::Position(Position3::from_quanta(1, 2, 3))
    );
}
