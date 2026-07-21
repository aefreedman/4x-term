use game_core::*;
use std::collections::BTreeMap;
use std::num::NonZeroU64;

fn id(value: &str) -> ContentId {
    ContentId::new(value).unwrap()
}

fn store(values: &[(&str, u64)]) -> ResourceStore {
    values
        .iter()
        .map(|(resource, quantity)| (id(resource), *quantity))
        .collect()
}

fn recipe(values: &[(&str, u64)]) -> ConstructionRecipe {
    ConstructionRecipe {
        cost: store(values),
        required_work: 1,
    }
}

fn tuning() -> WorldTuning {
    WorldTuning {
        energy_resource: id("core:energy"),
        ore_resource: id("core:ore"),
        alloy_resource: id("core:alloy"),
        seasonal_shape: [1; 10],
        seasonal_baseline_average: 1,
        life_support_per_population: 1,
        origin_construction_work: 1,
        intrinsic_energy_capacity: 1_000,
        battery_energy_capacity: 100,
        habitat_population_energy: 500,
        coordinate_quanta_per_map_unit: 1,
        collector_recipe: recipe(&[("core:energy", 2), ("core:alloy", 1)]),
        battery_recipe: recipe(&[("core:energy", 1)]),
        extractor_recipe: recipe(&[("core:energy", 1)]),
        refinery_recipe: recipe(&[("core:energy", 1)]),
        habitat_recipe: recipe(&[("core:energy", 1)]),
        shipyard_recipe: recipe(&[("core:energy", 1)]),
        extractor: ExtractorParameters {
            energy_upkeep: 1,
            cycle_duration: 1,
            output: 1,
        },
        refinery: RefineryParameters {
            energy_upkeep: 1,
            cycle_duration: 1,
            input: 1,
            output: 1,
        },
        probe_project: ProbeProjectTuning {
            material_commitment: store(&[("core:energy", 3), ("core:alloy", 1)]),
            duration_ticks: 1,
            energy_per_progress_tick: 2,
        },
        expedition_project: ExpeditionProjectTuning {
            hull_material_commitment: store(&[("core:energy", 4), ("core:alloy", 2)]),
            founding_stocks: store(&[("core:energy", 5), ("core:ore", 3)]),
            duration_ticks: 1,
            energy_per_progress_tick: 2,
        },
        probe_travel: ShipTravelTuning {
            maximum_jump_quanta: 10,
            speed_quanta_per_tick: 4,
            energy_per_quantum: FixedRate::new(1, NonZeroU64::new(4).unwrap()),
        },
        expedition_travel: ShipTravelTuning {
            maximum_jump_quanta: 10,
            speed_quanta_per_tick: 4,
            energy_per_quantum: FixedRate::new(1, NonZeroU64::new(4).unwrap()),
        },
        probe_reveal_radius_quanta: 10,
        communication_delay_per_quantum: FixedRate::new(1, NonZeroU64::new(2).unwrap()),
        resource_richness: BTreeMap::from([(
            id("core:ore"),
            RichnessThresholds {
                poor_minimum: 1,
                poor_maximum: 1,
                normal_minimum: 2,
                normal_maximum: 2,
                rich_minimum: 3,
            },
        )]),
    }
}

fn development(value: &str, role: DevelopmentRole) -> DevelopmentDefinition {
    DevelopmentDefinition {
        id: id(value),
        role,
        condition: DevelopmentCondition::Functional,
        extractor_target: None,
    }
}

fn slot(value: &str, development: Option<DevelopmentDefinition>) -> DevelopmentSlotDefinition {
    DevelopmentSlotDefinition {
        id: id(value),
        development,
    }
}

