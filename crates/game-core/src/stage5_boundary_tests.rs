use crate::*;
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

fn recipe(values: &[(&str, u64)], required_work: u64) -> ConstructionRecipe {
    ConstructionRecipe {
        cost: store(values),
        required_work,
    }
}

fn tuning() -> WorldTuning {
    WorldTuning {
        energy_resource: id("core:energy"),
        ore_resource: id("core:ore"),
        alloy_resource: id("core:alloy"),
        seasonal_shape: [5, 6, 7, 8, 9, 10, 11, 12, 15, 17],
        seasonal_baseline_average: 10,
        life_support_per_population: 1,
        origin_construction_work: 1,
        intrinsic_energy_capacity: 1_000,
        battery_energy_capacity: 100,
        habitat_population_energy: 5,
        coordinate_quanta_per_map_unit: 1,
        collector_recipe: recipe(&[("core:energy", 2), ("core:alloy", 1)], 2),
        battery_recipe: recipe(&[("core:energy", 3)], 3),
        extractor_recipe: recipe(&[("core:energy", 4)], 4),
        refinery_recipe: recipe(&[("core:energy", 5)], 5),
        habitat_recipe: recipe(&[("core:energy", 6)], 6),
        shipyard_recipe: recipe(&[("core:energy", 7)], 7),
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
            energy_per_progress_tick: 1,
        },
        expedition_project: ExpeditionProjectTuning {
            hull_material_commitment: store(&[("core:energy", 4), ("core:alloy", 2)]),
            founding_stocks: store(&[("core:energy", 5), ("core:ore", 3)]),
            duration_ticks: 1,
            energy_per_progress_tick: 1,
        },
        probe_travel: ShipTravelTuning {
            maximum_jump_quanta: 3,
            speed_quanta_per_tick: 3,
            energy_per_quantum: FixedRate::new(1, NonZeroU64::new(2).unwrap()),
        },
        expedition_travel: ShipTravelTuning {
            maximum_jump_quanta: 3,
            speed_quanta_per_tick: 3,
            energy_per_quantum: FixedRate::new(1, NonZeroU64::new(2).unwrap()),
        },
        probe_reveal_radius_quanta: 3,
        communication_delay_per_quantum: FixedRate::new(1, NonZeroU64::new(1).unwrap()),
        resource_richness: BTreeMap::from([(
            id("core:ore"),
            RichnessThresholds {
                poor_minimum: 1,
                poor_maximum: 2,
                normal_minimum: 3,
                normal_maximum: 5,
                rich_minimum: 6,
            },
        )]),
    }
}

fn development(value: &str, role: DevelopmentRole) -> DevelopmentDefinition {
    DevelopmentDefinition {
        id: id(value),
        role,
        condition: DevelopmentCondition::Functional,
        extractor_target: (role == DevelopmentRole::Extractor).then(|| BodyResourceTarget {
            body: id("core:origin_body"),
            resource: id("core:ore"),
        }),
    }
}

fn slot(value: &str, development: Option<DevelopmentDefinition>) -> DevelopmentSlotDefinition {
    DevelopmentSlotDefinition {
        id: id(value),
        development,
    }
}

