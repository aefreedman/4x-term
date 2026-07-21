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

fn tuning(probe_duration: u64, expedition_duration: u64) -> WorldTuning {
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
            duration_ticks: probe_duration,
            energy_per_progress_tick: 2,
        },
        expedition_project: ExpeditionProjectTuning {
            hull_material_commitment: store(&[("core:energy", 4), ("core:alloy", 2)]),
            founding_stocks: store(&[("core:energy", 5), ("core:ore", 3)]),
            duration_ticks: expedition_duration,
            energy_per_progress_tick: 2,
        },
        probe_travel: ShipTravelTuning {
            maximum_jump_quanta: 10,
            speed_quanta_per_tick: 4,
            energy_per_quantum: FixedRate::new(1, NonZeroU64::new(4).unwrap()),
        },
        expedition_travel: ShipTravelTuning {
            maximum_jump_quanta: 5,
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

fn body(value: &str, slots: Vec<DevelopmentSlotDefinition>) -> BodyDefinition {
    BodyDefinition {
        id: id(value),
        name: value.into(),
        eccentricity_hundredths: 0,
        initial_resources: ResourceStore::new(),
        slots,
    }
}

fn fixture(
    energy: u64,
    target_energy: u64,
    population_count: u64,
    target_slots: usize,
    probe_duration: u64,
    expedition_duration: u64,
) -> WorldState {
    let source_slots = vec![
        slot(
            "core:z_habitat_slot",
            Some(development("core:z_habitat", DevelopmentRole::Habitat)),
        ),
        slot(
            "core:a_habitat_slot",
            Some(development("core:a_habitat", DevelopmentRole::Habitat)),
        ),
        slot(
            "core:yard_slot_a",
            Some(development("core:yard_a", DevelopmentRole::Shipyard)),
        ),
        slot(
            "core:yard_slot_b",
            Some(development("core:yard_b", DevelopmentRole::Shipyard)),
        ),
    ];
    let target_slots = (0..target_slots)
        .map(|index| {
            let id = match index {
                0 => "core:z_target_slot".into(),
                1 => "core:a_target_slot".into(),
                _ => format!("core:m_target_slot_{index}"),
            };
            slot(&id, None)
        })
        .collect();
    let systems = [
        (
            "core:origin",
            0,
            0,
            body("core:origin_body", source_slots),
            true,
        ),
        (
            "core:mid",
            4,
            0,
            body("core:mid_body", vec![slot("core:mid_slot", None)]),
            false,
        ),
        (
            "core:target",
            8,
            0,
            body("core:target_body", target_slots),
            false,
        ),
        (
            "core:route_bridge",
            10,
            -4,
            body(
                "core:route_bridge_body",
                vec![slot("core:route_bridge_slot", None)],
            ),
            false,
        ),
        (
            "core:far",
            6,
            -7,
            body(
                "core:far_body",
                vec![slot("core:far_slot_0", None), slot("core:far_slot_1", None)],
            ),
            false,
        ),
    ];
    let origin_community = id("core:origin_community");
    let mut fixture_tuning = tuning(probe_duration, expedition_duration);
    if population_count != 0 {
        fixture_tuning.habitat_population_energy = 1;
    }
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
        locations: systems
            .iter()
            .map(|(system, x, y, _, _)| LocationDefinition {
                id: id(system),
                name: (*system).into(),
                position: Position3::from_quanta(*x, *y, 0),
            })
            .collect(),
        origin_system: id("core:origin"),
        origin_community: origin_community.clone(),
        communities: vec![CommunityDefinition {
            id: origin_community,
            system: id("core:origin"),
        }],
        population_tokens: Vec::new(),
        systems: systems
            .into_iter()
            .map(|(system, _, _, body, founded)| SystemDefinition {
                location: id(system),
                stellar_strength_hundredths: 100,
                bodies: vec![body],
                stocks: if system == "core:origin" {
                    store(&[
                        ("core:energy", energy + population_count * 2),
                        ("core:ore", 100),
                        ("core:alloy", 100),
                    ])
                } else if target_energy == 0 {
                    ResourceStore::new()
                } else {
                    store(&[("core:energy", target_energy), ("core:alloy", 4)])
                },
                player_founded: founded,
                command_unlock_received: founded,
            })
            .collect(),
        sites: Vec::new(),
        tuning: fixture_tuning,
    })
    .unwrap();
    if population_count != 0 {
        world.advance_tick().unwrap();
        world.advance_tick().unwrap();
        assert_eq!(
            world.debug_snapshot().populations.tokens.len(),
            population_count as usize
        );
    }
    world
}