fn two_system_fixture(
    source_energy: u64,
    target_energy: u64,
    population_count: u64,
    target_slots: usize,
) -> WorldState {
    let origin = id("core:origin");
    let target = id("core:target");
    let community = id("core:origin_community");
    let origin_slots = vec![
        slot(
            "core:origin_collector_slot",
            Some(development(
                "core:origin_collector",
                DevelopmentRole::Collector,
            )),
        ),
        slot(
            "core:origin_habitat_slot_0",
            Some(development(
                "core:origin_habitat_0",
                DevelopmentRole::Habitat,
            )),
        ),
        slot(
            "core:origin_habitat_slot_1",
            Some(development(
                "core:origin_habitat_1",
                DevelopmentRole::Habitat,
            )),
        ),
        slot(
            "core:origin_yard_slot_0",
            Some(development("core:origin_yard_0", DevelopmentRole::Shipyard)),
        ),
        slot(
            "core:origin_yard_slot_1",
            Some(development("core:origin_yard_1", DevelopmentRole::Shipyard)),
        ),
    ];
    let target_slots = (0..target_slots)
        .map(|index| slot(&format!("core:target_slot_{index}"), None))
        .collect();
    let mut fixture_tuning = tuning();
    if population_count != 0 {
        fixture_tuning.habitat_population_energy = 1;
    }
    let initial_source_energy = if population_count == 0 {
        source_energy
    } else {
        source_energy - 56 + population_count * 2
    };

    let mut world = WorldState::new(WorldDefinition {
        resources: vec![
            ResourceDefinition {
                id: id("core:energy"),
                name: "Energy".into(),
                naturally_deposit_bearing: false,
            },
            ResourceDefinition {
                id: id("core:ore"),
                name: "Ore".into(),
                naturally_deposit_bearing: true,
            },
            ResourceDefinition {
                id: id("core:alloy"),
                name: "Alloy".into(),
                naturally_deposit_bearing: false,
            },
        ],
        locations: vec![
            LocationDefinition {
                id: origin.clone(),
                name: "Origin".into(),
                position: Position3::from_quanta(0, 0, 0),
            },
            LocationDefinition {
                id: target.clone(),
                name: "Target".into(),
                position: Position3::from_quanta(4, 0, 0),
            },
        ],
        origin_system: origin.clone(),
        origin_community: community.clone(),
        communities: vec![CommunityDefinition {
            id: community,
            system: origin.clone(),
        }],
        population_tokens: Vec::new(),
        systems: vec![
            SystemDefinition {
                location: origin,
                stellar_strength_hundredths: 100,
                bodies: vec![BodyDefinition {
                    id: id("core:origin_body"),
                    name: "Origin Body".into(),
                    eccentricity_hundredths: 0,
                    initial_resources: ResourceStore::new(),
                    slots: origin_slots,
                }],
                stocks: store(&[
                    ("core:energy", initial_source_energy),
                    ("core:ore", 100),
                    ("core:alloy", 100),
                ]),
                player_founded: true,
                command_unlock_received: true,
            },
            SystemDefinition {
                location: target,
                stellar_strength_hundredths: 100,
                bodies: vec![BodyDefinition {
                    id: id("core:target_body"),
                    name: "Target Body".into(),
                    eccentricity_hundredths: 0,
                    initial_resources: ResourceStore::new(),
                    slots: target_slots,
                }],
                stocks: store(&[("core:energy", target_energy)]),
                player_founded: false,
                command_unlock_received: false,
            },
        ],
        sites: Vec::new(),
        tuning: fixture_tuning,
    })
    .unwrap();
    if population_count == 1 {
        world
            .set_habitat_generation_enabled(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:origin_habitat_slot_1"),
                false,
            )
            .unwrap();
    }
    if population_count != 0 {
        world.advance_tick().unwrap();
        world.advance_tick().unwrap();
        assert_eq!(
            world.debug_snapshot().populations.tokens.len(),
            population_count as usize
        );
        assert_eq!(
            world
                .debug_system_snapshot(&id("core:origin"))
                .unwrap()
                .stocks
                .quantity(&id("core:energy")),
            source_energy
        );
    }
    world
}

fn enqueue_expedition(world: &mut WorldState, yard_slot: &str) -> ShipProjectIds {
    world
        .enqueue_ship_project(
            &id("core:origin"),
            &id("core:origin_body"),
            &id(yard_slot),
            ShipProjectKind::Expedition,
        )
        .unwrap()
}