fn fixture() -> WorldState {
    let origin = id("core:origin");
    let community = id("core:origin_community");
    let occupied_habitat = id("core:habitat_occupied");
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
                id: id("core:hidden_mid"),
                name: "Hidden Mid".into(),
                position: Position3::from_quanta(3, 0, 0),
            },
            LocationDefinition {
                id: id("core:target"),
                name: "Target".into(),
                position: Position3::from_quanta(6, 0, 0),
            },
        ],
        origin_system: origin.clone(),
        origin_community: community.clone(),
        communities: vec![CommunityDefinition {
            id: community.clone(),
            system: origin.clone(),
        }],
        population_tokens: vec![PopulationToken {
            id: PopulationId::new(origin.clone(), 0),
            state: PopulationState::Resident {
                community_id: community,
                habitat_id: occupied_habitat,
            },
        }],
        systems: vec![
            SystemDefinition {
                location: origin,
                stellar_strength_hundredths: 100,
                bodies: vec![BodyDefinition {
                    id: id("core:origin_body"),
                    name: "Origin Body".into(),
                    eccentricity_hundredths: 0,
                    initial_resources: store(&[("core:ore", 10)]),
                    slots: vec![
                        slot(
                            "core:occupied_habitat_slot",
                            Some(development(
                                "core:habitat_occupied",
                                DevelopmentRole::Habitat,
                            )),
                        ),
                        slot(
                            "core:empty_habitat_slot",
                            Some(development("core:habitat_empty", DevelopmentRole::Habitat)),
                        ),
                        slot(
                            "core:shipyard_slot",
                            Some(development("core:shipyard", DevelopmentRole::Shipyard)),
                        ),
                        slot(
                            "core:collector_slot",
                            Some(development("core:collector", DevelopmentRole::Collector)),
                        ),
                        slot(
                            "core:battery_slot",
                            Some(development("core:battery", DevelopmentRole::Battery)),
                        ),
                        slot(
                            "core:extractor_slot",
                            Some(development("core:extractor", DevelopmentRole::Extractor)),
                        ),
                        slot(
                            "core:refinery_slot",
                            Some(development("core:refinery", DevelopmentRole::Refinery)),
                        ),
                        slot("core:build_slot", None),
                    ],
                }],
                stocks: store(&[("core:energy", 500), ("core:ore", 100), ("core:alloy", 100)]),
                player_founded: true,
                command_unlock_received: true,
            },
            SystemDefinition {
                location: id("core:hidden_mid"),
                stellar_strength_hundredths: 100,
                bodies: vec![BodyDefinition {
                    id: id("core:hidden_mid_body"),
                    name: "Hidden Mid Body".into(),
                    eccentricity_hundredths: 0,
                    initial_resources: ResourceStore::new(),
                    slots: vec![slot("core:hidden_mid_slot", None)],
                }],
                stocks: ResourceStore::new(),
                player_founded: false,
                command_unlock_received: false,
            },
            SystemDefinition {
                location: id("core:target"),
                stellar_strength_hundredths: 100,
                bodies: vec![BodyDefinition {
                    id: id("core:target_body"),
                    name: "Target Body".into(),
                    eccentricity_hundredths: 0,
                    initial_resources: ResourceStore::new(),
                    slots: vec![
                        slot("core:target_slot_0", None),
                        slot("core:target_slot_1", None),
                    ],
                }],
                stocks: ResourceStore::new(),
                player_founded: false,
                command_unlock_received: false,
            },
        ],
        sites: Vec::new(),
        tuning: tuning(),
    })
    .unwrap();

    let hidden = world
        .knowledge
        .systems
        .get_mut(&id("core:hidden_mid"))
        .unwrap();
    hidden.level = KnowledgeLevel::Anonymous;
    hidden.facts.clear();
    world
        .knowledge
        .systems
        .get_mut(&id("core:target"))
        .unwrap()
        .level = KnowledgeLevel::IdentifiedSummary;
    world
}

fn add_probe_asset(world: &mut WorldState, sequence: u64) -> ShipId {
    let ship_id = ShipId::new(id("core:origin"), sequence);
    world
        .systems
        .get_mut(&id("core:origin"))
        .unwrap()
        .completed_assets
        .push(CompletedAsset::Probe {
            ship_id: ship_id.clone(),
            available_at_tick: world.time.tick,
        });
    ship_id
}