fn shipyard_queue<'a>(snapshot: &'a SystemSnapshot, slot_id: &str) -> &'a [ShipyardProject] {
    &snapshot.bodies[0]
        .slots
        .iter()
        .find(|slot| slot.id == id(slot_id))
        .unwrap()
        .development
        .as_ref()
        .unwrap()
        .shipyard
        .as_ref()
        .unwrap()
        .queue
}

fn build_asset(
    world: &mut WorldState,
    kind: ShipProjectKind,
    yard_slot: &str,
    duration: u64,
) -> ShipProjectIds {
    let ids = world
        .enqueue_ship_project(
            &id("core:origin"),
            &id("core:origin_body"),
            &id(yard_slot),
            kind,
        )
        .unwrap();
    for _ in 0..duration {
        world.advance_tick().unwrap();
    }
    ids
}

fn wait_for_complete_knowledge(world: &mut WorldState, system: &ContentId) {
    for _ in 0..20 {
        if world.debug_snapshot().knowledge.level(system) == KnowledgeLevel::Complete {
            return;
        }
        world.advance_tick().unwrap();
    }
    panic!("complete observation was not received");
}

fn wait_for_mission_resolution(world: &mut WorldState, ship_id: &ShipId) {
    for _ in 0..20 {
        if !matches!(
            world.debug_snapshot().knowledge.mission_state(ship_id),
            Some(MissionState::AwaitingOutcome { .. })
        ) {
            return;
        }
        world.advance_tick().unwrap();
    }
    panic!("mission outcome was not received");
}

#[test]
fn shipyards_have_independent_fifo_queues_pause_cancel_and_never_reuse_ids() {
    // Three complete probe commitments plus exactly two ticks of life support.
    let mut world = fixture(11, 0, 2, 2, 2, 2);
    let first = world
        .enqueue_ship_project(
            &id("core:origin"),
            &id("core:origin_body"),
            &id("core:yard_slot_a"),
            ShipProjectKind::Probe,
        )
        .unwrap();
    let second = world
        .enqueue_ship_project(
            &id("core:origin"),
            &id("core:origin_body"),
            &id("core:yard_slot_a"),
            ShipProjectKind::Probe,
        )
        .unwrap();
    let third = world
        .enqueue_ship_project(
            &id("core:origin"),
            &id("core:origin_body"),
            &id("core:yard_slot_b"),
            ShipProjectKind::Probe,
        )
        .unwrap();
    assert_eq!((first.project_id.sequence, first.ship_id.sequence), (0, 0));
    assert_eq!(
        (second.project_id.sequence, second.ship_id.sequence),
        (1, 1)
    );
    assert_eq!((third.project_id.sequence, third.ship_id.sequence), (2, 2));

    world.advance_tick().unwrap();
    let paused = world.debug_system_snapshot(&id("core:origin")).unwrap();
    assert_eq!(shipyard_queue(&paused, "core:yard_slot_a")[0].progress, 0);
    assert_eq!(shipyard_queue(&paused, "core:yard_slot_b")[0].progress, 0);
    assert_eq!(
        paused
            .accounting
            .ship_project_committed
            .quantity(&id("core:energy")),
        9
    );

    world.cancel_ship_project(&second.project_id).unwrap();
    let after_cancel = world.debug_system_snapshot(&id("core:origin")).unwrap();
    assert_eq!(shipyard_queue(&after_cancel, "core:yard_slot_a").len(), 1);
    assert_eq!(shipyard_queue(&after_cancel, "core:yard_slot_b").len(), 1);
    assert_eq!(
        after_cancel
            .accounting
            .ship_project_refunded
            .quantity(&id("core:energy")),
        3
    );

    let fourth = world
        .enqueue_ship_project(
            &id("core:origin"),
            &id("core:origin_body"),
            &id("core:yard_slot_a"),
            ShipProjectKind::Probe,
        )
        .unwrap();
    assert_eq!(
        (fourth.project_id.sequence, fourth.ship_id.sequence),
        (3, 3)
    );
}