fn system<'a>(snapshot: &'a WorldSnapshot, system_id: &str) -> &'a SystemSnapshot {
    snapshot
        .systems
        .iter()
        .find(|system| system.location == id(system_id))
        .unwrap()
}

fn global_stock(snapshot: &WorldSnapshot, resource: &ContentId) -> u64 {
    snapshot
        .systems
        .iter()
        .map(|system| system.stocks.quantity(resource))
        .sum()
}

fn global_accounting(
    snapshot: &WorldSnapshot,
    resource: &ContentId,
    field: impl Fn(&ResourceAccounting) -> &ResourceStore,
) -> u64 {
    snapshot
        .systems
        .iter()
        .map(|system| field(&system.accounting).quantity(resource))
        .sum()
}

#[test]
fn two_system_tick_orders_production_arrival_observation_receipt_and_retention_exactly() {
    let mut world = two_system_fixture(100, 998, 1, 2);
    world
        .set_habitat_generation_enabled(
            &id("core:origin"),
            &id("core:origin_body"),
            &id("core:origin_habitat_slot_1"),
            false,
        )
        .unwrap();
    let expedition = enqueue_expedition(&mut world, "core:origin_yard_slot_0");

    world.advance_tick().unwrap();
    let completed = world.debug_snapshot();
    assert_eq!(completed.time.tick, 3);
    assert_eq!(
        system(&completed, "core:origin")
            .accounting
            .produced
            .quantity(&id("core:energy")),
        84
    );
    assert_eq!(system(&completed, "core:origin").completed_assets.len(), 1);

    let route = world
        .launch_expedition(
            &id("core:origin"),
            &expedition.ship_id,
            &id("core:target"),
            None,
        )
        .unwrap();
    assert_eq!(route.total_distance, 4);
    assert_eq!(world.debug_snapshot().transit[0].remaining_leg_ticks, 1);

    world.advance_tick().unwrap();
    let arrived = world.debug_snapshot();
    assert_eq!(arrived.time.tick, 4);
    assert!(arrived.transit.is_empty());
    let target = system(&arrived, "core:target");
    assert_eq!(target.accounting.produced.quantity(&id("core:energy")), 0);
    assert_eq!(target.stocks.quantity(&id("core:energy")), 1_000);
    assert_eq!(target.energy_overflow.last_tick_retention, 3);
    assert!(matches!(
        arrived.knowledge.mission_state(&expedition.ship_id),
        Some(MissionState::AwaitingOutcome { .. })
    ));
    assert_eq!(
        arrived.knowledge.level(&id("core:target")),
        KnowledgeLevel::IdentifiedSummary
    );
    let player_before_receipt = world.player_view().unwrap();
    let target_before_receipt = player_before_receipt
        .systems
        .iter()
        .find(|system| system.system == id("core:target"))
        .unwrap();
    assert!(target_before_receipt.local_state.is_none());
    assert!(matches!(
        player_before_receipt.missions.get(&expedition.ship_id),
        Some(MissionState::AwaitingOutcome { .. })
    ));
    let report = arrived
        .knowledge
        .pending_transmissions
        .values()
        .find(|report| report.id.observer == ObserverId::Ship(expedition.ship_id.clone()))
        .unwrap();
    assert_eq!((report.tick_observed, report.tick_received), (3, 5));
    assert!(report.facts.iter().any(|fact| {
        fact.system == id("core:target")
            && fact.key == FactKey::Inhabited
            && fact.value == FactValue::Boolean(true)
    }));

    world.advance_tick().unwrap();
    let operating_before_receipt = world.debug_snapshot();
    assert_eq!(operating_before_receipt.time.tick, 5);
    assert_eq!(
        system(&operating_before_receipt, "core:target")
            .accounting
            .produced
            .quantity(&id("core:energy")),
        28
    );
    assert!(matches!(
        operating_before_receipt
            .knowledge
            .mission_state(&expedition.ship_id),
        Some(MissionState::AwaitingOutcome { .. })
    ));

    world.advance_tick().unwrap();
    let received = world.debug_snapshot();
    assert_eq!(received.time.tick, 6);
    assert_eq!(
        received.knowledge.level(&id("core:target")),
        KnowledgeLevel::Complete
    );
    assert!(matches!(
        received.knowledge.mission_state(&expedition.ship_id),
        Some(MissionState::Founded { .. })
    ));
    assert!(
        world
            .player_view()
            .unwrap()
            .systems
            .iter()
            .find(|system| system.system == id("core:target"))
            .unwrap()
            .local_state
            .is_some()
    );
    assert_eq!(
        world.commandability(&id("core:target")),
        Ok(Commandability::Commandable)
    );
}