fn add_expedition_asset(world: &mut WorldState, sequence: u64) -> ShipId {
    let ship_id = ShipId::new(id("core:origin"), sequence);
    world
        .systems
        .get_mut(&id("core:origin"))
        .unwrap()
        .completed_assets
        .push(CompletedAsset::Expedition {
            ship_id: ship_id.clone(),
            payload: ExpeditionPayload {
                founding_stocks: store(&[("core:energy", 5), ("core:ore", 3)]),
                collector_id: id(&format!("core:expedition_{sequence}_collector")),
                habitat_id: id(&format!("core:expedition_{sequence}_habitat")),
            },
            available_at_tick: world.time.tick,
        });
    ship_id
}

#[test]
fn player_projection_exposes_only_local_population_coordinates_and_core_phase() {
    let mut world = fixture();
    let view = world.player_view().unwrap();
    let origin = view
        .systems
        .iter()
        .find(|system| system.system == id("core:origin"))
        .unwrap()
        .local_state
        .as_ref()
        .unwrap();
    assert_eq!(view.seasonal_phase, 0);
    assert_eq!(origin.local_population.population_count, 1);
    assert_eq!(
        origin.local_population.occupied_habitat_slots,
        vec![SlotCoordinate {
            body: id("core:origin_body"),
            slot: id("core:occupied_habitat_slot"),
        }]
    );
    assert!(
        view.systems
            .iter()
            .find(|system| system.system == id("core:target"))
            .unwrap()
            .local_state
            .is_none()
    );

    world.advance_tick().unwrap();
    let generated = world.advance_tick().unwrap();
    let origin = generated
        .systems
        .iter()
        .find(|system| system.system == id("core:origin"))
        .unwrap()
        .local_state
        .as_ref()
        .unwrap();
    assert_eq!(origin.local_population.population_count, 2);
    assert_eq!(origin.local_population.occupied_habitat_slots.len(), 2);

    world.time.tick = 17;
    assert_eq!(world.player_view().unwrap().seasonal_phase, 7);
}

#[test]
fn construction_assessment_is_non_mutating_and_commit_revalidates_every_role() {
    for role in [
        DevelopmentRole::Collector,
        DevelopmentRole::Battery,
        DevelopmentRole::Extractor,
        DevelopmentRole::Refinery,
        DevelopmentRole::Habitat,
        DevelopmentRole::Shipyard,
    ] {
        let mut world = fixture();
        let extractor = (role == DevelopmentRole::Extractor).then(|| id("core:ore"));
        let before = world.debug_snapshot();
        let assessment = world.assess_construction(
            &id("core:origin"),
            &id("core:origin_body"),
            &id("core:build_slot"),
            role,
            extractor.as_ref(),
        );
        assert!(assessment.is_available(), "{role:?}: {assessment:?}");
        assert_eq!(
            assessment.required_work,
            recipe_for(&world.tuning, role).required_work
        );
        assert_eq!(assessment.cost, recipe_for(&world.tuning, role).cost);
        assert_eq!(world.debug_snapshot(), before);
        world
            .enqueue_construction(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:build_slot"),
                role,
                extractor.as_ref(),
            )
            .unwrap();
    }

    let mut stale = fixture();
    assert!(
        stale
            .assess_construction(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:build_slot"),
                DevelopmentRole::Battery,
                None,
            )
            .is_available()
    );
    stale
        .enqueue_construction(
            &id("core:origin"),
            &id("core:origin_body"),
            &id("core:build_slot"),
            DevelopmentRole::Collector,
            None,
        )
        .unwrap();
    for slot in ["core:build_slot", "core:occupied_habitat_slot"] {
        assert!(matches!(
            stale
                .assess_construction(
                    &id("core:origin"),
                    &id("core:origin_body"),
                    &id(slot),
                    DevelopmentRole::Battery,
                    None,
                )
                .limiting_reason,
            Some(CoreError::DevelopmentSlotUnavailable { .. })
        ));
    }
    assert!(matches!(
        stale.enqueue_construction(
            &id("core:origin"),
            &id("core:origin_body"),
            &id("core:build_slot"),
            DevelopmentRole::Battery,
            None,
        ),
        Err(CoreError::DevelopmentSlotUnavailable { .. })
    ));
}