#[test]
fn begun_shipyard_project_cancellation_rejects_without_mutating_world() {
    let mut world = fixture(100, 0, 0, 2, 2, 2);
    let project = world
        .enqueue_ship_project(
            &id("core:origin"),
            &id("core:origin_body"),
            &id("core:yard_slot_a"),
            ShipProjectKind::Probe,
        )
        .unwrap();
    world.advance_tick().unwrap();
    assert_eq!(
        shipyard_queue(
            &world.debug_system_snapshot(&id("core:origin")).unwrap(),
            "core:yard_slot_a",
        )[0]
        .progress,
        1
    );

    let before_rejection = world.debug_snapshot();
    assert_eq!(
        world.cancel_ship_project(&project.project_id),
        Err(CoreError::ShipProjectAlreadyBegun(
            project.project_id.clone()
        ))
    );
    assert_eq!(world.debug_snapshot(), before_rejection);
}

#[test]
fn probe_duration_one_multileg_stops_reveal_and_launch_rejections_are_exact() {
    let mut world = fixture(500, 0, 2, 2, 1, 1);
    let direct = build_asset(&mut world, ShipProjectKind::Probe, "core:yard_slot_a", 1);
    let player_view = world.player_view().unwrap();
    assert!(
        !player_view
            .systems
            .iter()
            .any(|system| system.system == id("core:route_bridge"))
    );
    assert!(player_view.anonymous_indication_count > 0);
    let before_rejection = world.debug_snapshot();
    assert_eq!(
        world.launch_probe(
            &id("core:origin"),
            &direct.ship_id,
            &id("core:route_bridge"),
            10,
        ),
        Err(CoreError::SystemNotTargetable(id("core:route_bridge")))
    );
    assert_eq!(world.debug_snapshot(), before_rejection);
    assert!(matches!(
        world.launch_probe(&id("core:origin"), &direct.ship_id, &id("core:mid"), 11,),
        Err(CoreError::InvalidProbeJumpLimit { .. })
    ));

    let route = world
        .launch_probe(&id("core:origin"), &direct.ship_id, &id("core:mid"), 5)
        .unwrap();
    assert_eq!(route.total_distance, 4);
    assert_eq!(world.debug_snapshot().transit[0].remaining_leg_ticks, 1);
    assert_eq!(
        world
            .debug_system_snapshot(&id("core:origin"))
            .unwrap()
            .accounting
            .travel_spent
            .quantity(&id("core:energy")),
        1
    );
    world.advance_tick().unwrap();
    assert!(world.debug_snapshot().transit.is_empty());

    let multileg = build_asset(&mut world, ShipProjectKind::Probe, "core:yard_slot_b", 1);
    let route = world
        .launch_probe(&id("core:origin"), &multileg.ship_id, &id("core:target"), 5)
        .unwrap();
    assert_eq!(
        route
            .stops
            .iter()
            .map(|stop| stop.system.clone())
            .collect::<Vec<_>>(),
        vec![
            Some(id("core:origin")),
            Some(id("core:mid")),
            Some(id("core:target")),
        ]
    );
    assert_eq!(
        world
            .debug_system_snapshot(&id("core:origin"))
            .unwrap()
            .accounting
            .travel_spent
            .quantity(&id("core:energy")),
        3
    );
    world.advance_tick().unwrap();
    let at_mid = world.debug_snapshot();
    assert_eq!(at_mid.transit.len(), 1);
    assert!(at_mid.transit[0].reached_stops.contains(&id("core:mid")));
    assert_eq!(at_mid.transit[0].remaining_leg_ticks, 1);
    world.advance_tick().unwrap();
    assert!(world.debug_snapshot().transit.is_empty());
    wait_for_complete_knowledge(&mut world, &id("core:target"));
    // The mid-stop reveal scan retains the otherwise anonymous bridge indication.
    assert!(
        world
            .debug_snapshot()
            .knowledge
            .level(&id("core:route_bridge"))
            >= KnowledgeLevel::Anonymous
    );
}