#[test]
fn project_cancel_complete_launch_arrival_overflow_and_loss_reconcile_exactly() {
    let mut world = two_system_fixture(1_000, 999, 2, 2);
    let energy = id("core:energy");
    let ore = id("core:ore");
    let alloy = id("core:alloy");

    let cancelled = enqueue_expedition(&mut world, "core:origin_yard_slot_0");
    world.cancel_ship_project(&cancelled.project_id).unwrap();
    let after_cancel = world.debug_snapshot();
    assert_eq!(
        system(&after_cancel, "core:origin")
            .stocks
            .quantity(&energy),
        1_000
    );
    assert_eq!(
        system(&after_cancel, "core:origin")
            .energy_overflow
            .cumulative,
        0
    );

    let first = enqueue_expedition(&mut world, "core:origin_yard_slot_0");
    let second = enqueue_expedition(&mut world, "core:origin_yard_slot_1");
    let committed = world.debug_snapshot();
    assert_eq!(
        global_accounting(&committed, &energy, |value| &value.ship_project_committed),
        33
    );
    assert_eq!(
        global_accounting(&committed, &energy, |value| &value.ship_project_refunded),
        11
    );

    world.advance_tick().unwrap();
    let completed = world.debug_snapshot();
    assert_eq!(system(&completed, "core:origin").completed_assets.len(), 2);
    assert_eq!(
        global_accounting(&completed, &energy, |value| &value.construction_spent),
        12
    );
    for asset in &system(&completed, "core:origin").completed_assets {
        let CompletedAsset::Expedition { payload, .. } = asset else {
            panic!("expected expedition asset");
        };
        assert_eq!(payload.founding_stocks.quantity(&energy), 5);
        assert_eq!(payload.founding_stocks.quantity(&ore), 3);
    }

    for ship_id in [&first.ship_id, &second.ship_id] {
        world
            .launch_expedition(&id("core:origin"), ship_id, &id("core:target"), None)
            .unwrap();
    }
    let launched = world.debug_snapshot();
    assert_eq!(launched.transit.len(), 2);
    assert!(launched.transit.iter().all(|ship| {
        matches!(
            &ship.kind,
            TransitKind::Expedition { payload, .. }
                if payload.founding_stocks.quantity(&energy) == 5
                    && payload.founding_stocks.quantity(&ore) == 3
        )
    }));

    world.advance_tick().unwrap();
    let final_state = world.debug_snapshot();
    assert!(final_state.transit.is_empty());
    assert_eq!(final_state.populations.tokens.len(), 1);
    assert_eq!(final_state.population_accounting.removed, 1);
    assert_eq!(
        global_accounting(&final_state, &energy, |value| &value.founding_received),
        5
    );
    assert_eq!(
        global_accounting(&final_state, &energy, |value| &value.expedition_lost),
        5
    );

    for resource in [&energy, &ore, &alloy] {
        let committed = global_accounting(&final_state, resource, |value| {
            &value.ship_project_committed
        });
        let refunded =
            global_accounting(&final_state, resource, |value| &value.ship_project_refunded);
        let constructed =
            global_accounting(&final_state, resource, |value| &value.construction_spent);
        let received = global_accounting(&final_state, resource, |value| &value.founding_received);
        let lost = global_accounting(&final_state, resource, |value| &value.expedition_lost);
        assert_eq!(committed - refunded, constructed + received + lost);
    }

    // Actual construction-time initial stocks reconcile with bootstrap and scenario
    // production. Receipt accounting is not added twice because received stock is
    // already in target available stock.
    let initial_energy = 948_u64 + 999;
    let final_energy = global_stock(&final_state, &energy)
        + global_accounting(&final_state, &energy, |value| &value.construction_spent)
        + global_accounting(&final_state, &energy, |value| &value.operation_spent)
        + global_accounting(&final_state, &energy, |value| &value.travel_spent)
        + final_state
            .systems
            .iter()
            .map(|system| system.energy_overflow.cumulative)
            .sum::<u64>()
        + global_accounting(&final_state, &energy, |value| &value.expedition_lost);
    assert_eq!(final_energy, initial_energy + 112); // bootstrap plus scenario production
    assert_eq!(
        global_stock(&final_state, &ore)
            + global_accounting(&final_state, &ore, |value| &value.construction_spent)
            + global_accounting(&final_state, &ore, |value| &value.expedition_lost),
        100
    );
    assert_eq!(
        global_stock(&final_state, &alloy)
            + global_accounting(&final_state, &alloy, |value| &value.construction_spent)
            + global_accounting(&final_state, &alloy, |value| &value.expedition_lost),
        100
    );
}