#[test]
fn construction_and_habitat_assessments_return_typed_limiting_reasons() {
    let mut world = fixture();
    let before = world.debug_snapshot();
    assert!(matches!(
        world
            .assess_construction(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:build_slot"),
                DevelopmentRole::Extractor,
                None,
            )
            .limiting_reason,
        Some(CoreError::ExtractorTargetRequired)
    ));
    assert!(matches!(
        world
            .assess_construction(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:build_slot"),
                DevelopmentRole::Extractor,
                Some(&id("core:alloy")),
            )
            .limiting_reason,
        Some(CoreError::IncompatibleExtractorTarget { .. })
    ));
    world
        .systems
        .get_mut(&id("core:origin"))
        .unwrap()
        .stocks
        .set(id("core:energy"), 0);
    assert!(matches!(
        world
            .assess_construction(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:build_slot"),
                DevelopmentRole::Collector,
                None,
            )
            .limiting_reason,
        Some(CoreError::InsufficientResource { .. })
    ));
    world = fixture();
    for slot in ["core:occupied_habitat_slot", "core:shipyard_slot"] {
        assert!(matches!(
            world
                .assess_habitat_generation_toggle(
                    &id("core:origin"),
                    &id("core:origin_body"),
                    &id(slot),
                    false,
                )
                .limiting_reason,
            Some(CoreError::DevelopmentSlotUnavailable { .. })
        ));
    }
    let toggle = world.assess_habitat_generation_toggle(
        &id("core:origin"),
        &id("core:origin_body"),
        &id("core:empty_habitat_slot"),
        false,
    );
    assert!(toggle.is_available());
    assert_eq!(world.debug_snapshot(), before);
    world
        .set_habitat_generation_enabled(
            &id("core:origin"),
            &id("core:origin_body"),
            &id("core:empty_habitat_slot"),
            false,
        )
        .unwrap();
}

#[test]
fn habitat_commit_revalidates_occupancy_after_an_available_assessment() {
    let mut world = fixture();
    assert!(
        world
            .assess_habitat_generation_toggle(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:empty_habitat_slot"),
                false,
            )
            .is_available()
    );
    let population_id = PopulationId::new(id("core:origin"), 1);
    world.populations.tokens.insert(
        population_id.clone(),
        PopulationToken {
            id: population_id,
            state: PopulationState::Resident {
                community_id: id("core:origin_community"),
                habitat_id: id("core:habitat_empty"),
            },
        },
    );
    world.population_accounting.initialized += 1;
    world
        .systems
        .get_mut(&id("core:origin"))
        .unwrap()
        .counters
        .next_population_sequence = 2;
    assert!(matches!(
        world.set_habitat_generation_enabled(
            &id("core:origin"),
            &id("core:origin_body"),
            &id("core:empty_habitat_slot"),
            false,
        ),
        Err(CoreError::DevelopmentSlotUnavailable { .. })
    ));
}

#[test]
fn probe_assessment_is_non_mutating_redacted_and_agrees_with_atomic_launch() {
    let mut world = fixture();
    let ship_id = add_probe_asset(&mut world, 10);
    let before = world.debug_snapshot();
    let assessment = world.assess_probe_launch(&id("core:origin"), &ship_id, &id("core:target"), 3);
    assert!(assessment.is_available(), "{assessment:?}");
    assert!(assessment.asset_ready);
    assert_eq!(assessment.travel_energy, Some(3));
    assert_eq!(assessment.maximum_jump_limit, 3);
    assert_eq!(assessment.route.as_ref().unwrap().stops[1].system, None);
    assert_eq!(world.debug_snapshot(), before);

    let committed = world
        .launch_probe(&id("core:origin"), &ship_id, &id("core:target"), 3)
        .unwrap();
    assert_eq!(assessment.route, Some(committed));
    assert_eq!(
        world.player_view().unwrap().probe_reports.get(&ship_id),
        Some(&ProbeReportStatus::AwaitingReport)
    );
    assert!(matches!(
        world.launch_probe(&id("core:origin"), &ship_id, &id("core:target"), 3),
        Err(CoreError::UnknownCompletedShip(_))
    ));
}