#[test]
fn player_view_recomputes_active_route_revelation_and_hides_arrived_outcome() {
    let mut world = fixture(500, 999, 2, 3, 1, 1);

    let scouting_probe = build_asset(&mut world, ShipProjectKind::Probe, "core:yard_slot_a", 1);
    world
        .launch_probe(
            &id("core:origin"),
            &scouting_probe.ship_id,
            &id("core:target"),
            10,
        )
        .unwrap();
    world.advance_tick().unwrap();
    world.advance_tick().unwrap();
    wait_for_complete_knowledge(&mut world, &id("core:target"));

    let founding = build_asset(
        &mut world,
        ShipProjectKind::Expedition,
        "core:yard_slot_a",
        1,
    );
    world
        .launch_expedition(
            &id("core:origin"),
            &founding.ship_id,
            &id("core:target"),
            Some(ExpeditionReservations {
                habitat: SlotCoordinate {
                    body: id("core:target_body"),
                    slot: id("core:z_target_slot"),
                },
                collector: SlotCoordinate {
                    body: id("core:target_body"),
                    slot: id("core:a_target_slot"),
                },
            }),
        )
        .unwrap();
    world.advance_tick().unwrap();
    world.advance_tick().unwrap();
    wait_for_mission_resolution(&mut world, &founding.ship_id);

    world
        .enqueue_construction(
            &id("core:target"),
            &id("core:target_body"),
            &id("core:m_target_slot_2"),
            DevelopmentRole::Shipyard,
            None,
        )
        .unwrap();
    world.advance_tick().unwrap();

    let probe = world
        .enqueue_ship_project(
            &id("core:target"),
            &id("core:target_body"),
            &id("core:m_target_slot_2"),
            ShipProjectKind::Probe,
        )
        .unwrap();
    world.advance_tick().unwrap();
    let probe_route = world
        .launch_probe(&id("core:target"), &probe.ship_id, &id("core:far"), 10)
        .unwrap();
    assert_eq!(
        probe_route.stops.len(),
        2,
        "the probe takes the direct route"
    );
    world.advance_tick().unwrap();
    world.advance_tick().unwrap();
    wait_for_complete_knowledge(&mut world, &id("core:far"));
    let far_view = world
        .player_view()
        .unwrap()
        .systems
        .into_iter()
        .find(|system| system.system == id("core:far"))
        .unwrap();
    assert_eq!(
        far_view.knowledge.facts[&FactKey::Position].value,
        FactValue::Position(Position3::from_quanta(6, -7, 0))
    );
    assert_eq!(
        world
            .debug_snapshot()
            .knowledge
            .level(&id("core:route_bridge")),
        KnowledgeLevel::Anonymous
    );

    let expedition = world
        .enqueue_ship_project(
            &id("core:target"),
            &id("core:target_body"),
            &id("core:m_target_slot_2"),
            ShipProjectKind::Expedition,
        )
        .unwrap();
    world.advance_tick().unwrap();
    world
        .launch_expedition(
            &id("core:target"),
            &expedition.ship_id,
            &id("core:far"),
            Some(ExpeditionReservations {
                habitat: SlotCoordinate {
                    body: id("core:far_body"),
                    slot: id("core:far_slot_0"),
                },
                collector: SlotCoordinate {
                    body: id("core:far_body"),
                    slot: id("core:far_slot_1"),
                },
            }),
        )
        .unwrap();

    let launched = world.player_view().unwrap();
    let active = &launched.active_routes[&expedition.ship_id];
    assert_eq!(
        active
            .stops
            .iter()
            .map(|stop| (stop.system.clone(), stop.reached))
            .collect::<Vec<_>>(),
        vec![
            (Some(id("core:target")), true),
            (None, false),
            (Some(id("core:far")), false),
        ]
    );

    world.advance_tick().unwrap();
    let before_hidden_stop = world.player_view().unwrap();
    assert_eq!(
        before_hidden_stop.active_routes[&expedition.ship_id].stops[1],
        RedactedRouteStop {
            system: None,
            reached: false,
        }
    );
    world.advance_tick().unwrap();
    let at_hidden_stop = world.player_view().unwrap();
    assert_eq!(
        at_hidden_stop.active_routes[&expedition.ship_id].stops[1],
        RedactedRouteStop {
            system: Some(id("core:route_bridge")),
            reached: true,
        }
    );

    world.advance_tick().unwrap();
    world.advance_tick().unwrap();
    let arrived = world.player_view().unwrap();
    assert!(!arrived.active_routes.contains_key(&expedition.ship_id));
    assert!(matches!(
        arrived.missions.get(&expedition.ship_id),
        Some(MissionState::AwaitingOutcome { target }) if target == &id("core:far")
    ));
}