#[test]
fn generated_population_ids_remain_unique_through_departure_arrival_and_loss() {
    let mut world = two_system_fixture(1_000, 0, 0, 2);
    world.advance_tick().unwrap();
    assert!(world.debug_snapshot().populations.tokens.is_empty());
    world.advance_tick().unwrap();
    let generated = world.debug_snapshot();
    let first_id = PopulationId::new(id("core:origin"), 0);
    let second_id = PopulationId::new(id("core:origin"), 1);
    assert_eq!(generated.populations.tokens.len(), 2);
    assert!(generated.populations.tokens.contains_key(&first_id));
    assert!(generated.populations.tokens.contains_key(&second_id));
    assert_eq!(generated.population_accounting.generated, 2);

    let first = enqueue_expedition(&mut world, "core:origin_yard_slot_0");
    let second = enqueue_expedition(&mut world, "core:origin_yard_slot_1");
    world.advance_tick().unwrap();
    for ship_id in [&first.ship_id, &second.ship_id] {
        world
            .launch_expedition(&id("core:origin"), ship_id, &id("core:target"), None)
            .unwrap();
    }
    let transit = world.debug_snapshot();
    assert_eq!(transit.populations.tokens.len(), 2);
    assert!(matches!(
        transit.populations.tokens[&first_id].state,
        PopulationState::InTransit { ref ship_id } if ship_id == &first.ship_id
    ));
    assert!(matches!(
        transit.populations.tokens[&second_id].state,
        PopulationState::InTransit { ref ship_id } if ship_id == &second.ship_id
    ));

    world.advance_tick().unwrap();
    let resolved = world.debug_snapshot();
    assert_eq!(resolved.populations.tokens.len(), 1);
    assert!(resolved.populations.tokens.contains_key(&first_id));
    assert!(!resolved.populations.tokens.contains_key(&second_id));
    assert!(matches!(
        resolved.populations.tokens[&first_id].state,
        PopulationState::Resident { ref community_id, .. }
            if community_id == &id("core:target_community")
    ));
    assert_eq!(resolved.population_accounting.initialized, 0);
    assert_eq!(resolved.population_accounting.generated, 2);
    assert_eq!(resolved.population_accounting.removed, 1);
    assert_eq!(
        resolved.population_accounting.initialized + resolved.population_accounting.generated,
        u64::try_from(resolved.populations.tokens.len()).unwrap()
            + resolved.population_accounting.removed
    );
    assert_eq!(
        resolved
            .population_accounting
            .entries
            .iter()
            .filter(|entry| entry.population_id == first_id)
            .count(),
        3
    );
    assert_eq!(
        resolved
            .population_accounting
            .entries
            .iter()
            .filter(|entry| entry.population_id == second_id)
            .count(),
        3
    );
    assert_eq!(
        system(&resolved, "core:origin")
            .counters
            .next_population_sequence,
        2
    );
}