#[test]
fn probe_assessment_reports_jump_asset_and_energy_failures_without_mutation() {
    let mut world = fixture();
    let ship_id = add_probe_asset(&mut world, 11);
    assert!(matches!(
        world
            .assess_probe_launch(&id("core:origin"), &ship_id, &id("core:target"), 4)
            .limiting_reason,
        Some(CoreError::InvalidProbeJumpLimit { .. })
    ));
    world
        .systems
        .get_mut(&id("core:origin"))
        .unwrap()
        .stocks
        .set(id("core:energy"), 0);
    let before = world.debug_snapshot();
    let assessment = world.assess_probe_launch(&id("core:origin"), &ship_id, &id("core:target"), 3);
    assert_eq!(assessment.travel_energy, Some(3));
    assert!(matches!(
        assessment.limiting_reason,
        Some(CoreError::InsufficientResource { .. })
    ));
    assert_eq!(world.debug_snapshot(), before);
}

#[test]
fn expedition_assessment_exposes_commitment_and_population_readiness_then_agrees() {
    let mut world = fixture();
    let ship_id = add_expedition_asset(&mut world, 20);
    let stale_ship_id = add_expedition_asset(&mut world, 24);
    let before = world.debug_snapshot();
    let assessment =
        world.assess_expedition_launch(&id("core:origin"), &ship_id, &id("core:target"), None);
    assert!(assessment.is_available(), "{assessment:?}");
    assert_eq!(
        assessment.target_knowledge,
        KnowledgeLevel::IdentifiedSummary
    );
    assert_eq!(assessment.resident_population_required, 1);
    assert_eq!(assessment.resident_population_available, 1);
    assert!(assessment.resident_population_ready && assessment.asset_ready);
    let commitment = &assessment.complete_commitment;
    assert_eq!(commitment.quantity(&id("core:energy")), 11);
    assert_eq!(commitment.quantity(&id("core:alloy")), 3);
    assert_eq!(commitment.quantity(&id("core:ore")), 3);
    assert_eq!(assessment.travel_energy, Some(3));
    assert_eq!(assessment.route.as_ref().unwrap().stops[1].system, None);
    assert!(
        world
            .assess_expedition_launch(&id("core:origin"), &stale_ship_id, &id("core:target"), None,)
            .is_available()
    );
    assert_eq!(world.debug_snapshot(), before);

    let route = world
        .launch_expedition(&id("core:origin"), &ship_id, &id("core:target"), None)
        .unwrap();
    assert_eq!(assessment.route, Some(route));
    let origin = world
        .player_view()
        .unwrap()
        .systems
        .into_iter()
        .find(|system| system.system == id("core:origin"))
        .unwrap()
        .local_state
        .unwrap();
    assert_eq!(origin.local_population.population_count, 0);
    assert!(origin.local_population.occupied_habitat_slots.is_empty());
    assert!(matches!(
        world.launch_expedition(&id("core:origin"), &stale_ship_id, &id("core:target"), None,),
        Err(CoreError::NoResidentPopulation(_))
    ));
}