#[test]
fn complete_knowledge_reserves_typed_slots_and_success_unlocks_only_after_report() {
    let mut world = fixture(500, 999, 2, 3, 1, 1);
    let probe = build_asset(&mut world, ShipProjectKind::Probe, "core:yard_slot_a", 1);
    world
        .launch_probe(&id("core:origin"), &probe.ship_id, &id("core:target"), 5)
        .unwrap();
    world.advance_tick().unwrap();
    world.advance_tick().unwrap();
    wait_for_complete_knowledge(&mut world, &id("core:target"));

    let first = build_asset(
        &mut world,
        ShipProjectKind::Expedition,
        "core:yard_slot_a",
        1,
    );
    let second = build_asset(
        &mut world,
        ShipProjectKind::Expedition,
        "core:yard_slot_b",
        1,
    );
    // Founding Energy is received during movement and overflows only in phase 10.
    let reservations = ExpeditionReservations {
        habitat: SlotCoordinate {
            body: id("core:target_body"),
            slot: id("core:z_target_slot"),
        },
        collector: SlotCoordinate {
            body: id("core:target_body"),
            slot: id("core:a_target_slot"),
        },
    };
    world
        .launch_expedition(
            &id("core:origin"),
            &first.ship_id,
            &id("core:target"),
            Some(reservations.clone()),
        )
        .unwrap();
    assert_eq!(
        world.commandability(&id("core:target")),
        Ok(Commandability::AwaitingFoundingOutcome),
        "commandability stays at the player-safe awaiting state throughout transit"
    );
    let target = world.debug_system_snapshot(&id("core:target")).unwrap();
    assert!(target.bodies[0].slots[..2].iter().all(|slot| {
        slot.reserved_by == Some(ReservationOwner::Expedition(first.ship_id.clone()))
    }));
    assert!(target.bodies[0].slots[2].reserved_by.is_none());

    let before_collision = world.debug_snapshot();
    assert!(matches!(
        world.launch_expedition(
            &id("core:origin"),
            &second.ship_id,
            &id("core:target"),
            Some(reservations),
        ),
        Err(CoreError::InvalidExpeditionReservation(_))
    ));
    assert_eq!(world.debug_snapshot(), before_collision);

    world.advance_tick().unwrap();
    world.advance_tick().unwrap();
    let arrived = world.debug_snapshot();
    assert!(arrived.transit.is_empty());
    assert!(matches!(
        arrived.knowledge.mission_state(&first.ship_id),
        Some(MissionState::AwaitingOutcome { .. })
    ));
    assert_eq!(
        world.commandability(&id("core:target")),
        Ok(Commandability::AwaitingFoundingOutcome)
    );
    let target = world.debug_system_snapshot(&id("core:target")).unwrap();
    assert_eq!(target.stocks.quantity(&id("core:energy")), 1_000);
    assert_eq!(target.stocks.quantity(&id("core:ore")), 3);
    assert_eq!(target.accounting.produced.quantity(&id("core:energy")), 0);
    assert_eq!(target.energy_overflow.last_tick_retention, 4);
    assert_eq!(target.energy_overflow.cumulative, 4);
    assert_eq!(
        target
            .accounting
            .founding_received
            .quantity(&id("core:energy")),
        5
    );
    let source = world.debug_system_snapshot(&id("core:origin")).unwrap();
    assert_eq!(
        source
            .accounting
            .ship_project_committed
            .quantity(&id("core:energy")),
        25
    );
    assert_eq!(
        source
            .accounting
            .construction_spent
            .quantity(&id("core:energy")),
        15
    );
    assert_eq!(
        source.accounting.travel_spent.quantity(&id("core:energy")),
        4
    );
    assert_eq!(
        world
            .debug_snapshot()
            .populations
            .tokens
            .values()
            .filter(|token| matches!(token.state, PopulationState::InTransit { .. }))
            .count(),
        0
    );

    // The arrived Collector and population first operate on the following tick.
    world.advance_tick().unwrap();
    let operating = world.debug_system_snapshot(&id("core:target")).unwrap();
    assert_eq!(
        operating.accounting.produced.quantity(&id("core:energy")),
        28
    );
    assert_eq!(operating.life_support.supported_population, 1);
    assert!(matches!(
        world
            .debug_snapshot()
            .knowledge
            .mission_state(&first.ship_id),
        Some(MissionState::AwaitingOutcome { .. })
    ));

    wait_for_mission_resolution(&mut world, &first.ship_id);
    assert!(matches!(
        world.debug_snapshot().knowledge.mission_state(&first.ship_id),
        Some(MissionState::Founded {
            target,
            habitat_id,
            collector_id,
            ..
        }) if target == &id("core:target")
            && habitat_id == &id("core:origin_expedition_1_habitat")
            && collector_id == &id("core:origin_expedition_1_collector")
    ));
    assert_eq!(
        world.commandability(&id("core:target")),
        Ok(Commandability::Commandable)
    );

    world
        .enqueue_construction(
            &id("core:target"),
            &id("core:target_body"),
            &id("core:m_target_slot_2"),
            DevelopmentRole::Shipyard,
            None,
        )
        .unwrap();
    world.advance_tick().unwrap();
    let return_expedition = world
        .enqueue_ship_project(
            &id("core:target"),
            &id("core:target_body"),
            &id("core:m_target_slot_2"),
            ShipProjectKind::Expedition,
        )
        .unwrap();
    world.advance_tick().unwrap();
    world
        .launch_expedition(
            &id("core:target"),
            &return_expedition.ship_id,
            &id("core:origin"),
            None,
        )
        .unwrap();
    assert_eq!(
        world.commandability(&id("core:target")),
        Ok(Commandability::Depopulated)
    );
    world.advance_tick().unwrap();
    assert_eq!(
        world.commandability(&id("core:target")),
        Ok(Commandability::Depopulated)
    );
    world.advance_tick().unwrap();
    assert_eq!(
        world.commandability(&id("core:target")),
        Ok(Commandability::Commandable),
        "the vacated enabled Habitat automatically repopulates the founded remote"
    );
}