#[test]
fn expedition_assessment_reports_population_energy_and_reservation_collisions() {
    let mut no_population = fixture();
    no_population.populations.tokens.clear();
    no_population.population_accounting.initialized = 0;
    let ship_id = add_expedition_asset(&mut no_population, 21);
    assert!(matches!(
        no_population
            .assess_expedition_launch(&id("core:origin"), &ship_id, &id("core:target"), None,)
            .limiting_reason,
        Some(CoreError::NoResidentPopulation(_))
    ));

    let mut no_energy = fixture();
    let ship_id = add_expedition_asset(&mut no_energy, 22);
    no_energy
        .systems
        .get_mut(&id("core:origin"))
        .unwrap()
        .stocks
        .set(id("core:energy"), 0);
    let assessment =
        no_energy.assess_expedition_launch(&id("core:origin"), &ship_id, &id("core:target"), None);
    assert_eq!(assessment.travel_energy, Some(3));
    assert!(matches!(
        assessment.limiting_reason,
        Some(CoreError::InsufficientResource { .. })
    ));

    let mut collision = fixture();
    let ship_id = add_expedition_asset(&mut collision, 23);
    let target = collision
        .knowledge
        .systems
        .get_mut(&id("core:target"))
        .unwrap();
    target.level = KnowledgeLevel::Complete;
    target.facts.insert(
        FactKey::SlotOrder {
            body: id("core:target_body"),
        },
        KnowledgeFact {
            value: FactValue::ContentIds(vec![id("core:target_slot_0"), id("core:target_slot_1")]),
            detail: FactDetail::Complete,
            tick_observed: 0,
            observer: ObserverId::InitialOrigin(id("core:origin")),
            tick_received: 0,
        },
    );
    collision
        .systems
        .get_mut(&id("core:target"))
        .unwrap()
        .bodies[0]
        .slots[0]
        .reserved_by = Some(ReservationOwner::Expedition(ShipId::new(
        id("core:origin"),
        99,
    )));
    let reservations = ExpeditionReservations {
        habitat: SlotCoordinate {
            body: id("core:target_body"),
            slot: id("core:target_slot_0"),
        },
        collector: SlotCoordinate {
            body: id("core:target_body"),
            slot: id("core:target_slot_1"),
        },
    };
    let before = collision.debug_snapshot();
    assert!(matches!(
        collision
            .assess_expedition_launch(
                &id("core:origin"),
                &ship_id,
                &id("core:target"),
                Some(reservations),
            )
            .limiting_reason,
        Some(CoreError::InvalidExpeditionReservation(_))
    ));
    assert_eq!(collision.debug_snapshot(), before);
}

#[test]
fn generic_operational_toggle_stops_all_development_work_and_preserves_state() {
    let mut world = fixture();
    let system = id("core:origin");
    let body = id("core:origin_body");
    let roles = [
        "core:occupied_habitat_slot",
        "core:empty_habitat_slot",
        "core:collector_slot",
        "core:battery_slot",
        "core:extractor_slot",
        "core:refinery_slot",
    ];

    world.tuning.extractor.cycle_duration = 2;
    world.tuning.refinery.cycle_duration = 2;
    {
        let state = world.systems.get_mut(&system).unwrap();
        find_slot_mut(&mut state.bodies, &body, &id("core:empty_habitat_slot"))
            .unwrap()
            .development
            .as_mut()
            .unwrap()
            .habitat
            .as_mut()
            .unwrap()
            .generation_progress = 3;
        find_slot_mut(&mut state.bodies, &body, &id("core:extractor_slot"))
            .unwrap()
            .development
            .as_mut()
            .unwrap()
            .cycle
            .progress = 1;
        let refinery = find_slot_mut(&mut state.bodies, &body, &id("core:refinery_slot"))
            .unwrap()
            .development
            .as_mut()
            .unwrap();
        refinery.cycle.progress = 1;
        refinery.cycle.committed_inputs = store(&[("core:ore", 1)]);
    }

    for slot in roles {
        let slot = id(slot);
        let assessment = world.assess_development_operational_toggle(&system, &body, &slot, false);
        assert!(assessment.is_available());
        assert!(!assessment.enabled);
        world
            .set_development_operational_enabled(&system, &body, &slot, false)
            .unwrap();
    }

    assert!(matches!(
        world
            .assess_development_operational_toggle(&system, &body, &id("core:build_slot"), false,)
            .limiting_reason,
        Some(CoreError::DevelopmentSlotUnavailable { .. })
    ));
    assert!(matches!(
        world
            .assess_development_operational_toggle(
                &id("core:target"),
                &id("core:target_body"),
                &id("core:target_slot_0"),
                false,
            )
            .limiting_reason,
        Some(CoreError::SystemNotCommandable(_))
    ));

    let before = world.systems.get(&system).unwrap().clone();
    world.advance_tick().unwrap();
    let after = world.systems.get(&system).unwrap();
    assert_eq!(after.stocks, before.stocks);
    assert_eq!(
        after.bodies[0].remaining_resources,
        before.bodies[0].remaining_resources
    );
    assert_eq!(after.life_support.supported_population, 0);
    assert_eq!(after.life_support.underserved_population, 1);
    assert_eq!(energy_capacity(after, &world.tuning).unwrap(), 1_000);
    for slot in roles {
        let before_development = find_slot(&before.bodies, &body, &id(slot))
            .unwrap()
            .development
            .as_ref()
            .unwrap();
        let after_development = find_slot(&after.bodies, &body, &id(slot))
            .unwrap()
            .development
            .as_ref()
            .unwrap();
        assert!(!after_development.enabled);
        assert_eq!(after_development.cycle, before_development.cycle);
        assert_eq!(after_development.habitat, before_development.habitat);
    }
}

#[test]
fn disabling_battery_applies_new_capacity_atomically() {
    let mut world = fixture();
    let system = id("core:origin");
    let body = id("core:origin_body");
    let slot = id("core:battery_slot");
    world.tuning.intrinsic_energy_capacity = 400;
    let before = world.debug_snapshot();

    assert!(
        world
            .assess_development_operational_toggle(&system, &body, &slot, false)
            .is_available()
    );
    assert_eq!(world.debug_snapshot(), before);
    world
        .set_development_operational_enabled(&system, &body, &slot, false)
        .unwrap();

    let state = &world.systems[&system];
    assert_eq!(state.stocks.quantity(&id("core:energy")), 400);
    assert_eq!(energy_capacity(state, &world.tuning).unwrap(), 400);
    assert_eq!(state.overflow.cumulative, 100);
    assert_eq!(
        state.overflow.evidence.last().unwrap().cause,
        EnergyOverflowCause::DevelopmentOperationalToggle
    );
}

#[test]
fn disabled_shipyard_pauses_projects_and_blocks_enqueue_and_launch() {
    let mut world = fixture();
    let system = id("core:origin");
    let body = id("core:origin_body");
    let slot = id("core:shipyard_slot");
    let ids = world
        .enqueue_ship_project(&system, &body, &slot, ShipProjectKind::Probe)
        .unwrap();
    world
        .set_development_operational_enabled(&system, &body, &slot, false)
        .unwrap();

    assert!(matches!(
        world.enqueue_ship_project(&system, &body, &slot, ShipProjectKind::Probe),
        Err(CoreError::NotFunctionalShipyard { .. })
    ));
    world.advance_tick().unwrap();
    let project = find_slot(&world.systems[&system].bodies, &body, &slot)
        .unwrap()
        .development
        .as_ref()
        .unwrap()
        .shipyard
        .as_ref()
        .unwrap()
        .queue
        .first()
        .unwrap();
    assert_eq!(project.id, ids.project_id);
    assert_eq!(project.progress, 0);

    world
        .set_development_operational_enabled(&system, &body, &slot, true)
        .unwrap();
    world.advance_tick().unwrap();
    world
        .set_development_operational_enabled(&system, &body, &slot, false)
        .unwrap();
    let assessment = world.assess_probe_launch(&system, &ids.ship_id, &id("core:target"), 3);
    assert!(!assessment.asset_ready);
    assert!(matches!(
        assessment.limiting_reason,
        Some(CoreError::NoOperationalShipyard(ref failed)) if failed == &system
    ));
    assert!(matches!(
        world.launch_probe(&system, &ids.ship_id, &id("core:target"), 3),
        Err(CoreError::NoOperationalShipyard(ref failed)) if failed == &system
    ));
}