#[test]
fn simultaneous_summary_arrivals_succeed_then_lose_in_ship_id_order() {
    let mut world = fixture(500, 0, 2, 2, 1, 1);
    assert_eq!(
        world.debug_snapshot().knowledge.level(&id("core:target")),
        KnowledgeLevel::IdentifiedSummary
    );
    let first = build_asset(
        &mut world,
        ShipProjectKind::Expedition,
        "core:yard_slot_a",
        1,
    );
    let second = build_asset(
        &mut world,
        ShipProjectKind::Expedition,
        "core:yard_slot_b",
        1,
    );
    for ship_id in [&first.ship_id, &second.ship_id] {
        world
            .launch_expedition(&id("core:origin"), ship_id, &id("core:target"), None)
            .unwrap();
    }
    assert_eq!(world.debug_snapshot().transit.len(), 2);
    let launched = world.debug_snapshot();
    assert_eq!(
        launched
            .populations
            .tokens
            .values()
            .filter(|token| matches!(token.state, PopulationState::InTransit { .. }))
            .count(),
        2
    );
    let departure_habitats = launched
        .population_accounting
        .entries
        .iter()
        .filter_map(|entry| match &entry.transition {
            PopulationTransition::EnteredTransit {
                source_habitat_id, ..
            } => Some(source_habitat_id.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        departure_habitats,
        vec![id("core:z_habitat"), id("core:a_habitat")],
        "departure follows authored slot order, not lexical ID order"
    );
    for slot in ["core:z_habitat_slot", "core:a_habitat_slot"] {
        world
            .set_habitat_generation_enabled(
                &id("core:origin"),
                &id("core:origin_body"),
                &id(slot),
                false,
            )
            .unwrap();
    }

    world.advance_tick().unwrap();
    world.advance_tick().unwrap();
    let physical = world.debug_snapshot();
    assert!(physical.transit.is_empty());
    assert_eq!(physical.populations.tokens.len(), 1);
    assert_eq!(physical.population_accounting.removed, 1);
    let source = physical
        .systems
        .iter()
        .find(|system| system.location == id("core:origin"))
        .unwrap();
    assert_eq!(
        source
            .accounting
            .expedition_lost
            .quantity(&id("core:energy")),
        5
    );
    assert_eq!(
        source
            .accounting
            .ship_project_committed
            .quantity(&id("core:energy")),
        22
    );
    assert_eq!(
        source
            .accounting
            .construction_spent
            .quantity(&id("core:energy")),
        12
    );
    assert_eq!(
        source.accounting.travel_spent.quantity(&id("core:energy")),
        4
    );
    let target = physical
        .systems
        .iter()
        .find(|system| system.location == id("core:target"))
        .unwrap();
    assert_eq!(
        target
            .accounting
            .founding_received
            .quantity(&id("core:energy")),
        5
    );
    assert_eq!(
        target.bodies[0].slots[0]
            .development
            .as_ref()
            .map(|development| development.definition.role),
        Some(DevelopmentRole::Habitat)
    );
    assert_eq!(
        target.bodies[0].slots[1]
            .development
            .as_ref()
            .map(|development| development.definition.role),
        Some(DevelopmentRole::Collector),
        "unreserved settlement follows authored z-then-a slot order"
    );
    assert!(matches!(
        physical.knowledge.mission_state(&first.ship_id),
        Some(MissionState::AwaitingOutcome { .. })
    ));
    assert!(matches!(
        physical.knowledge.mission_state(&second.ship_id),
        Some(MissionState::AwaitingOutcome { .. })
    ));
    let player_before_outcomes = world.player_view().unwrap();
    assert!(matches!(
        player_before_outcomes.missions.get(&first.ship_id),
        Some(MissionState::AwaitingOutcome { .. })
    ));
    assert!(matches!(
        player_before_outcomes.missions.get(&second.ship_id),
        Some(MissionState::AwaitingOutcome { .. })
    ));
    assert!(
        player_before_outcomes
            .systems
            .iter()
            .find(|system| system.system == id("core:target"))
            .unwrap()
            .local_state
            .is_none()
    );
    assert_eq!(
        player_before_outcomes
            .systems
            .iter()
            .find(|system| system.system == id("core:origin"))
            .unwrap()
            .local_state
            .as_ref()
            .unwrap()
            .accounting
            .expedition_lost
            .quantity(&id("core:energy")),
        0,
        "physical loss accounting stays redacted until outcome receipt"
    );

    wait_for_mission_resolution(&mut world, &first.ship_id);
    assert!(matches!(
        world
            .debug_snapshot()
            .knowledge
            .mission_state(&first.ship_id),
        Some(MissionState::Founded { .. })
    ));
    assert!(matches!(
        world.debug_snapshot().knowledge.mission_state(&second.ship_id),
        Some(MissionState::FoundingLost {
            reason: FoundingLossReason::InsufficientSlots,
            population_id,
            founding_stocks,
            ..
        }) if population_id == &PopulationId::new(id("core:origin"), 1)
            && founding_stocks.quantity(&id("core:energy")) == 5
            && founding_stocks.quantity(&id("core:ore")) == 3
    ));
    assert_eq!(
        world
            .player_view()
            .unwrap()
            .systems
            .iter()
            .find(|system| system.system == id("core:origin"))
            .unwrap()
            .local_state
            .as_ref()
            .unwrap()
            .accounting
            .expedition_lost
            .quantity(&id("core:energy")),
        5
    );
}

#[test]
fn no_population_and_insufficient_launch_energy_reject_without_mutation() {
    let mut no_population = fixture(100, 0, 0, 2, 1, 1);
    for slot in ["core:z_habitat_slot", "core:a_habitat_slot"] {
        no_population
            .set_habitat_generation_enabled(
                &id("core:origin"),
                &id("core:origin_body"),
                &id(slot),
                false,
            )
            .unwrap();
    }
    let expedition = build_asset(
        &mut no_population,
        ShipProjectKind::Expedition,
        "core:yard_slot_a",
        1,
    );
    let before = no_population.debug_snapshot();
    assert_eq!(
        no_population.launch_expedition(
            &id("core:origin"),
            &expedition.ship_id,
            &id("core:target"),
            None,
        ),
        Err(CoreError::NoResidentPopulation(id("core:origin")))
    );
    assert_eq!(no_population.debug_snapshot(), before);

    // Commitment plus one progress step consumes all available Energy; launch
    // therefore rejects and leaves the completed asset and accounting unchanged.
    let mut no_launch_energy = fixture(5, 0, 0, 2, 1, 1);
    let probe = build_asset(
        &mut no_launch_energy,
        ShipProjectKind::Probe,
        "core:yard_slot_a",
        1,
    );
    let before = no_launch_energy.debug_snapshot();
    assert!(matches!(
        no_launch_energy.launch_probe(&id("core:origin"), &probe.ship_id, &id("core:mid"), 5,),
        Err(CoreError::InsufficientResource { .. })
    ));
    assert_eq!(no_launch_energy.debug_snapshot(), before);
}
