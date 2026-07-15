use super::*;
fn id(s: &str) -> ContentId {
    ContentId::new(s).unwrap()
}
fn physical_energy(snapshot: &CoreSnapshot) -> i128 {
    let markets = snapshot
        .markets
        .iter()
        .map(|market| i128::from(market.energy_stock.0))
        .sum::<i128>();
    let tanks = snapshot
        .traders
        .iter()
        .map(|trader| i128::from(trader.energy_tank.0))
        .sum::<i128>();
    let bulk = snapshot
        .traders
        .iter()
        .map(|trader| {
            i128::from(trader.bulk_energy.owned.0)
                + trader
                    .bulk_energy
                    .locked
                    .map_or(0, |lot| i128::from(lot.amount.0))
        })
        .sum::<i128>();
    markets + tanks + bulk
}
fn assert_physical_delta_reconciles(before: &CoreSnapshot, after: &CoreSnapshot) {
    let physical_delta = physical_energy(after) - physical_energy(before);
    let flow_delta = i128::from(after.energy_flow.net_external_delta().0)
        - i128::from(before.energy_flow.net_external_delta().0);
    assert_eq!(physical_delta, flow_delta);
}

fn definition() -> GameDefinition {
    let energy = id(ENERGY_ID);
    let ore = id("core:ore");
    GameDefinition {
        goods: vec![
            GoodDefinition {
                id: energy.clone(),
                name: "Energy".into(),
                category: GoodCategory::Energy,
                bootstrap_cost: Energy(1),
            },
            GoodDefinition {
                id: ore.clone(),
                name: "Ore".into(),
                category: GoodCategory::Raw,
                bootstrap_cost: Energy(3),
            },
        ],
        recipes: vec![],
        systems: (0..2)
            .map(|i| SystemDefinition {
                id: id(&format!("core:s{i}")),
                name: format!("S{i}"),
                position: Position3 {
                    x: f64::from(i) * 10.0,
                    y: 0.0,
                    z: 0.0,
                },
                inventory: BTreeMap::from([
                    (energy.clone(), 1000),
                    (ore.clone(), if i == 0 { 100 } else { 0 }),
                ]),
                targets: BTreeMap::from([(ore.clone(), 10), (energy.clone(), 100)]),
                recipes: vec![],
                sources: vec![],
                energy_output_per_tick: Energy(10),
                seasonal_generation: SeasonalGenerationState {
                    base_output: Energy(10),
                    amplitude_percent: 0,
                    period_ticks: 100,
                    phase_ticks: 0,
                    current_effective_output: Energy(10),
                },
                energy_storage_cap: Energy(2000),
                population: 1,
                population_state: PopulationState {
                    current: 1,
                    reference: 1,
                    carrying_capacity: 1,
                    ..PopulationState::default()
                },
                investment_policy: InvestmentPolicy::default(),
                governance: Governance {
                    authority: MarketAuthority::Player(id("core:player")),
                },
                policy: MarketPolicy::default(),
                energy_logistics: EnergyLogisticsPolicy::default(),
                protected_liquidation_budget: Energy(20),
                bootstrap_risk_acknowledged: false,
            })
            .collect(),
        traders: vec![TraderDefinition {
            id: id("core:player"),
            name: "Player".into(),
            system: id("core:s0"),
            archetype: None,
            energy_tank: Energy(100),
            energy_tank_capacity: Energy(1000),
            bulk_energy_capacity: Energy::ZERO,
            cargo_capacity: 20,
            speed: 10.0,
            travel_burn_per_distance: Energy(1),
            refuel_policy: RefuelPolicy::DepositAndWithdraw,
            player: true,
        }],
        player_trade_network_access: TradeNetworkAccess::Offline,
        fleet: FleetDynamics {
            mode: Some(FleetMode::Fixed { count: 0 }),
            ..FleetDynamics::default()
        },
        economy: EconomyConfig::default(),
    }
}
fn local_energy_contract_definition() -> GameDefinition {
    let mut definition = definition();
    let energy = id(ENERGY_ID);
    definition.systems[0]
        .inventory
        .insert(energy.clone(), 5_000);
    definition.systems[0].energy_storage_cap = Energy(5_000);
    definition.systems[0].energy_logistics.authored_export_base = Energy(500);
    definition.systems[1].inventory.insert(energy.clone(), 100);
    definition.systems[1].energy_storage_cap = Energy(5_000);
    definition.systems[1].targets.insert(energy, 1_000);
    definition.traders[0].energy_tank = Energy(1_000);
    definition.traders[0].energy_tank_capacity = Energy(1_500);
    definition.traders[0].bulk_energy_capacity = Energy(1_000);
    definition
}

fn three_loaded_energy_contract_session(permutation: [&str; 3]) -> GameSession {
    let mut configured = local_energy_contract_definition();
    for trader_id in permutation {
        configured.traders.push(TraderDefinition {
            id: id(trader_id),
            name: trader_id.into(),
            system: id("core:s0"),
            archetype: None,
            energy_tank: Energy(1_000),
            energy_tank_capacity: Energy(1_500),
            bulk_energy_capacity: Energy(1_000),
            cargo_capacity: 20,
            speed: 10.0,
            travel_burn_per_distance: Energy(1),
            refuel_policy: RefuelPolicy::DepositAndWithdraw,
            player: false,
        });
    }
    let mut session = GameSession::new(configured).unwrap();
    for trader_id in permutation {
        session
            .world
            .resource_mut::<PendingEnergyContractIntents>()
            .0
            .push(EnergyContractIntent {
                carrier: id(trader_id),
                source: id("core:s0"),
                destination: id("core:s1"),
                gross_payload: Energy(300),
                command_driven: false,
            });
    }
    session.resolve_pending_energy_contract_intents().unwrap();
    assert_eq!(session.world.resource::<EnergyContracts>().active.len(), 3);
    session
}

fn remote_energy_contract_definition() -> GameDefinition {
    let mut definition = local_energy_contract_definition();
    let energy = id(ENERGY_ID);
    definition.systems[0].inventory.insert(energy.clone(), 100);
    definition.systems[0].targets.insert(energy.clone(), 4_000);
    definition.systems[1].inventory.insert(energy, 5_000);
    definition.systems[1].energy_logistics.authored_export_base = Energy(3_200);
    definition.traders[0].bulk_energy_capacity = Energy(4_000);
    definition
}

fn dynamic_fleet(
    initial_count: usize,
    maximum_count: usize,
    opportunity_window: u32,
    retirement_window: u32,
) -> FleetDynamics {
    let archetype = FleetArchetype {
        id: id("core:archetype"),
        id_prefix: "core:trader".into(),
        name_prefix: "Trader".into(),
        initial_count,
        maximum_count,
        starting_tank: Energy(100),
        energy_tank_capacity: Energy(500),
        bulk_energy_capacity: Energy::ZERO,
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
    };
    FleetDynamics {
        mode: Some(FleetMode::Dynamic {
            initial_count,
            opportunity_threshold: 1,
            opportunity_window,
            spawn_cooldown_ticks: 3,
            retirement_window,
            retirement_threshold: 0,
            maximum_count,
        }),
        archetypes: BTreeMap::from([(archetype.id.clone(), archetype)]),
        ..FleetDynamics::default()
    }
}

fn single_spawn_dynamic_fleet(archetype: FleetArchetype) -> FleetDynamics {
    FleetDynamics {
        mode: Some(FleetMode::Dynamic {
            initial_count: 0,
            opportunity_threshold: 1,
            opportunity_window: 1,
            spawn_cooldown_ticks: 10,
            retirement_window: 10,
            retirement_threshold: -1,
            maximum_count: 1,
        }),
        archetypes: BTreeMap::from([(archetype.id.clone(), archetype)]),
        ..FleetDynamics::default()
    }
}

fn test_bulk_hauler() -> FleetArchetype {
    FleetArchetype {
        id: id("core:hauler"),
        id_prefix: "core:hauler".into(),
        name_prefix: "Hauler".into(),
        initial_count: 0,
        maximum_count: 1,
        starting_tank: Energy(1_000),
        energy_tank_capacity: Energy(1_500),
        bulk_energy_capacity: Energy(1_000),
        cargo_capacity: 1,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
    }
}

#[test]
fn fixed_fleet_mode_is_a_strict_lifecycle_bypass() {
    let mut session = GameSession::new(definition()).unwrap();
    for _ in 0..100 {
        session.step().unwrap();
    }
    let events = session.drain_events();
    let snapshot = session.snapshot();
    assert_eq!(
        snapshot
            .traders
            .iter()
            .filter(|trader| !trader.player)
            .count(),
        0
    );
    assert_eq!(snapshot.fleet.opportunity_persistence, 0);
    assert!(!events.iter().any(|event| matches!(
        event,
        GameEvent::TraderSpawned { .. } | GameEvent::TraderRetired { .. }
    )));
}

#[test]
fn unserved_energy_demand_spawns_a_bulk_capable_archetype_at_its_source() {
    let mut configured = local_energy_contract_definition();
    configured.systems[0].inventory.insert(id("core:ore"), 0);
    configured.systems[1].inventory.insert(id("core:ore"), 0);
    configured.fleet = FleetDynamics {
        mode: Some(FleetMode::Dynamic {
            initial_count: 1,
            opportunity_threshold: 1,
            opportunity_window: 1,
            spawn_cooldown_ticks: 10,
            retirement_window: 10,
            retirement_threshold: -1,
            maximum_count: 2,
        }),
        archetypes: BTreeMap::from([
            (
                id("core:a_general"),
                FleetArchetype {
                    id: id("core:a_general"),
                    id_prefix: "core:general".into(),
                    name_prefix: "General".into(),
                    initial_count: 1,
                    maximum_count: 1,
                    starting_tank: Energy(1_000),
                    energy_tank_capacity: Energy(1_500),
                    bulk_energy_capacity: Energy::ZERO,
                    cargo_capacity: 20,
                    speed: 10.0,
                    travel_burn_per_distance: Energy(1),
                    refuel_policy: RefuelPolicy::DepositAndWithdraw,
                },
            ),
            (
                id("core:z_hauler"),
                FleetArchetype {
                    id: id("core:z_hauler"),
                    id_prefix: "core:hauler".into(),
                    name_prefix: "Hauler".into(),
                    initial_count: 0,
                    maximum_count: 1,
                    starting_tank: Energy(1_000),
                    energy_tank_capacity: Energy(1_500),
                    bulk_energy_capacity: Energy(1_000),
                    cargo_capacity: 1,
                    speed: 10.0,
                    travel_burn_per_distance: Energy(1),
                    refuel_policy: RefuelPolicy::DepositAndWithdraw,
                },
            ),
        ]),
        ..FleetDynamics::default()
    };
    configured.traders.push(TraderDefinition {
        id: id("core:idle_general"),
        name: "Idle General".into(),
        system: id("core:s0"),
        archetype: Some(id("core:a_general")),
        energy_tank: Energy(1_000),
        energy_tank_capacity: Energy(1_500),
        bulk_energy_capacity: Energy::ZERO,
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut session = GameSession::new(configured).unwrap();

    session.collect_automated_trader_requests().unwrap();
    assert!(
        session
            .world
            .resource::<FleetDynamics>()
            .normalized_unserved_opportunity
            > 0
    );
    session.evaluate_dynamic_fleet().unwrap();

    let spawned = session
        .snapshot()
        .traders
        .into_iter()
        .find(|trader| trader.archetype == Some(id("core:z_hauler")))
        .unwrap();
    assert_eq!(spawned.archetype, Some(id("core:z_hauler")));
    assert_eq!(spawned.system, id("core:s0"));
}

#[test]
fn unserved_ordinary_demand_spawns_the_higher_scoring_general_archetype() {
    let mut configured = definition();
    configured.systems[1].targets.insert(id("core:ore"), 1_000);
    configured.fleet = FleetDynamics {
        mode: Some(FleetMode::Dynamic {
            initial_count: 0,
            opportunity_threshold: 1,
            opportunity_window: 1,
            spawn_cooldown_ticks: 10,
            retirement_window: 10,
            retirement_threshold: -1,
            maximum_count: 1,
        }),
        archetypes: BTreeMap::from([
            (
                id("core:a_hauler"),
                FleetArchetype {
                    id: id("core:a_hauler"),
                    id_prefix: "core:hauler".into(),
                    name_prefix: "Hauler".into(),
                    initial_count: 0,
                    maximum_count: 1,
                    starting_tank: Energy(100),
                    energy_tank_capacity: Energy(500),
                    bulk_energy_capacity: Energy(1_000),
                    cargo_capacity: 1,
                    speed: 10.0,
                    travel_burn_per_distance: Energy(1),
                    refuel_policy: RefuelPolicy::DepositAndWithdraw,
                },
            ),
            (
                id("core:z_general"),
                FleetArchetype {
                    id: id("core:z_general"),
                    id_prefix: "core:general".into(),
                    name_prefix: "General".into(),
                    initial_count: 0,
                    maximum_count: 1,
                    starting_tank: Energy(100),
                    energy_tank_capacity: Energy(500),
                    bulk_energy_capacity: Energy::ZERO,
                    cargo_capacity: 20,
                    speed: 10.0,
                    travel_burn_per_distance: Energy(1),
                    refuel_policy: RefuelPolicy::DepositAndWithdraw,
                },
            ),
        ]),
        ..FleetDynamics::default()
    };
    let mut session = GameSession::new(configured).unwrap();

    session.collect_automated_trader_requests().unwrap();
    assert!(
        session
            .world
            .resource::<FleetDynamics>()
            .normalized_unserved_opportunity
            > 0
    );
    session.evaluate_dynamic_fleet().unwrap();

    let spawned = session
        .snapshot()
        .traders
        .into_iter()
        .find(|trader| !trader.player)
        .unwrap();
    assert_eq!(spawned.archetype, Some(id("core:z_general")));
    assert_eq!(spawned.system, id("core:s0"));
}

#[test]
fn hypothetical_energy_spawn_reserves_starting_tank_before_scoring_payload() {
    let mut configured = local_energy_contract_definition();
    configured.systems[0].inventory.insert(id(ENERGY_ID), 1_100);
    configured.systems[0].inventory.insert(id("core:ore"), 0);
    configured.systems[1].inventory.insert(id("core:ore"), 0);
    configured.fleet = single_spawn_dynamic_fleet(test_bulk_hauler());
    let mut session = GameSession::new(configured).unwrap();
    let before = session.snapshot();

    session.collect_automated_trader_requests().unwrap();
    assert_eq!(
        session
            .world
            .resource::<FleetDynamics>()
            .normalized_unserved_opportunity,
        0
    );
    session.evaluate_dynamic_fleet().unwrap();

    let after = session.snapshot();
    assert_eq!(
        after.traders.iter().filter(|trader| !trader.player).count(),
        0
    );
    assert_eq!(
        after.markets[0].energy_stock,
        before.markets[0].energy_stock
    );
    assert_eq!(after.dynamics_history.fleet_spawns, 0);
}

#[test]
fn stale_captured_energy_opportunity_does_not_spawn() {
    let mut configured = local_energy_contract_definition();
    configured.systems[0].inventory.insert(id("core:ore"), 0);
    configured.systems[1].inventory.insert(id("core:ore"), 0);
    configured.fleet = single_spawn_dynamic_fleet(test_bulk_hauler());
    let mut session = GameSession::new(configured).unwrap();
    session.collect_automated_trader_requests().unwrap();
    assert!(
        session
            .world
            .resource::<FleetDynamics>()
            .normalized_unserved_opportunity
            > 0
    );
    let source_before = session.snapshot().markets[0].energy_stock;
    let destination = session.market_entity(&id("core:s1")).unwrap();
    session
        .world
        .get_mut::<Market>(destination)
        .unwrap()
        .set_energy_stock(Energy(1_000))
        .unwrap();

    session.evaluate_dynamic_fleet().unwrap();

    let after = session.snapshot();
    assert_eq!(
        after.traders.iter().filter(|trader| !trader.player).count(),
        0
    );
    assert_eq!(after.markets[0].energy_stock, source_before);
    assert_eq!(after.dynamics_history.fleet_spawns, 0);
}

#[test]
fn stale_captured_ordinary_opportunity_does_not_spawn() {
    let mut configured = definition();
    configured.systems[1].targets.insert(id("core:ore"), 1_000);
    configured.fleet = single_spawn_dynamic_fleet(FleetArchetype {
        id: id("core:general"),
        id_prefix: "core:general".into(),
        name_prefix: "General".into(),
        initial_count: 0,
        maximum_count: 1,
        starting_tank: Energy(100),
        energy_tank_capacity: Energy(500),
        bulk_energy_capacity: Energy::ZERO,
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
    });
    let mut session = GameSession::new(configured).unwrap();
    session.collect_automated_trader_requests().unwrap();
    assert!(
        session
            .world
            .resource::<FleetDynamics>()
            .normalized_unserved_opportunity
            > 0
    );
    let source = session.market_entity(&id("core:s0")).unwrap();
    session
        .world
        .get_mut::<Market>(source)
        .unwrap()
        .inventory
        .insert(id("core:ore"), 0);
    let source_energy = session.snapshot().markets[0].energy_stock;

    session.evaluate_dynamic_fleet().unwrap();

    let after = session.snapshot();
    assert_eq!(
        after.traders.iter().filter(|trader| !trader.player).count(),
        0
    );
    assert_eq!(after.markets[0].energy_stock, source_energy);
    assert_eq!(after.dynamics_history.fleet_spawns, 0);
}

#[test]
fn dynamic_generated_namespace_collision_is_rejected_at_startup_and_atomic_at_runtime() {
    let mut invalid = definition();
    invalid.fleet = dynamic_fleet(1, 2, 1, 100);
    invalid.traders.push(TraderDefinition {
        id: id("core:trader_dynamic_00000001"),
        name: "Collision".into(),
        system: id("core:s0"),
        archetype: None,
        energy_tank: Energy(100),
        energy_tank_capacity: Energy(500),
        bulk_energy_capacity: Energy::ZERO,
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    assert!(matches!(
        GameSession::new(invalid),
        Err(CoreError::InvalidWorldDynamics)
    ));

    // Defense in depth: even an impossible post-startup collision leaves
    // the complete tick-visible state and event stream untouched.
    let mut runtime = definition();
    runtime.fleet = dynamic_fleet(0, 2, 1, 100);
    let mut session = GameSession::new(runtime).unwrap();
    session
        .world
        .spawn(StableId(id("core:trader_dynamic_00000001")));
    let before = session.snapshot();
    assert_eq!(
        session.spawn_dynamic_trader(),
        Err(CoreError::InvalidPhysicalDefinition)
    );
    assert_eq!(session.snapshot(), before);
    assert!(session.drain_events().is_empty());
}

#[test]
fn fixed_laden_liquidation_failures_preserve_all_lifecycle_state() {
    let mut fixed = definition();
    fixed.fleet.mode = Some(FleetMode::Fixed { count: 1 });
    fixed.traders.push(TraderDefinition {
        id: id("core:trader_01"),
        name: "Trader 01".into(),
        system: id("core:s0"),
        archetype: None,
        energy_tank: Energy::ZERO,
        energy_tank_capacity: Energy(500),
        bulk_energy_capacity: Energy::ZERO,
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    for system in &mut fixed.systems {
        system.inventory.insert(id(ENERGY_ID), 0);
        system.protected_liquidation_budget = Energy::ZERO;
    }
    let mut session = GameSession::new(fixed).unwrap();
    let npc = session
        .world
        .query_filtered::<(Entity, &StableId), (With<Trader>, Without<PlayerControlled>)>()
        .iter(&session.world)
        .find(|(_, stable)| stable.0 == id("core:trader_01"))
        .unwrap()
        .0;
    {
        let mut trader = session.world.get_mut::<Trader>(npc).unwrap();
        trader.cargo.insert(id("core:ore"), 1);
        trader.cargo_cost_basis.insert(
            id("core:ore"),
            CostBasis {
                stock_quantity: 1,
                total_embodied_energy: Energy(3),
            },
        );
    }
    let fleet_before = session.world.resource::<FleetDynamics>().clone();
    let history_before = session.world.resource::<AggregateDynamicsHistory>().clone();
    let lifecycle_before = session.world.get::<TraderLifecycle>(npc).unwrap().clone();

    session.settle_idle_laden().unwrap();
    session.evaluate_dynamic_fleet().unwrap();

    assert_eq!(*session.world.resource::<FleetDynamics>(), fleet_before);
    assert_eq!(
        *session.world.resource::<AggregateDynamicsHistory>(),
        history_before
    );
    assert_eq!(
        *session.world.get::<TraderLifecycle>(npc).unwrap(),
        lifecycle_before
    );
    let events = session.drain_events();
    assert!(!events.is_empty());
    assert!(events.iter().all(|event| matches!(
        event,
        GameEvent::SaleDeferred { trader, .. } if trader == &id("core:trader_01")
    )));
}

#[test]
fn dynamic_spawn_is_persistent_funded_stable_and_next_tick_eligible() {
    let mut definition = definition();
    definition.fleet = dynamic_fleet(0, 1, 2, 100);
    let initial_energy = definition
        .systems
        .iter()
        .map(|system| i128::from(system.inventory[&id(ENERGY_ID)]))
        .sum::<i128>()
        + i128::from(definition.traders[0].energy_tank.0);
    let mut session = GameSession::new(definition).unwrap();
    session.step().unwrap();
    assert_eq!(session.snapshot().traders.len(), 1);
    session.drain_events();
    session.step().unwrap();
    let events = session.drain_events();
    let snapshot = session.snapshot();
    let spawned = snapshot
        .traders
        .iter()
        .find(|trader| !trader.player)
        .unwrap();
    assert_eq!(spawned.id, id("core:trader_dynamic_00000001"));
    assert_eq!(spawned.system, id("core:s0"));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::TraderSpawned { trader, system }
            if trader == &id("core:trader_dynamic_00000001") && system == &id("core:s0")
    )));
    assert!(!events.iter().any(|event| matches!(
        event,
        GameEvent::Bought { trader, .. }
            if trader == &id("core:trader_dynamic_00000001")
    )));
    let expected = initial_energy + i128::from(snapshot.energy_flow.net_external_delta().0);
    assert_eq!(physical_energy(&snapshot), expected);
    session.step().unwrap();
    assert_eq!(
        session
            .snapshot()
            .traders
            .iter()
            .filter(|trader| !trader.player)
            .count(),
        1
    );
}

#[test]
fn dynamic_spawn_obeys_cooldown_and_monotonic_ids() {
    let mut definition = definition();
    for index in 2..4 {
        let mut system = definition.systems[1].clone();
        system.id = id(&format!("core:s{index}"));
        system.name = format!("S{index}");
        system.position.x = f64::from(index) * 10.0;
        definition.systems.push(system);
    }
    definition.fleet = dynamic_fleet(0, 3, 1, 100);
    let mut session = GameSession::new(definition).unwrap();
    session.step().unwrap();
    assert_eq!(
        session
            .snapshot()
            .traders
            .iter()
            .filter(|trader| !trader.player)
            .count(),
        1
    );
    session.step().unwrap();
    session.step().unwrap();
    assert_eq!(
        session
            .snapshot()
            .traders
            .iter()
            .filter(|trader| !trader.player)
            .count(),
        1
    );
    session.step().unwrap();
    let snapshot = session.snapshot();
    assert_eq!(
        snapshot
            .traders
            .iter()
            .filter(|trader| !trader.player)
            .count(),
        2
    );
    assert!(
        snapshot
            .traders
            .iter()
            .any(|trader| trader.id == id("core:trader_dynamic_00000002"))
    );
}

#[test]
fn dynamic_spawn_defers_without_safe_market_funding() {
    let mut definition = definition();
    let mut fleet = dynamic_fleet(0, 1, 1, 100);
    let archetype = fleet.archetypes.values_mut().next().unwrap();
    archetype.starting_tank = Energy(10_000);
    archetype.energy_tank_capacity = Energy(10_000);
    definition.fleet = fleet;
    let mut session = GameSession::new(definition).unwrap();
    for _ in 0..5 {
        session.step().unwrap();
    }
    assert_eq!(session.snapshot().traders.len(), 1);
    assert_eq!(session.snapshot().fleet.spawn_sequence, 0);
}

#[test]
fn dynamic_opportunity_scoring_skips_emergency_suppressed_demand() {
    let mut definition = definition();
    definition.fleet = dynamic_fleet(0, 1, 1, 100);
    let mut session = GameSession::new(definition).unwrap();
    let destination = session.market_entity(&id("core:s1")).unwrap();
    session
        .world
        .get_mut::<Market>(destination)
        .unwrap()
        .operating_profile
        .stage = BrownoutStage::Emergency;

    session.collect_automated_trader_requests().unwrap();

    assert_eq!(
        session
            .world
            .resource::<FleetDynamics>()
            .normalized_unserved_opportunity,
        0
    );
    assert!(
        session
            .world
            .resource::<PendingTradeRequests>()
            .0
            .is_empty()
    );
}

#[test]
fn subsidized_dynamic_opportunity_requires_canonical_destination_funding() {
    let mut definition = definition();
    enable_investments(&mut definition);
    definition.fleet = dynamic_fleet(0, 1, 1, 100);
    let mut session = GameSession::new(definition).unwrap();
    let destination = session.market_entity(&id("core:s1")).unwrap();
    {
        let mut market = session.world.get_mut::<Market>(destination).unwrap();
        market
            .investment_state
            .levels
            .insert(InvestmentKind::RouteSubsidy, 1);
        let protected = market.protected_liquidation_budget;
        market.reserved_energy = Energy(10);
        market
            .set_energy_stock(protected.checked_add(Energy(10)).unwrap())
            .unwrap();
    }

    session.collect_automated_trader_requests().unwrap();
    assert_eq!(
        session
            .world
            .resource::<FleetDynamics>()
            .normalized_unserved_opportunity,
        0
    );
    session.evaluate_dynamic_fleet().unwrap();
    assert_eq!(
        session
            .world
            .query_filtered::<Entity, (With<Trader>, Without<PlayerControlled>)>()
            .iter(&session.world)
            .count(),
        0,
        "an advertised but zero-funded subsidy must not spawn a donor"
    );

    {
        let mut market = session.world.get_mut::<Market>(destination).unwrap();
        market.reserved_energy = Energy::ZERO;
        market.set_energy_stock(Energy(1_000)).unwrap();
    }
    session.collect_automated_trader_requests().unwrap();
    assert!(
        session
            .world
            .resource::<FleetDynamics>()
            .normalized_unserved_opportunity
            > 0
    );
    session.evaluate_dynamic_fleet().unwrap();
    assert_eq!(
        session
            .world
            .query_filtered::<Entity, (With<Trader>, Without<PlayerControlled>)>()
            .iter(&session.world)
            .count(),
        1,
        "restored destination funding makes the opportunity spawnable"
    );
}

#[test]
fn dynamic_spawn_overflows_are_atomic_and_retry_uses_unique_monotonic_ids() {
    let mut dynamic = definition();
    dynamic.fleet = dynamic_fleet(0, 3, 1, 100);
    let mut session = GameSession::new(dynamic).unwrap();

    session.world.resource_mut::<FleetDynamics>().spawn_sequence = u64::MAX;
    let before_sequence_overflow = session.snapshot();
    assert_eq!(session.spawn_dynamic_trader(), Err(CoreError::Overflow));
    assert_eq!(session.snapshot(), before_sequence_overflow);
    assert!(session.drain_events().is_empty());

    session.world.resource_mut::<FleetDynamics>().spawn_sequence = 0;
    session
        .world
        .resource_mut::<AggregateDynamicsHistory>()
        .fleet_spawns = u64::MAX;
    let before_counter_overflow = session.snapshot();
    let energy_before = physical_energy(&before_counter_overflow);
    assert_eq!(session.spawn_dynamic_trader(), Err(CoreError::Overflow));
    assert_eq!(session.snapshot(), before_counter_overflow);
    assert_eq!(physical_energy(&session.snapshot()), energy_before);
    assert!(session.drain_events().is_empty());

    session
        .world
        .resource_mut::<AggregateDynamicsHistory>()
        .fleet_spawns = 0;
    assert_eq!(session.spawn_dynamic_trader(), Ok(true));
    assert_eq!(session.spawn_dynamic_trader(), Ok(true));
    let snapshot = session.snapshot();
    let ids = snapshot
        .traders
        .iter()
        .filter(|trader| !trader.player)
        .map(|trader| trader.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            id("core:trader_dynamic_00000001"),
            id("core:trader_dynamic_00000002")
        ]
    );
    assert_eq!(snapshot.fleet.spawn_sequence, 2);
    assert_eq!(snapshot.dynamics_history.fleet_spawns, 2);
    assert_eq!(physical_energy(&snapshot), energy_before);
    assert_eq!(
        session
            .drain_events()
            .iter()
            .filter(|event| matches!(event, GameEvent::TraderSpawned { .. }))
            .count(),
        2
    );
}

#[test]
fn dynamic_retirement_counter_overflow_is_atomic_and_retry_returns_tank_once() {
    let mut dynamic = definition();
    dynamic.fleet = dynamic_fleet(1, 1, 100, 100);
    dynamic.traders.push(TraderDefinition {
        id: id("core:trader_01"),
        name: "Trader 01".into(),
        system: id("core:s0"),
        archetype: Some(id("core:archetype")),
        energy_tank: Energy(100),
        energy_tank_capacity: Energy(500),
        bulk_energy_capacity: Energy::ZERO,
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut session = GameSession::new(dynamic).unwrap();
    let npc = session
        .world
        .query_filtered::<(Entity, &StableId), (With<Trader>, Without<PlayerControlled>)>()
        .iter(&session.world)
        .find(|(_, stable)| stable.0 == id("core:trader_01"))
        .unwrap()
        .0;
    session
        .world
        .get_mut::<TraderLifecycle>(npc)
        .unwrap()
        .retirement = Some(TraderRetirementState::CleaningUp);
    session
        .world
        .resource_mut::<AggregateDynamicsHistory>()
        .fleet_retirements = u64::MAX;
    let before = session.snapshot();
    let energy_before = physical_energy(&before);

    assert_eq!(
        session.finish_deferred_retirements(),
        Err(CoreError::Overflow)
    );
    assert_eq!(session.snapshot(), before);
    assert_eq!(physical_energy(&session.snapshot()), energy_before);
    assert!(session.drain_events().is_empty());

    session
        .world
        .resource_mut::<AggregateDynamicsHistory>()
        .fleet_retirements = 0;
    session.finish_deferred_retirements().unwrap();
    let retired = session.snapshot();
    assert_eq!(retired.dynamics_history.fleet_retirements, 1);
    assert!(retired.traders.iter().all(|trader| trader.player));
    assert_eq!(physical_energy(&retired), energy_before);
    assert!(matches!(
        session.drain_events().as_slice(),
        [GameEvent::TraderRetired { trader, .. }] if trader == &id("core:trader_01")
    ));
    session.finish_deferred_retirements().unwrap();
    assert_eq!(session.snapshot(), retired);
    assert!(session.drain_events().is_empty());
}

#[test]
fn empty_unprofitable_dynamic_trader_returns_tank_before_retiring() {
    let mut definition = definition();
    definition.fleet = dynamic_fleet(1, 1, 100, 2);
    for system in &mut definition.systems {
        system.targets.insert(id("core:ore"), 0);
        system.inventory.insert(id("core:ore"), 0);
    }
    definition.traders.push(TraderDefinition {
        id: id("core:trader_01"),
        name: "Trader 01".into(),
        system: id("core:s0"),
        archetype: Some(id("core:archetype")),
        energy_tank: Energy(100),
        energy_tank_capacity: Energy(500),
        bulk_energy_capacity: Energy::ZERO,
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut session = GameSession::new(definition).unwrap();
    session.step().unwrap();
    session.drain_events();
    session.step().unwrap();
    let events = session.drain_events();
    let snapshot = session.snapshot();
    assert_eq!(
        snapshot
            .traders
            .iter()
            .filter(|trader| !trader.player)
            .count(),
        0
    );
    assert_eq!(snapshot.dynamics_history.fleet_retirements, 1);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::TraderRetired { trader, .. } if trader == &id("core:trader_01")
    )));
    assert_eq!(
        snapshot
            .reservations
            .iter()
            .filter(|r| r.status == ReservationStatus::Active)
            .count(),
        0
    );
}

#[test]
fn laden_sustained_unprofitable_trader_uses_anti_strand_cleanup_and_retires() {
    let mut definition = definition();
    definition.fleet = dynamic_fleet(1, 1, 100, 2);
    definition.traders.push(TraderDefinition {
        id: id("core:trader_01"),
        name: "Trader 01".into(),
        system: id("core:s0"),
        archetype: Some(id("core:archetype")),
        energy_tank: Energy(40),
        energy_tank_capacity: Energy(1_000),
        bulk_energy_capacity: Energy::ZERO,
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut session = GameSession::new(definition).unwrap();
    let npc = session
        .world
        .query_filtered::<(Entity, &StableId), (With<Trader>, Without<PlayerControlled>)>()
        .iter(&session.world)
        .find(|(_, stable)| stable.0 == id("core:trader_01"))
        .unwrap()
        .0;
    session
        .commit_and_depart(npc, &id("core:s1"), &id("core:ore"), 5)
        .unwrap();
    // Remove ordinary purchasing power after the valid commitment. Once
    // cleanup cancels the reservation, only Slice 1's protected liquidation
    // payout can fund this low-tank trader's adjacent jump.
    let destination = session.market_entity(&id("core:s1")).unwrap();
    session
        .world
        .get_mut::<Market>(destination)
        .unwrap()
        .set_energy_stock(Energy::ZERO)
        .unwrap();
    assert_eq!(
        session
            .world
            .get::<TraderLifecycle>(npc)
            .unwrap()
            .retirement,
        None,
        "retirement must be triggered by sustained unprofitability"
    );
    let initial = session.snapshot();
    let initial_energy = physical_energy(&initial);
    let initial_flow = i128::from(initial.energy_flow.net_external_delta().0);
    let initial_ore = initial
        .markets
        .iter()
        .map(|market| market.inventory[&id("core:ore")])
        .sum::<u64>()
        + initial
            .traders
            .iter()
            .flat_map(|trader| trader.cargo.get(&id("core:ore")))
            .sum::<u64>();

    let retirement_window = 2_usize;
    let destination_storage_cap = session
        .world
        .get::<Market>(destination)
        .unwrap()
        .energy_storage_cap;
    let mut blocked_tank_return = false;
    let mut releases = 0;
    let mut liquidation_sale = false;
    let mut observed_profitability_retirement = false;
    let mut was_retiring = false;
    let mut retired = false;
    for _ in 0..20 {
        session.step().unwrap();
        let events = session.drain_events();
        releases += events
            .iter()
            .filter(|event| matches!(event, GameEvent::ReservationReleased { .. }))
            .count();
        liquidation_sale |= events.iter().any(|event| {
            matches!(
                event,
                GameEvent::Sold { trader, .. } if trader == &id("core:trader_01")
            )
        });
        let snapshot = session.snapshot();
        if let Some(trader) = snapshot
            .traders
            .iter()
            .find(|trader| trader.id == id("core:trader_01"))
        {
            let is_retiring = trader.retirement.is_some();
            if is_retiring && !was_retiring {
                assert_eq!(trader.profitability_window.len(), retirement_window);
                assert!(
                    trader.profitability_window.iter().sum::<i64>() <= 0,
                    "profitability retirement transitioned without a sustained loss"
                );
                assert!(
                    usize::try_from(trader.failed_liquidation_ticks).unwrap() < retirement_window,
                    "failed-liquidation trigger, not profitability, caused retirement"
                );
                observed_profitability_retirement = true;
                session
                    .world
                    .get_mut::<Market>(destination)
                    .unwrap()
                    .energy_storage_cap = destination_storage_cap;
            } else if trader.profitability_window.len() == retirement_window - 1
                && !blocked_tank_return
            {
                // Keep the retiring entity observable for one snapshot by
                // temporarily filling local tank-return headroom. Restoring
                // the cap immediately after the transition preserves the
                // bounded cleanup and conservation checks below.
                let mut market = session.world.get_mut::<Market>(destination).unwrap();
                market.energy_storage_cap = market.energy_stock().unwrap();
                blocked_tank_return = true;
            }
            was_retiring = is_retiring;
        }
        assert!(!snapshot.traders.iter().any(|trader| {
            !trader.player
                && trader.travel.is_none()
                && !trader.cargo.is_empty()
                && trader.retirement.is_some()
        }));
        if snapshot.traders.iter().all(|trader| trader.player) {
            retired = true;
            break;
        }
    }
    assert!(retired, "laden retirement did not finish within 20 ticks");
    assert!(
        observed_profitability_retirement,
        "the profitability-triggered retirement transition was not observed"
    );
    assert_eq!(releases, 1);
    assert!(
        liquidation_sale,
        "cleanup never used the funded liquidation sale path"
    );
    let final_snapshot = session.snapshot();
    let final_ore = final_snapshot
        .markets
        .iter()
        .map(|market| market.inventory[&id("core:ore")])
        .sum::<u64>()
        + final_snapshot
            .traders
            .iter()
            .flat_map(|trader| trader.cargo.get(&id("core:ore")))
            .sum::<u64>();
    assert_eq!(final_ore, initial_ore);
    assert_eq!(
        physical_energy(&final_snapshot),
        initial_energy + i128::from(final_snapshot.energy_flow.net_external_delta().0)
            - initial_flow
    );
}

#[test]
fn physical_tick_generates_caps_burns_and_reports_deficit() {
    let mut d = definition();
    d.systems[0].inventory.insert(id(ENERGY_ID), 1999);
    d.systems[0].population = 3;
    let mut s = GameSession::new(d).unwrap();
    s.step().unwrap();
    let m = &s.snapshot().markets[0];
    assert_eq!(m.energy_stock, Energy(1997));
    assert_eq!(m.energy_flow.generated, Energy(10));
    assert_eq!(m.energy_flow.curtailed, Energy(9));
    assert_eq!(m.energy_flow.life_support_burned, Energy(3));
}
#[test]
fn funded_quantity_keeps_reserves_independent() {
    assert_eq!(
        funded_quantity(
            30,
            Energy(400),
            Energy(87),
            Energy(50),
            Energy(20),
            Energy(13)
        )
        .unwrap(),
        18
    );
    assert_eq!(
        funded_quantity(
            30,
            Energy(400),
            Energy(87),
            Energy(100),
            Energy(20),
            Energy(13)
        )
        .unwrap(),
        14
    );
}
#[test]
fn cost_basis_and_weighted_allocation_preserve_exact_energy() {
    let mut b = CostBasis {
        stock_quantity: 3,
        total_embodied_energy: Energy(10),
    };
    assert_eq!(b.remove(2).unwrap(), Energy(6));
    assert_eq!(
        b,
        CostBasis {
            stock_quantity: 1,
            total_embodied_energy: Energy(4)
        }
    );
    let a = allocate_embodied_energy(Energy(11), &[(id("core:a"), 1, 1), (id("core:b"), 1, 2)])
        .unwrap();
    assert_eq!(a[0].1, Energy(4));
    assert_eq!(a[1].1, Energy(7));
    let permuted =
        allocate_embodied_energy(Energy(11), &[(id("core:b"), 1, 2), (id("core:a"), 1, 1)])
            .unwrap();
    assert_eq!(a, permuted);
}
#[test]
fn ordinary_energy_quotes_limits_buy_sell_and_commit_reject_atomically() {
    let energy = id(ENERGY_ID);

    let mut quotes = GameSession::new(definition()).unwrap();
    let before = quotes.snapshot();
    assert_eq!(
        quotes.quotes(&id("core:s0"), &energy),
        Err(CoreError::EnergyNotTradable)
    );
    assert_eq!(quotes.snapshot(), before);

    let mut limits = GameSession::new(definition()).unwrap();
    let before = limits.snapshot();
    assert_eq!(
        limits.player_local_trade_limits(&energy).unwrap(),
        LocalTradeLimits {
            buy: LocalTradeQuantityLimit {
                maximum: 0,
                reason: LocalTradeLimitReason::TradingUnavailable,
            },
            sell: LocalTradeQuantityLimit {
                maximum: 0,
                reason: LocalTradeLimitReason::TradingUnavailable,
            },
        }
    );
    assert_eq!(limits.snapshot(), before);

    for command in [
        GameCommand::Buy {
            good: energy.clone(),
            quantity: 1,
        },
        GameCommand::Sell {
            good: energy.clone(),
            quantity: 1,
        },
    ] {
        let mut session = GameSession::new(definition()).unwrap();
        let before = session.snapshot();
        assert_eq!(session.submit(command), Err(CoreError::EnergyNotTradable));
        assert_eq!(session.snapshot(), before);
    }

    let mut enabled = definition();
    enabled.player_trade_network_access = TradeNetworkAccess::ReservationContracts;
    let mut commit = GameSession::new(enabled).unwrap();
    let before = commit.snapshot();
    assert_eq!(
        commit.submit(GameCommand::CommitTrade {
            origin: id("core:s0"),
            destination: id("core:s1"),
            good: energy,
            quantity: 1,
        }),
        Err(CoreError::EnergyNotTradable)
    );
    assert!(commit.world.resource::<PendingTradeRequests>().0.is_empty());
    assert_eq!(commit.snapshot(), before);
}

#[test]
fn injected_ordinary_energy_work_and_funded_sale_reject_without_mutation() {
    let energy = id(ENERGY_ID);
    let mut session = GameSession::new(definition()).unwrap();
    let trader = session.player_entity().unwrap();
    let destination = session.market_entity(&id("core:s1")).unwrap();
    session
        .world
        .resource_mut::<PendingTradeRequests>()
        .0
        .push(PendingTradeRequest {
            score: 1,
            trader_id: id("core:player"),
            trader,
            destination: id("core:s1"),
            good: energy.clone(),
            quantity: 1,
            buy_at_origin: true,
            command_driven: false,
        });
    let before = session.snapshot();
    session.resolve_pending_trade_requests().unwrap();
    assert!(
        session
            .world
            .resource::<PendingTradeRequests>()
            .0
            .is_empty()
    );
    assert_eq!(session.snapshot(), before);

    {
        let mut trader_state = session.world.get_mut::<Trader>(trader).unwrap();
        trader_state.cargo.insert(energy.clone(), 5);
        trader_state.cargo_cost_basis.insert(
            energy.clone(),
            CostBasis {
                stock_quantity: 5,
                total_embodied_energy: Energy(5),
            },
        );
    }
    let before = session.snapshot();
    assert_eq!(
        session.execute_funded_sale(
            trader,
            destination,
            &energy,
            1,
            SaleTerms {
                unit_price: Energy(1),
                reserved_release: Energy::ZERO,
                partial: false,
            },
        ),
        Err(CoreError::EnergyNotTradable)
    );
    assert_eq!(session.snapshot(), before);
    assert_eq!(
        session.local_sell(trader, &energy, 1, true),
        Err(CoreError::EnergyNotTradable)
    );
    assert_eq!(session.snapshot(), before);
}

#[test]
fn npc_ordinary_opportunity_collection_omits_energy() {
    let energy = id(ENERGY_ID);
    let mut configured = definition();
    configured.systems[0].targets = BTreeMap::new();
    configured.systems[1].targets = BTreeMap::from([(energy.clone(), 500)]);
    configured.systems[1].inventory.insert(energy.clone(), 0);
    configured.traders.push(TraderDefinition {
        id: id("core:ai"),
        name: "AI".into(),
        system: id("core:s0"),
        archetype: None,
        energy_tank: Energy(500),
        energy_tank_capacity: Energy(1_000),
        bulk_energy_capacity: Energy(1_000),
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut session = GameSession::new(configured).unwrap();
    session.collect_automated_trader_requests().unwrap();
    assert!(
        session
            .world
            .resource::<PendingTradeRequests>()
            .0
            .iter()
            .all(|request| request.good != energy)
    );
}

#[test]
fn idle_npc_selects_and_resolves_a_viable_energy_contract() {
    let mut configured = local_energy_contract_definition();
    let ore = id("core:ore");
    for system in &mut configured.systems {
        system.inventory.insert(ore.clone(), 0);
        system.targets.insert(ore.clone(), 0);
    }
    configured.traders.push(TraderDefinition {
        id: id("core:ai"),
        name: "AI".into(),
        system: id("core:s0"),
        archetype: None,
        energy_tank: Energy(1_000),
        energy_tank_capacity: Energy(1_500),
        bulk_energy_capacity: Energy(1_000),
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut session = GameSession::new(configured).unwrap();

    session.step().unwrap();

    let contracts = session.world.resource::<EnergyContracts>();
    assert_eq!(contracts.active.len(), 1);
    let contract = contracts.active.values().next().unwrap();
    assert_eq!(contract.carrier, id("core:ai"));
    assert_eq!(contract.source, id("core:s0"));
    assert_eq!(contract.destination, id("core:s1"));
    assert!(contract.gross_payload > Energy::ZERO);
    assert!(matches!(
        contract.state,
        EnergyContractState::InTransit { loaded_tick: 0 }
    ));
    assert!(
        session
            .world
            .resource::<PendingEnergyContractIntents>()
            .0
            .is_empty()
    );
    assert!(
        session
            .world
            .resource::<PendingTradeRequests>()
            .0
            .is_empty()
    );
}

#[test]
fn npc_chooses_more_profitable_ordinary_work_over_viable_energy() {
    let mut configured = local_energy_contract_definition();
    configured.systems[1].targets.insert(id("core:ore"), 1_000);
    configured.traders.push(TraderDefinition {
        id: id("core:ai"),
        name: "AI".into(),
        system: id("core:s0"),
        archetype: None,
        energy_tank: Energy(1_000),
        energy_tank_capacity: Energy(1_500),
        bulk_energy_capacity: Energy(1_000),
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut session = GameSession::new(configured).unwrap();

    assert!(
        !session
            .npc_energy_contract_opportunities(&id("core:ai"))
            .unwrap()
            .is_empty()
    );
    session.collect_automated_trader_requests().unwrap();

    assert!(
        session
            .world
            .resource::<PendingEnergyContractIntents>()
            .0
            .is_empty()
    );
    let ordinary = &session.world.resource::<PendingTradeRequests>().0;
    assert_eq!(ordinary.len(), 1);
    assert_eq!(ordinary[0].trader_id, id("core:ai"));
    assert_eq!(ordinary[0].good, id("core:ore"));
}

#[test]
fn higher_brownout_fee_can_make_energy_beat_the_same_ordinary_work() {
    let mut configured = local_energy_contract_definition();
    configured.systems[1].targets.insert(id("core:ore"), 1_000);
    configured.systems[1].energy_logistics.carrier_fee_bps = CarrierFeeSchedule {
        normal: 1_000,
        throttled: 1_100,
        emergency: 1_200,
        starvation: 1_300,
    };
    configured.systems[1].energy_logistics.max_allocation_bps = 2_000;
    configured.traders.push(TraderDefinition {
        id: id("core:ai"),
        name: "AI".into(),
        system: id("core:s0"),
        archetype: None,
        energy_tank: Energy(1_000),
        energy_tank_capacity: Energy(1_500),
        bulk_energy_capacity: Energy(1_000),
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut session = GameSession::new(configured).unwrap();

    session.collect_automated_trader_requests().unwrap();

    assert_eq!(
        session
            .world
            .resource::<PendingEnergyContractIntents>()
            .0
            .len(),
        1
    );
    assert!(
        session
            .world
            .resource::<PendingTradeRequests>()
            .0
            .is_empty()
    );
}

#[test]
fn contract_reimbursement_is_profit_neutral_across_tick_boundaries() {
    let mut configured = local_energy_contract_definition();
    configured.traders.push(TraderDefinition {
        id: id("core:ai"),
        name: "AI".into(),
        system: id("core:s0"),
        archetype: None,
        energy_tank: Energy(1_000),
        energy_tank_capacity: Energy(1_500),
        bulk_energy_capacity: Energy(1_000),
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut session = GameSession::new(configured).unwrap();
    session.world.resource_mut::<FleetDynamics>().mode = Some(FleetMode::Dynamic {
        initial_count: 1,
        opportunity_threshold: u64::MAX,
        opportunity_window: 10,
        spawn_cooldown_ticks: 10,
        retirement_window: 1,
        retirement_threshold: 0,
        maximum_count: 1,
    });
    session
        .world
        .resource_mut::<PendingEnergyContractIntents>()
        .0
        .push(EnergyContractIntent {
            carrier: id("core:ai"),
            source: id("core:s0"),
            destination: id("core:s1"),
            gross_payload: Energy(300),
            command_driven: false,
        });
    session.resolve_pending_energy_contract_intents().unwrap();
    session.evaluate_dynamic_fleet().unwrap();
    let accepted = session
        .snapshot()
        .traders
        .into_iter()
        .find(|trader| trader.id == id("core:ai"))
        .unwrap();
    assert_eq!(accepted.profitability_window, vec![0]);
    assert!(accepted.retirement.is_none());

    session.step().unwrap();
    let completed = session
        .snapshot()
        .traders
        .into_iter()
        .find(|trader| trader.id == id("core:ai"))
        .unwrap();
    assert_eq!(completed.profitability_window, vec![1]);
    assert!(completed.retirement.is_none());
}

#[test]
fn remote_contract_profitability_counts_deadhead_and_fee_only() {
    let mut configured = remote_energy_contract_definition();
    configured.systems[0].energy_logistics.carrier_fee_bps = CarrierFeeSchedule {
        normal: 1_000,
        throttled: 1_100,
        emergency: 1_200,
        starvation: 1_300,
    };
    configured.systems[0].energy_logistics.max_allocation_bps = 2_000;
    configured.traders.push(TraderDefinition {
        id: id("core:ai"),
        name: "AI".into(),
        system: id("core:s0"),
        archetype: None,
        energy_tank: Energy(1_000),
        energy_tank_capacity: Energy(1_500),
        bulk_energy_capacity: Energy(1_000),
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut session = GameSession::new(configured).unwrap();
    session.world.resource_mut::<FleetDynamics>().mode = Some(FleetMode::Dynamic {
        initial_count: 1,
        opportunity_threshold: u64::MAX,
        opportunity_window: 10,
        spawn_cooldown_ticks: 10,
        retirement_window: 10,
        retirement_threshold: i64::MIN,
        maximum_count: 1,
    });
    session
        .world
        .resource_mut::<PendingEnergyContractIntents>()
        .0
        .push(EnergyContractIntent {
            carrier: id("core:ai"),
            source: id("core:s1"),
            destination: id("core:s0"),
            gross_payload: Energy(300),
            command_driven: false,
        });
    session.resolve_pending_energy_contract_intents().unwrap();
    session.evaluate_dynamic_fleet().unwrap();
    session.step().unwrap();
    let loaded = session
        .snapshot()
        .traders
        .into_iter()
        .find(|trader| trader.id == id("core:ai"))
        .unwrap();
    assert_eq!(loaded.profitability_window, vec![-10, 0]);
    assert!(loaded.retirement.is_none());

    {
        let mut traders = session.world.query::<(&StableId, &mut Trader)>();
        let (_, mut trader) = traders
            .iter_mut(&mut session.world)
            .find(|(stable_id, _)| stable_id.0 == id("core:ai"))
            .unwrap();
        trader.system = id("core:s0");
        trader.travel = None;
    }
    session.settle_energy_contracts().unwrap();
    session.evaluate_dynamic_fleet().unwrap();
    let completed = session
        .snapshot()
        .traders
        .into_iter()
        .find(|trader| trader.id == id("core:ai"))
        .unwrap();
    assert_eq!(completed.profitability_window, vec![-10, 0, 30]);
    assert!(completed.retirement.is_none());
}

#[test]
fn energy_intent_contention_is_insertion_order_invariant() {
    let permutations = [
        ["core:ai_a", "core:ai_b", "core:ai_c"],
        ["core:ai_c", "core:ai_b", "core:ai_a"],
        ["core:ai_b", "core:ai_c", "core:ai_a"],
    ];
    let mut outcomes = Vec::new();
    for permutation in permutations {
        let mut configured = local_energy_contract_definition();
        configured.systems[0].inventory.insert(id(ENERGY_ID), 600);
        configured.systems[0].energy_logistics.authored_export_base = Energy(300);
        configured.systems[1].energy_logistics.carrier_fee_bps = CarrierFeeSchedule {
            normal: 1_000,
            throttled: 1_100,
            emergency: 1_200,
            starvation: 1_300,
        };
        configured.systems[1].energy_logistics.max_allocation_bps = 2_000;
        for trader_id in permutation {
            configured.traders.push(TraderDefinition {
                id: id(trader_id),
                name: trader_id.into(),
                system: id("core:s0"),
                archetype: None,
                energy_tank: Energy(1_000),
                energy_tank_capacity: Energy(1_500),
                bulk_energy_capacity: Energy(1_000),
                cargo_capacity: 20,
                speed: 10.0,
                travel_burn_per_distance: Energy(1),
                refuel_policy: RefuelPolicy::DepositAndWithdraw,
                player: false,
            });
        }
        let mut session = GameSession::new(configured).unwrap();
        for trader_id in permutation {
            session
                .world
                .resource_mut::<PendingEnergyContractIntents>()
                .0
                .push(EnergyContractIntent {
                    carrier: id(trader_id),
                    source: id("core:s0"),
                    destination: id("core:s1"),
                    gross_payload: Energy(300),
                    command_driven: false,
                });
        }
        session.resolve_pending_energy_contract_intents().unwrap();
        assert_eq!(
            session
                .world
                .resource::<EnergyContracts>()
                .active
                .values()
                .next()
                .unwrap()
                .carrier,
            id("core:ai_a")
        );
        let events = session.drain_events();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    GameEvent::EnergyLogistics(EnergyContractEvent::Rejected {
                        blocker: EnergyContractBlocker::StaleMaximum,
                        current_maximum: Some(_),
                    })
                ))
                .count(),
            2
        );
        outcomes.push((
            session.snapshot(),
            session.world.resource::<EnergyContracts>().clone(),
            events,
        ));
    }
    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0], outcomes[2]);
}

#[test]
fn destination_settlement_order_is_insertion_order_invariant() {
    let permutations = [
        ["core:ai_a", "core:ai_b", "core:ai_c"],
        ["core:ai_c", "core:ai_b", "core:ai_a"],
        ["core:ai_b", "core:ai_c", "core:ai_a"],
    ];
    let mut outcomes = Vec::new();
    for permutation in permutations {
        let mut session = three_loaded_energy_contract_session(permutation);
        {
            let mut traders = session.world.query::<(&StableId, &mut Trader)>();
            for (stable_id, mut trader) in traders.iter_mut(&mut session.world) {
                if stable_id.0.as_str().starts_with("core:ai_") {
                    trader.system = id("core:s1");
                    trader.travel = None;
                }
            }
        }
        let destination = session.market_entity(&id("core:s1")).unwrap();
        session
            .world
            .get_mut::<Market>(destination)
            .unwrap()
            .set_energy_stock(Energy(4_700))
            .unwrap();
        session.drain_events();
        session.settle_energy_contracts().unwrap();

        let contracts = session.world.resource::<EnergyContracts>().clone();
        assert_eq!(contracts.active.len(), 2);
        assert!(
            !contracts
                .active
                .values()
                .any(|contract| contract.carrier == id("core:ai_a"))
        );
        assert_eq!(
            contracts
                .active
                .values()
                .find(|contract| contract.carrier == id("core:ai_b"))
                .unwrap()
                .cumulative_settled,
            Energy(11)
        );
        assert_eq!(
            contracts
                .active
                .values()
                .find(|contract| contract.carrier == id("core:ai_c"))
                .unwrap()
                .cumulative_settled,
            Energy::ZERO
        );
        outcomes.push((
            session.snapshot(),
            contracts.clone(),
            session.drain_events(),
        ));
    }
    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0], outcomes[2]);
}

#[test]
fn recovery_arrival_order_is_insertion_order_invariant() {
    let permutations = [
        ["core:ai_a", "core:ai_b", "core:ai_c"],
        ["core:ai_c", "core:ai_b", "core:ai_a"],
        ["core:ai_b", "core:ai_c", "core:ai_a"],
    ];
    let mut outcomes = Vec::new();
    for permutation in permutations {
        let mut session = three_loaded_energy_contract_session(permutation);
        {
            let mut traders = session.world.query::<(&StableId, &mut Trader)>();
            for (stable_id, mut trader) in traders.iter_mut(&mut session.world) {
                if stable_id.0.as_str().starts_with("core:ai_") {
                    trader.system = id("core:s1");
                    trader.travel = None;
                }
            }
        }
        let destination = session.market_entity(&id("core:s1")).unwrap();
        session
            .world
            .get_mut::<Market>(destination)
            .unwrap()
            .set_energy_stock(Energy(5_000))
            .unwrap();
        session.settle_energy_contracts().unwrap();
        for contract in session
            .world
            .resource_mut::<EnergyContracts>()
            .active
            .values_mut()
        {
            let EnergyContractState::Arrived { arrived_tick, .. } = contract.state else {
                panic!("expected arrived contract")
            };
            contract.state = EnergyContractState::Arrived {
                arrived_tick,
                settlement_deadline: 0,
            };
        }
        session.settle_energy_contracts().unwrap();
        let contract_ids = session
            .world
            .resource::<EnergyContracts>()
            .active
            .values()
            .map(|contract| (contract.carrier.clone(), contract.id))
            .collect::<BTreeMap<_, _>>();
        for contract in session
            .world
            .resource_mut::<EnergyContracts>()
            .active
            .values_mut()
        {
            contract.state = EnergyContractState::Recovering {
                recovery_departure_tick: match contract.carrier.as_str() {
                    "core:ai_a" => 5,
                    "core:ai_b" => 3,
                    "core:ai_c" => 4,
                    _ => unreachable!(),
                },
            };
        }
        session.world.resource_mut::<Clock>().0 = 10;
        {
            let mut traders = session.world.query::<(&StableId, &mut Trader)>();
            for (stable_id, mut trader) in traders.iter_mut(&mut session.world) {
                if stable_id.0.as_str().starts_with("core:ai_") {
                    trader.system = id("core:s0");
                    trader.travel = None;
                }
            }
        }
        let source = session.market_entity(&id("core:s0")).unwrap();
        session
            .world
            .get_mut::<Market>(source)
            .unwrap()
            .set_energy_stock(Energy(4_700))
            .unwrap();
        session.drain_events();
        session.settle_energy_contracts().unwrap();

        let contracts = session.world.resource::<EnergyContracts>().clone();
        assert!(contracts.active.is_empty());
        assert_eq!(contracts.diagnostics.recovered_after_failure, 3);
        assert_eq!(contracts.diagnostics.recovery_curtailed, Energy(540));
        let curtailments = session
            .world
            .resource::<EventBuffer>()
            .0
            .iter()
            .filter_map(|event| match event {
                GameEvent::EnergyLogistics(EnergyContractEvent::RecoveryCurtailed {
                    contract_id,
                    amount,
                    ..
                }) => Some((*contract_id, *amount)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(curtailments.len(), 2);
        assert_eq!(
            curtailments[0],
            (contract_ids[&id("core:ai_c")], Energy(260))
        );
        assert_eq!(
            curtailments[1],
            (contract_ids[&id("core:ai_a")], Energy(280))
        );
        outcomes.push((
            session.snapshot(),
            contracts.clone(),
            session.drain_events(),
        ));
    }
    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0], outcomes[2]);
}

#[test]
fn exact_owned_bulk_transfers_never_consume_locked_energy() {
    let mut configured = definition();
    configured.traders[0].bulk_energy_capacity = Energy(100);
    configured.traders[0].refuel_policy = RefuelPolicy::Disabled;
    let mut session = GameSession::new(configured).unwrap();
    let trader = session.player_entity().unwrap();
    session
        .world
        .get_mut::<Trader>(trader)
        .unwrap()
        .bulk_energy
        .owned = Energy(50);

    let before_tank = session.snapshot().traders[0].energy_tank;
    session
        .submit(GameCommand::TransferOwnedBulkToTank { amount: Energy(20) })
        .unwrap();
    let after_tank = session.snapshot().traders[0].clone();
    assert_eq!(
        after_tank.energy_tank,
        before_tank.checked_add(Energy(20)).unwrap()
    );
    assert_eq!(after_tank.bulk_energy.owned, Energy(30));

    let before_market = session.snapshot().markets[0].energy_stock;
    session
        .submit(GameCommand::DepositOwnedBulkEnergy { amount: Energy(10) })
        .unwrap();
    let after = session.snapshot();
    assert_eq!(after.traders[0].bulk_energy.owned, Energy(20));
    assert_eq!(
        after.markets[0].energy_stock,
        before_market.checked_add(Energy(10)).unwrap()
    );
    assert_eq!(
        after.markets[0].energy_flow.owned_bulk_deposited,
        Energy(10)
    );

    let contract_id = session
        .world
        .resource_mut::<EnergyContracts>()
        .allocate_id()
        .unwrap();
    {
        let mut trader_state = session.world.get_mut::<Trader>(trader).unwrap();
        trader_state.bulk_energy.owned = Energy::ZERO;
        trader_state.bulk_energy.locked = Some(LockedEnergyLot {
            contract_id,
            amount: Energy(40),
        });
    }
    let before = session.snapshot();
    assert_eq!(
        session.submit(GameCommand::TransferOwnedBulkToTank { amount: Energy(1) }),
        Err(CoreError::InsufficientStock)
    );
    assert_eq!(session.snapshot(), before);
}

#[test]
fn local_energy_contract_acceptance_loads_and_departs_atomically() {
    let mut session = GameSession::new(local_energy_contract_definition()).unwrap();
    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s0"),
            destination: id("core:s1"),
            gross_payload: Energy(300),
        })
        .unwrap();
    assert_eq!(
        session
            .world
            .resource::<PendingEnergyContractIntents>()
            .0
            .len(),
        1
    );

    session.step().unwrap();
    let contracts = session.world.resource::<EnergyContracts>();
    assert_eq!(contracts.active.len(), 1);
    let contract = contracts.active.values().next().unwrap();
    assert!(matches!(
        contract.state,
        EnergyContractState::InTransit { loaded_tick: 0 }
    ));
    let contract_id = contract.id;
    let snapshot = session.snapshot();
    let player = &snapshot.traders[0];
    assert_eq!(player.energy_tank, Energy(990));
    assert_eq!(
        player.bulk_energy.locked,
        Some(LockedEnergyLot {
            contract_id,
            amount: Energy(300),
        })
    );
    assert_eq!(player.travel.as_ref().unwrap().destination, id("core:s1"));
    assert_eq!(snapshot.markets[0].energy_stock, Energy(4_699));
}

#[test]
fn duplicate_player_energy_intent_is_rejected_until_resolution() {
    let mut session = GameSession::new(local_energy_contract_definition()).unwrap();
    let command = GameCommand::AcceptEnergyContract {
        source: id("core:s0"),
        destination: id("core:s1"),
        gross_payload: Energy(300),
    };
    session.submit(command.clone()).unwrap();
    assert_eq!(
        session.submit(command).unwrap_err(),
        CoreError::PendingEnergyContractIntent
    );
    assert_eq!(
        session
            .world
            .resource::<PendingEnergyContractIntents>()
            .0
            .len(),
        1
    );
}

#[test]
fn remote_acceptance_claims_and_player_cancellation_releases_once() {
    let mut session = GameSession::new(remote_energy_contract_definition()).unwrap();
    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s1"),
            destination: id("core:s0"),
            gross_payload: Energy(3_000),
        })
        .unwrap();
    session.step().unwrap();

    let contract_id = {
        let contracts = session.world.resource::<EnergyContracts>();
        let contract = contracts.active.values().next().unwrap();
        assert!(matches!(
            contract.state,
            EnergyContractState::DeadheadingToSource {
                source_claim: Energy(3_000),
                accepted_tick: 0,
            }
        ));
        contract.id
    };
    let accepted = session.snapshot().traders[0].clone();
    assert_eq!(accepted.energy_tank, Energy(990));
    assert!(accepted.travel.is_some());
    assert!(accepted.bulk_energy.locked.is_none());

    session
        .submit(GameCommand::CancelEnergyContract { contract_id })
        .unwrap();
    assert!(
        session
            .world
            .resource::<EnergyContracts>()
            .active
            .is_empty()
    );
    let cancelled = session.snapshot().traders[0].clone();
    assert_eq!(cancelled.energy_tank, Energy(990));
    assert!(
        cancelled.travel.is_some(),
        "deadhead travel is non-cancellable"
    );
    assert!(cancelled.bulk_energy.locked.is_none());
    assert_eq!(
        session
            .world
            .resource::<EnergyContracts>()
            .diagnostics
            .cancelled_before_load,
        1
    );
    assert!(session.drain_events().iter().any(|event| matches!(
        event,
        GameEvent::EnergyLogistics(EnergyContractEvent::Terminal {
            contract_id: id,
            outcome: EnergyContractTerminalOutcome::CancelledBeforeLoad,
        }) if *id == contract_id
    )));
}

#[test]
fn source_distress_revokes_preload_claim_before_loading() {
    let mut session = GameSession::new(remote_energy_contract_definition()).unwrap();
    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s1"),
            destination: id("core:s0"),
            gross_payload: Energy(3_000),
        })
        .unwrap();
    session.step().unwrap();
    session.drain_events();

    let source = session.market_entity(&id("core:s1")).unwrap();
    session
        .world
        .get_mut::<Market>(source)
        .unwrap()
        .set_energy_stock(Energy(100))
        .unwrap();
    session.step().unwrap();

    assert!(
        session
            .world
            .resource::<EnergyContracts>()
            .active
            .is_empty()
    );
    assert_eq!(
        session
            .world
            .resource::<EnergyContracts>()
            .diagnostics
            .revoked_before_load,
        1
    );
    let player = session.snapshot().traders[0].clone();
    assert_eq!(player.system, id("core:s1"));
    assert!(player.travel.is_none());
    assert!(player.bulk_energy.locked.is_none());
    assert!(session.drain_events().iter().any(|event| matches!(
        event,
        GameEvent::EnergyLogistics(EnergyContractEvent::Terminal {
            outcome: EnergyContractTerminalOutcome::RevokedBeforeLoad,
            ..
        })
    )));
}

#[test]
fn preload_arithmetic_failure_propagates_without_terminalizing_contract() {
    let mut session = GameSession::new(remote_energy_contract_definition()).unwrap();
    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s1"),
            destination: id("core:s0"),
            gross_payload: Energy(3_000),
        })
        .unwrap();
    session.step().unwrap();
    let player = session.player_entity().unwrap();
    {
        let mut trader = session.world.get_mut::<Trader>(player).unwrap();
        trader.system = id("core:s1");
        trader.travel = None;
    }
    let source = session.market_entity(&id("core:s1")).unwrap();
    session
        .world
        .get_mut::<Market>(source)
        .unwrap()
        .energy_flow
        .contract_source_loaded = Energy(i64::MAX);
    let before_snapshot = session.snapshot();
    let before_contracts = session.world.resource::<EnergyContracts>().clone();
    let before_events = session.world.resource::<EventBuffer>().0.clone();

    assert_eq!(
        session.maintain_preload_energy_contracts().unwrap_err(),
        CoreError::Overflow
    );
    assert_eq!(session.snapshot(), before_snapshot);
    assert_eq!(
        *session.world.resource::<EnergyContracts>(),
        before_contracts
    );
    assert_eq!(session.world.resource::<EventBuffer>().0, before_events);
}

#[test]
fn immutable_energy_logistics_snapshots_project_markets_opportunities_and_contracts() {
    let mut session = GameSession::new(local_energy_contract_definition()).unwrap();
    let initial = session.snapshot();
    assert_eq!(initial.energy_markets.len(), 2);
    let source = initial
        .energy_markets
        .iter()
        .find(|market| market.system == id("core:s0"))
        .unwrap();
    assert_eq!(source.offered, Energy(681));
    let destination = initial
        .energy_markets
        .iter()
        .find(|market| market.system == id("core:s1"))
        .unwrap();
    assert_eq!(destination.requested, Energy(900));
    let opportunity = initial
        .energy_opportunities
        .iter()
        .find(|opportunity| {
            opportunity.source == id("core:s0") && opportunity.destination == id("core:s1")
        })
        .unwrap();
    assert_eq!(opportunity.maximum_gross_payload, Energy(681));
    assert_eq!(opportunity.net_delivery, Energy(668));
    assert_eq!(opportunity.carrier_allocation, Energy(13));
    assert!(initial.energy_contracts.is_empty());

    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s0"),
            destination: id("core:s1"),
            gross_payload: Energy(300),
        })
        .unwrap();
    session.step().unwrap();
    let accepted = session.snapshot();
    assert_eq!(accepted.energy_contracts.len(), 1);
    assert_eq!(accepted.energy_contracts[0].locked_amount, Energy(300));
    assert_eq!(
        accepted.energy_contracts[0].converted_reimbursement,
        Energy::ZERO
    );
    assert_eq!(accepted.energy_contracts[0].converted_fee, Energy::ZERO);
    assert_eq!(accepted.energy_logistics.accepted, 1);
}

#[test]
fn invalid_contract_projection_returns_typed_snapshot_error() {
    let mut session = GameSession::new(local_energy_contract_definition()).unwrap();
    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s0"),
            destination: id("core:s1"),
            gross_payload: Energy(300),
        })
        .unwrap();
    session.step().unwrap();
    let player = session.player_entity().unwrap();
    session
        .world
        .get_mut::<Trader>(player)
        .unwrap()
        .bulk_energy
        .locked = None;

    assert_eq!(
        session.try_snapshot().unwrap_err(),
        CoreError::InvalidPhysicalDefinition
    );
}

#[test]
fn unsupplied_life_support_has_one_exhaustive_logistics_attribution() {
    let mut configured = definition();
    configured.systems[0].energy_output_per_tick = Energy::ZERO;
    configured.systems[0].seasonal_generation.base_output = Energy::ZERO;
    configured.systems[0]
        .seasonal_generation
        .current_effective_output = Energy::ZERO;
    configured.systems[0].inventory.insert(id(ENERGY_ID), 0);
    configured.systems[1].energy_output_per_tick = Energy::ZERO;
    configured.systems[1].seasonal_generation.base_output = Energy::ZERO;
    configured.systems[1]
        .seasonal_generation
        .current_effective_output = Energy::ZERO;
    configured.systems[1].inventory.insert(id(ENERGY_ID), 0);
    let mut session = GameSession::new(configured).unwrap();

    session.step().unwrap();

    let snapshot = session.snapshot();
    assert_eq!(
        snapshot.energy_starvation.get(&id("core:s1")),
        Some(&EnergyStarvationCause::NoReachableSurplus)
    );
    assert_eq!(snapshot.energy_starvation.len(), 2);
    assert_eq!(snapshot.energy_logistics.no_reachable_surplus, 2);
}

#[test]
fn full_energy_delivery_settles_net_and_allocation_exactly_once() {
    let mut session = GameSession::new(local_energy_contract_definition()).unwrap();
    let initial = session.snapshot();
    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s0"),
            destination: id("core:s1"),
            gross_payload: Energy(300),
        })
        .unwrap();
    session.step().unwrap();
    session.drain_events();
    session.step().unwrap();

    assert!(
        session
            .world
            .resource::<EnergyContracts>()
            .active
            .is_empty()
    );
    assert_eq!(
        session
            .world
            .resource::<EnergyContracts>()
            .diagnostics
            .completed,
        1
    );
    let snapshot = session.snapshot();
    assert_eq!(snapshot.traders[0].energy_tank, Energy(1_001));
    assert_eq!(snapshot.traders[0].bulk_energy, BulkEnergyHold::default());
    assert_eq!(snapshot.markets[1].energy_stock, Energy(407));
    assert_eq!(
        snapshot.markets[0].energy_flow.contract_source_loaded,
        Energy(300)
    );
    assert_eq!(
        snapshot.markets[1]
            .energy_flow
            .contract_destination_delivered,
        Energy(289)
    );
    assert_eq!(
        snapshot.markets[1]
            .energy_flow
            .contract_allocation_converted,
        Energy(11)
    );
    assert_physical_delta_reconciles(&initial, &snapshot);
    let events = session.drain_events();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::EnergyLogistics(EnergyContractEvent::Settled {
                    amount: Energy(289),
                    ..
                })
            ))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::EnergyLogistics(EnergyContractEvent::Terminal {
                    outcome: EnergyContractTerminalOutcome::Completed,
                    ..
                })
            ))
            .count(),
        1
    );
}

#[test]
fn zero_energy_destination_receives_contract_energy_without_prepayment() {
    let mut configured = local_energy_contract_definition();
    configured.systems[1].inventory.insert(id(ENERGY_ID), 0);
    let mut session = GameSession::new(configured).unwrap();
    let initial = session.snapshot();
    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s0"),
            destination: id("core:s1"),
            gross_payload: Energy(300),
        })
        .unwrap();
    session.step().unwrap();
    session.step().unwrap();

    let completed = session.snapshot();
    assert!(completed.markets[1].energy_stock > Energy::ZERO);
    assert_eq!(completed.energy_logistics.completed, 1);
    assert_physical_delta_reconciles(&initial, &completed);
}

#[test]
fn partial_delivery_retries_then_recovers_same_contract() {
    let mut definition = local_energy_contract_definition();
    definition.systems[1]
        .energy_logistics
        .settlement_timeout_ticks = 2;
    let mut session = GameSession::new(definition).unwrap();
    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s0"),
            destination: id("core:s1"),
            gross_payload: Energy(300),
        })
        .unwrap();
    session.step().unwrap();
    let contract_id = *session
        .world
        .resource::<EnergyContracts>()
        .active
        .keys()
        .next()
        .unwrap();
    let destination = session.market_entity(&id("core:s1")).unwrap();
    session
        .world
        .get_mut::<Market>(destination)
        .unwrap()
        .set_energy_stock(Energy(4_900))
        .unwrap();
    let recovery_baseline = session.snapshot();

    session.step().unwrap();
    {
        let contracts = session.world.resource::<EnergyContracts>();
        let contract = &contracts.active[&contract_id];
        assert_eq!(contract.cumulative_settled, Energy(91));
        assert!(matches!(
            contract.state,
            EnergyContractState::Arrived {
                arrived_tick: 1,
                settlement_deadline: 3,
            }
        ));
    }
    let first = session.snapshot().traders[0].clone();
    assert_eq!(first.energy_tank, Energy(1_000));
    assert_eq!(first.bulk_energy.locked.unwrap().amount, Energy(199));

    session.step().unwrap();
    {
        let contract = session
            .world
            .resource::<EnergyContracts>()
            .active
            .get(&contract_id)
            .unwrap();
        assert_eq!(contract.cumulative_settled, Energy(92));
    }
    assert_eq!(
        session.snapshot().traders[0]
            .bulk_energy
            .locked
            .unwrap()
            .amount,
        Energy(198)
    );

    session.step().unwrap();
    {
        let contract = session
            .world
            .resource::<EnergyContracts>()
            .active
            .get(&contract_id)
            .unwrap();
        assert!(matches!(
            contract.state,
            EnergyContractState::Recovering {
                recovery_departure_tick: 3,
            }
        ));
    }
    let recovering = session.snapshot().traders[0].clone();
    assert_eq!(recovering.energy_tank, Energy(1_000));
    assert_eq!(recovering.bulk_energy.locked.unwrap().amount, Energy(188));
    assert_eq!(
        recovering.travel.as_ref().unwrap().destination,
        id("core:s0")
    );

    session.step().unwrap();
    assert!(
        session
            .world
            .resource::<EnergyContracts>()
            .active
            .is_empty()
    );
    assert_eq!(
        session
            .world
            .resource::<EnergyContracts>()
            .diagnostics
            .recovered_after_failure,
        1
    );
    let recovered = session.snapshot();
    assert!(recovered.traders[0].bulk_energy.locked.is_none());
    assert_physical_delta_reconciles(&recovery_baseline, &recovered);
}

#[test]
fn zero_settlement_timeout_returns_or_curtails_every_locked_unit() {
    let mut definition = local_energy_contract_definition();
    definition.economy.life_support_burn_per_capita = Energy::ZERO;
    definition.systems[1].sources.clear();
    definition.systems[1]
        .energy_logistics
        .settlement_timeout_ticks = 1;
    let mut session = GameSession::new(definition).unwrap();
    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s0"),
            destination: id("core:s1"),
            gross_payload: Energy(300),
        })
        .unwrap();
    session.step().unwrap();
    let contract_id = *session
        .world
        .resource::<EnergyContracts>()
        .active
        .keys()
        .next()
        .unwrap();
    let destination = session.market_entity(&id("core:s1")).unwrap();
    session
        .world
        .get_mut::<Market>(destination)
        .unwrap()
        .set_energy_stock(Energy(5_000))
        .unwrap();
    let timeout_baseline = session.snapshot();

    session.step().unwrap();
    let arrived = session
        .world
        .resource::<EnergyContracts>()
        .active
        .get(&contract_id)
        .unwrap();
    assert_eq!(arrived.cumulative_settled, Energy::ZERO);
    assert!(matches!(arrived.state, EnergyContractState::Arrived { .. }));

    session.step().unwrap();
    let recovering = session.snapshot().traders[0].clone();
    assert_eq!(recovering.energy_tank, Energy(1_000));
    assert_eq!(recovering.bulk_energy.locked.unwrap().amount, Energy(280));
    let timeout_complete = session.snapshot();
    assert_physical_delta_reconciles(&timeout_baseline, &timeout_complete);
    let source = session.market_entity(&id("core:s0")).unwrap();
    session
        .world
        .get_mut::<Market>(source)
        .unwrap()
        .set_energy_stock(Energy(5_000))
        .unwrap();
    let recovery_baseline = session.snapshot();

    session.step().unwrap();
    let contracts = session.world.resource::<EnergyContracts>();
    assert!(contracts.active.is_empty());
    assert_eq!(contracts.diagnostics.recovered_after_failure, 1);
    assert_eq!(contracts.diagnostics.recovery_curtailed, Energy(280));
    let recovered = session.snapshot();
    assert_eq!(
        recovered.markets[0].energy_flow.contract_recovery_curtailed,
        Energy(280)
    );
    assert_eq!(
        recovered.markets[0].energy_flow.contract_recovery_returned,
        Energy::ZERO
    );
    assert!(recovered.traders[0].bulk_energy.locked.is_none());
    assert_physical_delta_reconciles(&recovery_baseline, &recovered);
    assert!(session.drain_events().iter().any(|event| matches!(
        event,
        GameEvent::EnergyLogistics(EnergyContractEvent::RecoveryCurtailed {
            contract_id: id,
            amount: Energy(280),
            ..
        }) if *id == contract_id
    )));
}

#[test]
fn proportional_fee_converts_incrementally_without_duplicate_terminal_effects() {
    let mut definition = local_energy_contract_definition();
    definition.systems[1].energy_logistics.carrier_fee_bps = CarrierFeeSchedule {
        normal: 1_000,
        throttled: 1_100,
        emergency: 1_200,
        starvation: 1_300,
    };
    definition.systems[1].energy_logistics.max_allocation_bps = 2_000;
    let mut session = GameSession::new(definition).unwrap();
    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s0"),
            destination: id("core:s1"),
            gross_payload: Energy(300),
        })
        .unwrap();
    session.step().unwrap();
    session.drain_events();
    let contract_id = *session
        .world
        .resource::<EnergyContracts>()
        .active
        .keys()
        .next()
        .unwrap();
    let destination = session.market_entity(&id("core:s1")).unwrap();
    session
        .world
        .get_mut::<Market>(destination)
        .unwrap()
        .set_energy_stock(Energy(4_900))
        .unwrap();

    session.step().unwrap();
    let partial = session
        .world
        .resource::<EnergyContracts>()
        .active
        .get(&contract_id)
        .unwrap();
    assert_eq!(partial.carrier_profit, Energy(30));
    assert_eq!(partial.net_delivery, Energy(260));
    assert_eq!(partial.cumulative_settled, Energy(91));
    let trader = session.snapshot().traders[0].clone();
    assert_eq!(trader.energy_tank, Energy(1_010));
    assert_eq!(trader.bulk_energy.locked.unwrap().amount, Energy(189));

    session
        .world
        .get_mut::<Market>(destination)
        .unwrap()
        .set_energy_stock(Energy(4_800))
        .unwrap();
    session.step().unwrap();
    let completed = session.snapshot().traders[0].clone();
    assert_eq!(completed.energy_tank, Energy(1_030));
    assert_eq!(completed.bulk_energy, BulkEnergyHold::default());
    assert_eq!(completed.ledger.sales_revenue, Energy(40));
    assert_eq!(
        session
            .world
            .resource::<EnergyContracts>()
            .diagnostics
            .completed,
        1
    );
    let terminal_events = session
        .drain_events()
        .into_iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::EnergyLogistics(EnergyContractEvent::Terminal {
                    contract_id: id,
                    outcome: EnergyContractTerminalOutcome::Completed,
                }) if *id == contract_id
            )
        })
        .count();
    assert_eq!(terminal_events, 1);

    session.step().unwrap();
    assert_eq!(
        session
            .world
            .resource::<EnergyContracts>()
            .diagnostics
            .completed,
        1
    );
    assert!(!session.drain_events().iter().any(|event| matches!(
        event,
        GameEvent::EnergyLogistics(EnergyContractEvent::Terminal {
            contract_id: id,
            ..
        }) if *id == contract_id
    )));
}

#[test]
fn settlement_allocation_fills_tank_then_uses_owned_bulk() {
    let mut definition = local_energy_contract_definition();
    definition.traders[0].energy_tank_capacity = Energy(1_000);
    let mut session = GameSession::new(definition).unwrap();
    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s0"),
            destination: id("core:s1"),
            gross_payload: Energy(300),
        })
        .unwrap();
    session.step().unwrap();
    session.step().unwrap();

    let trader = session.snapshot().traders[0].clone();
    assert_eq!(trader.energy_tank, Energy(1_000));
    assert_eq!(trader.bulk_energy.owned, Energy(1));
    assert!(trader.bulk_energy.locked.is_none());
}

#[test]
fn settlement_timeout_and_recovery_failures_are_atomic() {
    let mut session = GameSession::new(local_energy_contract_definition()).unwrap();
    session
        .submit(GameCommand::AcceptEnergyContract {
            source: id("core:s0"),
            destination: id("core:s1"),
            gross_payload: Energy(300),
        })
        .unwrap();
    session.step().unwrap();
    let contract_id = *session
        .world
        .resource::<EnergyContracts>()
        .active
        .keys()
        .next()
        .unwrap();
    let carrier = {
        let mut traders = session.world.query::<(Entity, &StableId, &Trader)>();
        traders
            .iter(&session.world)
            .find_map(|(entity, stable_id, _)| (stable_id.0 == id("core:player")).then_some(entity))
            .unwrap()
    };
    let destination = session.market_entity(&id("core:s1")).unwrap();
    {
        let mut trader = session.world.get_mut::<Trader>(carrier).unwrap();
        trader.system = id("core:s1");
        trader.travel = None;
    }
    session
        .world
        .get_mut::<Market>(destination)
        .unwrap()
        .set_energy_stock(Energy(5_000))
        .unwrap();
    session.settle_energy_contracts().unwrap();
    assert!(matches!(
        session
            .world
            .resource::<EnergyContracts>()
            .active
            .get(&contract_id)
            .unwrap()
            .state,
        EnergyContractState::Arrived { .. }
    ));

    {
        let mut market = session.world.get_mut::<Market>(destination).unwrap();
        market.set_energy_stock(Energy(100)).unwrap();
        market.energy_flow.contract_destination_delivered = Energy(i64::MAX);
    }
    let before_snapshot = session.snapshot();
    let before_contracts = session.world.resource::<EnergyContracts>().clone();
    let before_events = session.world.resource::<EventBuffer>().0.clone();
    assert_eq!(
        session.settle_energy_contracts().unwrap_err(),
        CoreError::Overflow
    );
    assert_eq!(session.snapshot(), before_snapshot);
    assert_eq!(
        *session.world.resource::<EnergyContracts>(),
        before_contracts
    );
    assert_eq!(session.world.resource::<EventBuffer>().0, before_events);

    {
        let mut market = session.world.get_mut::<Market>(destination).unwrap();
        market.set_energy_stock(Energy(5_000)).unwrap();
        market.energy_flow.contract_destination_delivered = Energy::ZERO;
    }
    session
        .world
        .resource_mut::<EnergyContracts>()
        .active
        .get_mut(&contract_id)
        .unwrap()
        .state = EnergyContractState::Arrived {
        arrived_tick: 1,
        settlement_deadline: 1,
    };
    session
        .world
        .get_mut::<Trader>(carrier)
        .unwrap()
        .ledger
        .sales_revenue = Energy(i64::MAX);
    let before_snapshot = session.snapshot();
    let before_contracts = session.world.resource::<EnergyContracts>().clone();
    let before_events = session.world.resource::<EventBuffer>().0.clone();
    assert_eq!(
        session.settle_energy_contracts().unwrap_err(),
        CoreError::Overflow
    );
    assert_eq!(session.snapshot(), before_snapshot);
    assert_eq!(
        *session.world.resource::<EnergyContracts>(),
        before_contracts
    );
    assert_eq!(session.world.resource::<EventBuffer>().0, before_events);

    session
        .world
        .get_mut::<Trader>(carrier)
        .unwrap()
        .ledger
        .sales_revenue = Energy::ZERO;
    session.settle_energy_contracts().unwrap();
    {
        let mut trader = session.world.get_mut::<Trader>(carrier).unwrap();
        trader.system = id("core:s0");
        trader.travel = None;
    }
    session
        .world
        .resource_mut::<EnergyContracts>()
        .diagnostics
        .recovered_after_failure = u64::MAX;
    let before_snapshot = session.snapshot();
    let before_contracts = session.world.resource::<EnergyContracts>().clone();
    let before_events = session.world.resource::<EventBuffer>().0.clone();
    assert_eq!(
        session.settle_energy_contracts().unwrap_err(),
        CoreError::Overflow
    );
    assert_eq!(session.snapshot(), before_snapshot);
    assert_eq!(
        *session.world.resource::<EnergyContracts>(),
        before_contracts
    );
    assert_eq!(session.world.resource::<EventBuffer>().0, before_events);
}

#[test]
fn offline_trade_network_access_rejects_commit_trade_atomically() {
    let mut denied = GameSession::new(definition()).unwrap();
    let mut control = GameSession::new(definition()).unwrap();
    let before = denied.snapshot();

    assert_eq!(
        denied.submit(GameCommand::CommitTrade {
            origin: id("core:s0"),
            destination: id("core:s1"),
            good: id("core:ore"),
            quantity: 1,
        }),
        Err(CoreError::TradeNetworkAccessDenied)
    );
    assert!(
        denied.world.resource::<PendingTradeRequests>().0.is_empty(),
        "a denied player command must not leave deferred work"
    );
    assert_eq!(denied.snapshot(), before);
    assert!(matches!(
        denied.drain_events().as_slice(),
        [GameEvent::Rejected(reason)] if reason.contains("trade-network access")
    ));

    denied.step().unwrap();
    control.step().unwrap();
    assert!(denied.world.resource::<PendingTradeRequests>().0.is_empty());
    assert_eq!(
        denied.snapshot(),
        control.snapshot(),
        "stepping after rejection must match a session that never received the command"
    );
    assert_eq!(denied.drain_events(), control.drain_events());
}

#[test]
fn offline_player_access_does_not_gate_npc_automated_commitments() {
    let mut d = definition();
    d.traders.push(TraderDefinition {
        id: id("core:ai"),
        name: "AI".into(),
        system: id("core:s0"),
        archetype: None,
        energy_tank: Energy(500),
        energy_tank_capacity: Energy(1_000),
        bulk_energy_capacity: Energy::ZERO,
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut s = GameSession::new(d).unwrap();

    s.step().unwrap();

    let npc_entity = s
        .world
        .query_filtered::<(Entity, &StableId), Without<PlayerControlled>>()
        .iter(&s.world)
        .find(|(_, stable)| stable.0 == id("core:ai"))
        .unwrap()
        .0;
    assert!(
        s.world
            .get::<PlayerTradeNetworkAccess>(npc_entity)
            .is_none(),
        "NPC entities must remain capability-free"
    );
    let snapshot = s.snapshot();
    assert_eq!(
        snapshot.player_trade_network_access,
        TradeNetworkAccess::Offline
    );
    let reservation = snapshot
        .reservations
        .iter()
        .find(|reservation| reservation.trader == id("core:ai"))
        .expect("NPC automation should create its funded commitment");
    assert_eq!(reservation.status, ReservationStatus::Active);
    let npc = snapshot
        .traders
        .iter()
        .find(|trader| trader.id == id("core:ai"))
        .unwrap();
    assert!(npc.travel.is_some());
    assert!(npc.reservation.is_some());
}

#[test]
fn local_trade_limits_match_buy_capacity_and_target_met_sell_funding() {
    let ore = id("core:ore");
    let mut session = GameSession::new(definition()).unwrap();
    let initial = session.player_local_trade_limits(&ore).unwrap();
    assert_eq!(initial.buy.maximum, 20);
    assert_eq!(initial.buy.reason, LocalTradeLimitReason::CargoCapacity);
    assert_eq!(initial.sell.maximum, 0);
    assert_eq!(initial.sell.reason, LocalTradeLimitReason::UnitsHeld);

    session
        .submit(GameCommand::Buy {
            good: ore.clone(),
            quantity: 5,
        })
        .unwrap();
    let market = session
        .snapshot()
        .markets
        .into_iter()
        .find(|market| market.system_id == id("core:s0"))
        .unwrap();
    assert!(market.inventory[&ore] >= market.targets[&ore].into());
    assert_eq!(
        market.demand.get(&ore).copied().unwrap_or_default().funded,
        0,
        "advertised target demand is intentionally met"
    );

    let after_buy = session.player_local_trade_limits(&ore).unwrap();
    assert_eq!(after_buy.sell.maximum, 5);
    assert_eq!(after_buy.sell.reason, LocalTradeLimitReason::UnitsHeld);
    let before_rejected = session.snapshot();
    assert_eq!(
        session.submit(GameCommand::Sell {
            good: ore.clone(),
            quantity: 6,
        }),
        Err(CoreError::ExactQuantityUnavailable {
            requested: 6,
            maximum: 5,
        })
    );
    assert_eq!(
        session.snapshot().traders[0].cargo,
        before_rejected.traders[0].cargo
    );
    assert_eq!(
        session.snapshot().traders[0].energy_tank,
        before_rejected.traders[0].energy_tank
    );
    session
        .submit(GameCommand::Sell {
            good: ore,
            quantity: after_buy.sell.maximum,
        })
        .unwrap();
    assert_eq!(
        session
            .snapshot()
            .traders
            .into_iter()
            .find(|trader| trader.player)
            .unwrap()
            .cargo
            .values()
            .sum::<u64>(),
        0
    );
}

#[test]
fn invalid_policy_and_failed_purchase_are_atomic() {
    let mut s = GameSession::new(definition()).unwrap();
    let before = format!("{:?}", s.snapshot());
    let p = MarketPolicy {
        default_target: 0,
        ..MarketPolicy::default()
    };
    assert_eq!(
        s.submit(GameCommand::SetMarketPolicy {
            system: id("core:s0"),
            policy: p
        }),
        Err(CoreError::InvalidPolicy)
    );
    assert_eq!(format!("{:?}", s.snapshot()), before);
    let before = format!("{:?}", s.snapshot());
    assert!(
        s.submit(GameCommand::Buy {
            good: id("core:ore"),
            quantity: u32::MAX
        })
        .is_err()
    );
    assert_eq!(format!("{:?}", s.snapshot()), before);
}
#[test]
fn policy_replacement_recomputes_protection_and_rejects_infeasible_changes_atomically() {
    let mut s = GameSession::new(definition()).unwrap();
    let system = id("core:s0");
    let mut policy = MarketPolicy {
        liquidation_discount_percent: 100,
        operating_reserve_ticks: 99,
        ..MarketPolicy::default()
    };
    s.submit(GameCommand::SetMarketPolicy {
        system: system.clone(),
        policy: policy.clone(),
    })
    .unwrap();
    let changed = s.snapshot();
    assert_eq!(changed.markets[0].policy, policy);
    assert_eq!(changed.markets[0].protected_liquidation_budget, Energy(21));

    policy.operating_reserve_ticks = 0;
    s.submit(GameCommand::SetMarketPolicy {
        system: system.clone(),
        policy: policy.clone(),
    })
    .unwrap();
    assert_eq!(
        s.snapshot().markets[0].protected_liquidation_budget,
        Energy(21),
        "operating reserve must not weaken or inflate anti-strand protection"
    );

    s.drain_events();
    let before = format!("{:?}", s.snapshot());
    policy.liquidation_threshold_percent = u32::MAX;
    assert_eq!(
        s.submit(GameCommand::SetMarketPolicy { system, policy }),
        Err(CoreError::InvalidPhysicalDefinition)
    );
    assert_eq!(format!("{:?}", s.snapshot()), before);
    assert!(matches!(
        s.drain_events().as_slice(),
        [GameEvent::GovernorPolicyRejected {
            reason: GovernorRejectionReason::InvalidPolicy,
            ..
        }]
    ));

    let trader = s.player_entity().unwrap();
    s.world
        .get_mut::<Trader>(trader)
        .unwrap()
        .travel_burn_per_distance = Energy(i64::MAX);
    let before = format!("{:?}", s.snapshot());
    let feasible_policy = MarketPolicy {
        liquidation_discount_percent: 100,
        operating_reserve_ticks: 0,
        ..MarketPolicy::default()
    };
    assert_eq!(
        s.submit(GameCommand::SetMarketPolicy {
            system: id("core:s0"),
            policy: feasible_policy,
        }),
        Err(CoreError::Overflow)
    );
    assert_eq!(format!("{:?}", s.snapshot()), before);
    assert!(matches!(
        s.drain_events().as_slice(),
        [GameEvent::GovernorPolicyRejected {
            reason: GovernorRejectionReason::Arithmetic,
            ..
        }]
    ));
}

#[test]
fn failed_departure_after_staged_purchase_leaves_commitment_snapshot_and_events_unchanged() {
    let mut s = GameSession::new(definition()).unwrap();
    let trader = s.player_entity().unwrap();
    let origin = s.market_entity(&id("core:s0")).unwrap();
    s.world
        .get_mut::<Market>(origin)
        .unwrap()
        .energy_flow
        .travel_burned = Energy(i64::MAX);
    s.drain_events();
    let before_snapshot = format!("{:?}", s.snapshot());
    let before_events = s.drain_events();

    assert_eq!(
        s.commit_and_depart(trader, &id("core:s1"), &id("core:ore"), 1),
        Err(CoreError::Overflow)
    );

    assert_eq!(format!("{:?}", s.snapshot()), before_snapshot);
    assert_eq!(s.drain_events(), before_events);
}

#[test]
fn same_tick_contention_winner_is_invariant_to_trader_insertion_order() {
    fn run(reverse: bool) -> (ContentId, Energy) {
        let mut d = definition();
        d.systems[1].inventory.insert(id(ENERGY_ID), 50);
        d.systems[1].energy_output_per_tick = Energy::ZERO;
        d.systems[1].population = 0;
        d.systems[1].population_state.current = 0;
        d.systems[1].population_state.reference = 0;
        d.systems[1].population_state.carrying_capacity = 0;
        // Keep this fixture focused on reservation contention; autonomous
        // investment spending is covered independently.
        d.systems[1].investment_policy = InvestmentPolicy::default();
        d.systems[1]
            .policy
            .import_priorities
            .insert(id("core:ore"), 200);
        let mut npcs = vec![
            TraderDefinition {
                id: id("core:ai_a"),
                name: "A".into(),
                system: id("core:s0"),
                archetype: None,
                energy_tank: Energy(500),
                energy_tank_capacity: Energy(1_000),
                bulk_energy_capacity: Energy::ZERO,
                cargo_capacity: 20,
                speed: 10.0,
                travel_burn_per_distance: Energy(1),
                refuel_policy: RefuelPolicy::DepositAndWithdraw,
                player: false,
            },
            TraderDefinition {
                id: id("core:ai_b"),
                name: "B".into(),
                system: id("core:s0"),
                archetype: None,
                energy_tank: Energy(500),
                energy_tank_capacity: Energy(1_000),
                bulk_energy_capacity: Energy::ZERO,
                cargo_capacity: 20,
                speed: 10.0,
                travel_burn_per_distance: Energy(1),
                refuel_policy: RefuelPolicy::DepositAndWithdraw,
                player: false,
            },
        ];
        if reverse {
            npcs.reverse();
        }
        d.traders.extend(npcs);
        let mut s = GameSession::new(d).unwrap();
        s.step().unwrap();
        let snapshot = s.snapshot();
        let reservation = snapshot
            .reservations
            .iter()
            .filter(|reservation| reservation.status == ReservationStatus::Active)
            .min_by_key(|reservation| reservation.trader.clone())
            .unwrap();
        let market = snapshot
            .markets
            .iter()
            .find(|market| market.system_id == id("core:s1"))
            .unwrap();
        assert!(market.reserved_energy <= Energy(30));
        (reservation.trader.clone(), market.reserved_energy)
    }

    let forward = run(false);
    let reverse = run(true);
    assert_eq!(forward, reverse);
    assert_eq!(forward.0, id("core:ai_a"));
}

#[test]
fn low_liquidity_arrival_partially_settles_releases_claim_and_reroutes() {
    let mut d = definition();
    d.systems[0].energy_output_per_tick = Energy::ZERO;
    d.systems[1].energy_output_per_tick = Energy::ZERO;
    d.systems[0].population = 0;
    d.systems[1].population = 0;
    d.traders.push(TraderDefinition {
        id: id("core:ai"),
        name: "AI".into(),
        system: id("core:s0"),
        archetype: None,
        energy_tank: Energy(100),
        energy_tank_capacity: Energy(1_000),
        bulk_energy_capacity: Energy::ZERO,
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut s = GameSession::new(d).unwrap();
    let ai = s
        .world
        .query_filtered::<(Entity, &StableId), (With<Trader>, Without<PlayerControlled>)>()
        .iter(&s.world)
        .find(|(_, stable)| stable.0 == id("core:ai"))
        .unwrap()
        .0;
    s.commit_and_depart(ai, &id("core:s1"), &id("core:ore"), 10)
        .unwrap();
    let reservation_id = s.world.get::<Trader>(ai).unwrap().reservation.unwrap();
    let reservation = s
        .world
        .resource::<Reservations>()
        .entries
        .get(&reservation_id)
        .unwrap()
        .clone();
    let destination = s.market_entity(&id("core:s1")).unwrap();
    let protected = s
        .world
        .get::<Market>(destination)
        .unwrap()
        .protected_liquidation_budget;
    s.world
        .get_mut::<Market>(destination)
        .unwrap()
        .set_energy_stock(
            protected
                .checked_add(reservation.floor_unit_price.checked_mul(2).unwrap())
                .unwrap(),
        )
        .unwrap();
    s.drain_events();
    s.step().unwrap();
    let snapshot = s.snapshot();
    let trader = snapshot
        .traders
        .iter()
        .find(|trader| trader.id == id("core:ai"))
        .unwrap();
    assert!(trader.cargo.get(&id("core:ore")).copied().unwrap_or(0) > 0);
    assert!(trader.travel.is_some() || trader.energy_tank > Energy::ZERO);
    assert_eq!(snapshot.markets[1].reserved_energy, Energy::ZERO);
    let released = snapshot
        .reservations
        .iter()
        .find(|entry| entry.id == reservation_id)
        .unwrap();
    assert_eq!(released.status, ReservationStatus::Fulfilled);
    assert_eq!(released.reserved_energy, Energy::ZERO);
    let events = s.drain_events();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::Sold {
            partial: true,
            quantity: 2,
            ..
        }
    )));
    assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, GameEvent::ReservationReleased { reservation, .. } if *reservation == reservation_id))
                .count(),
            1
        );
}

#[test]
fn mandatory_life_support_may_exhaust_claimed_stock_without_failing_arrival_tick() {
    let mut d = definition();
    d.systems[0].population = 0;
    d.systems[1].population = 1_000;
    d.systems[1].policy.operating_reserve_ticks = 0;
    d.systems[0].energy_output_per_tick = Energy::ZERO;
    d.systems[1].energy_output_per_tick = Energy::ZERO;
    d.traders.push(TraderDefinition {
        id: id("core:ai"),
        name: "AI".into(),
        system: id("core:s0"),
        archetype: None,
        energy_tank: Energy(100),
        energy_tank_capacity: Energy(1_000),
        bulk_energy_capacity: Energy::ZERO,
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut s = GameSession::new(d).unwrap();
    let ai = s
        .world
        .query_filtered::<(Entity, &StableId), (With<Trader>, Without<PlayerControlled>)>()
        .iter(&s.world)
        .find(|(_, stable)| stable.0 == id("core:ai"))
        .unwrap()
        .0;
    s.commit_and_depart(ai, &id("core:s1"), &id("core:ore"), 2)
        .unwrap();
    s.step().unwrap();
    let snapshot = s.snapshot();
    assert_eq!(snapshot.markets[1].energy_stock, Energy::ZERO);
    assert_eq!(snapshot.markets[1].reserved_energy, Energy::ZERO);
    assert_eq!(
        snapshot.markets[1].energy_flow.life_support_burned,
        Energy(1_000)
    );
}

#[test]
fn reservation_contention_is_stable_and_partial_settlement_releases_claim() {
    let mut d = definition();
    d.traders.push(TraderDefinition {
        id: id("core:ai"),
        name: "AI".into(),
        system: id("core:s0"),
        archetype: None,
        energy_tank: Energy(500),
        energy_tank_capacity: Energy(1000),
        bulk_energy_capacity: Energy::ZERO,
        cargo_capacity: 20,
        speed: 10.0,
        travel_burn_per_distance: Energy(1),
        refuel_policy: RefuelPolicy::DepositAndWithdraw,
        player: false,
    });
    let mut s = GameSession::new(d).unwrap();
    let ai = s
        .world
        .query_filtered::<Entity, (With<Trader>, Without<PlayerControlled>)>()
        .iter(&s.world)
        .next()
        .unwrap();
    let q = s
        .create_reservation(ai, &id("core:s1"), &id("core:ore"), 20)
        .unwrap();
    assert!(q > 0);
    let reserved = s.snapshot().markets[1].reserved_energy;
    assert!(reserved.0 > 0);
    s.release_reservation(
        s.world.get::<Trader>(ai).unwrap().reservation.unwrap(),
        ReservationStatus::Cancelled,
    )
    .unwrap();
    assert_eq!(s.snapshot().markets[1].reserved_energy, Energy(0));
}
#[test]
fn energy_flow_reconciles_external_delta() {
    let mut s = GameSession::new(definition()).unwrap();
    let before = s.snapshot();
    let total_before: i64 = before.markets.iter().map(|m| m.energy_stock.0).sum::<i64>()
        + before
            .traders
            .iter()
            .map(|t| {
                t.energy_tank.0
                    + i64::try_from(t.cargo.get(&id(ENERGY_ID)).copied().unwrap_or(0)).unwrap()
            })
            .sum::<i64>();
    s.step().unwrap();
    let after = s.snapshot();
    let total_after: i64 = after.markets.iter().map(|m| m.energy_stock.0).sum::<i64>()
        + after
            .traders
            .iter()
            .map(|t| {
                t.energy_tank.0
                    + i64::try_from(t.cargo.get(&id(ENERGY_ID)).copied().unwrap_or(0)).unwrap()
            })
            .sum::<i64>();
    assert_eq!(
        total_after - total_before,
        i64::try_from(i128::from(after.energy_flow.net_external_delta().0)).unwrap()
    );
}

#[test]
fn active_claims_block_discretionary_burn_independently_of_operating_reserve() {
    let mut d = definition();
    d.economy.source_output_percent = 50;
    d.systems[0].policy.operating_reserve_ticks = 0;
    d.systems[0].sources = vec![SourceDefinition {
        good: id("core:ore"),
        quantity_per_tick: 2,
        extraction_energy: Energy(2),
    }];
    d.goods.push(GoodDefinition {
        id: id("core:alloy"),
        name: "Alloy".into(),
        category: GoodCategory::Primary,
        bootstrap_cost: Energy(5),
    });
    d.recipes.push(RecipeDefinition {
        id: id("core:smelt"),
        name: "Smelt".into(),
        layer: RecipeLayer::Primary,
        inputs: vec![GoodAmount {
            good: id("core:ore"),
            quantity: 1,
        }],
        outputs: vec![RecipeOutput {
            good: id("core:alloy"),
            quantity: 1,
            cost_weight: 1,
        }],
        operating_energy: Energy(2),
        margin_percent: None,
    });
    d.systems[0].recipes.push(id("core:smelt"));
    d.systems[0].energy_output_per_tick = Energy::ZERO;
    d.systems[0].population = 0;
    let mut s = GameSession::new(d).unwrap();
    let market = s.market_entity(&id("core:s0")).unwrap();
    let stock = s
        .world
        .get::<Market>(market)
        .unwrap()
        .energy_stock()
        .unwrap();
    s.world.get_mut::<Market>(market).unwrap().reserved_energy = Energy(stock.0 - 1);
    s.step().unwrap();
    let snapshot = s.snapshot();
    let market = snapshot
        .markets
        .iter()
        .find(|market| market.system_id == id("core:s0"))
        .unwrap();
    assert_eq!(market.energy_flow.source_burned, Energy::ZERO);
    assert_eq!(market.energy_flow.production_burned, Energy::ZERO);
    assert_eq!(market.inventory[&id("core:ore")], 100);
    assert_eq!(market.reserved_energy, Energy(stock.0 - 1));

    let mut d = definition();
    d.economy.source_output_percent = 50;
    d.economy.life_support_burn_per_capita = Energy::ZERO;
    d.systems[0].policy.operating_reserve_ticks = 0;
    d.systems[0].sources = vec![SourceDefinition {
        good: id("core:ore"),
        quantity_per_tick: 2,
        extraction_energy: Energy(2),
    }];
    d.systems[0].energy_output_per_tick = Energy::ZERO;
    let mut s = GameSession::new(d).unwrap();
    let entity = s.market_entity(&id("core:s0")).unwrap();
    let protected = s
        .world
        .get::<Market>(entity)
        .unwrap()
        .protected_liquidation_budget;
    s.world
        .get_mut::<Market>(entity)
        .unwrap()
        .set_energy_stock(protected.checked_add(Energy(1)).unwrap())
        .unwrap();
    s.step().unwrap();
    let snapshot = s.snapshot();
    assert_eq!(snapshot.markets[0].energy_flow.source_burned, Energy::ZERO);
    assert_eq!(
        snapshot.markets[0].energy_stock,
        protected.checked_add(Energy(1)).unwrap()
    );
}

#[test]
fn authored_refuel_policy_and_all_protected_claims_bound_tank_withdrawal() {
    let mut d = definition();
    d.traders[0].refuel_policy = RefuelPolicy::DepositOnly;
    let mut s = GameSession::new(d).unwrap();
    assert_eq!(
        s.submit(GameCommand::WithdrawTank { amount: Energy(1) }),
        Err(CoreError::RefuelForbidden)
    );

    let trader = s.player_entity().unwrap();
    s.world.get_mut::<Trader>(trader).unwrap().refuel_policy = RefuelPolicy::DepositAndWithdraw;
    let market = s.market_entity(&id("core:s0")).unwrap();
    let life = s
        .world
        .resource::<EconomyConfig>()
        .life_support_burn_per_capita;
    let policy = s.world.get::<MarketPolicy>(market).unwrap().clone();
    s.world.get_mut::<Market>(market).unwrap().reserved_energy = Energy(100);
    let available = s
        .world
        .get::<Market>(market)
        .unwrap()
        .unreserved_energy_for_purchases(&policy, life)
        .unwrap();
    assert_eq!(
        s.submit(GameCommand::WithdrawTank {
            amount: available.checked_add(Energy(1)).unwrap(),
        }),
        Err(CoreError::InsufficientEnergy)
    );
    s.submit(GameCommand::WithdrawTank { amount: available })
        .unwrap();
    let market = s.world.get::<Market>(market).unwrap();
    assert_eq!(
        market.energy_stock().unwrap(),
        market
            .reserved_energy
            .checked_add(market.operating_reserve(&policy, life).unwrap())
            .unwrap()
            .checked_add(market.protected_liquidation_budget)
            .unwrap()
    );
}

#[test]
fn buy_tank_transfer_and_travel_are_atomic_on_ledger_overflow() {
    let mut s = GameSession::new(definition()).unwrap();
    let market = s.market_entity(&id("core:s0")).unwrap();
    s.world
        .get_mut::<Market>(market)
        .unwrap()
        .ledger
        .energy_received_from_traders = Energy(i64::MAX);
    let before = format!("{:?}", s.snapshot());
    assert_eq!(
        s.submit(GameCommand::Buy {
            good: id("core:ore"),
            quantity: 1,
        }),
        Err(CoreError::Overflow)
    );
    assert_eq!(format!("{:?}", s.snapshot()), before);

    s.world
        .get_mut::<Market>(market)
        .unwrap()
        .energy_flow
        .tank_to_market = Energy(i64::MAX);
    let before = format!("{:?}", s.snapshot());
    assert_eq!(
        s.submit(GameCommand::DepositTank { amount: Energy(1) }),
        Err(CoreError::Overflow)
    );
    assert_eq!(format!("{:?}", s.snapshot()), before);

    s.world
        .get_mut::<Market>(market)
        .unwrap()
        .energy_flow
        .travel_burned = Energy(i64::MAX);
    let before = format!("{:?}", s.snapshot());
    assert_eq!(
        s.submit(GameCommand::BeginTravel {
            destination: id("core:s1"),
        }),
        Err(CoreError::Overflow)
    );
    assert_eq!(format!("{:?}", s.snapshot()), before);
}

#[test]
fn cost_aware_ask_compounds_margin_and_bounded_scarcity_with_checked_rounding() {
    let mut d = definition();
    d.systems[0].inventory.insert(id("core:ore"), 0);
    d.systems[0].targets.insert(id("core:ore"), 10);
    d.systems[0].policy.producer_margin_percent = 20;
    let mut s = GameSession::new(d).unwrap();
    // ceil(3 * 1.20) = 4, then ceil(4 * 1.50) = 6.
    assert_eq!(
        s.quotes(&id("core:s0"), &id("core:ore")).unwrap().1,
        Energy(6)
    );
    assert_eq!(
        checked_mul_ratio_ceil(Energy(i64::MAX), 2, 1),
        Err(CoreError::Overflow)
    );
}

#[test]
fn route_subsidy_raises_solvent_bid_and_canonical_dynamic_backlog() {
    let mut definition = definition();
    enable_investments(&mut definition);
    definition.fleet = dynamic_fleet(0, 2, 1, 100);
    definition.goods.push(GoodDefinition {
        id: id("core:alloy"),
        name: "Alloy".into(),
        category: GoodCategory::Primary,
        bootstrap_cost: Energy(20),
    });
    definition.recipes.push(RecipeDefinition {
        id: id("core:smelt"),
        name: "Smelt".into(),
        layer: RecipeLayer::Primary,
        inputs: vec![GoodAmount {
            good: id("core:ore"),
            quantity: 1,
        }],
        outputs: vec![RecipeOutput {
            good: id("core:alloy"),
            quantity: 1,
            cost_weight: 1,
        }],
        operating_energy: Energy::ZERO,
        margin_percent: Some(0),
    });
    definition.systems[1].recipes.push(id("core:smelt"));
    definition.systems[1].inventory.insert(id("core:alloy"), 10);
    definition.systems[1].targets.insert(id("core:alloy"), 10);

    let mut unsubsidized = GameSession::new(definition.clone()).unwrap();
    let mut subsidized = GameSession::new(definition).unwrap();
    let destination = subsidized.market_entity(&id("core:s1")).unwrap();
    subsidized
        .world
        .get_mut::<Market>(destination)
        .unwrap()
        .investment_state
        .levels
        .insert(InvestmentKind::RouteSubsidy, 1);

    let ore = id("core:ore");
    let normal_bid = unsubsidized.quotes(&id("core:s1"), &ore).unwrap().0;
    let subsidized_bid = subsidized.quotes(&id("core:s1"), &ore).unwrap().0;
    let ceiling = {
        let market = subsidized.world.get::<Market>(destination).unwrap();
        let policy = subsidized.world.get::<MarketPolicy>(destination).unwrap();
        subsidized
            .processor_input_bid_ceiling(market, policy, &ore)
            .unwrap()
            .unwrap()
    };
    assert!(subsidized_bid > normal_bid);
    assert!(subsidized_bid <= ceiling);
    let solvency = subsidized
        .processor_solvency()
        .unwrap()
        .into_iter()
        .find(|row| row.system == id("core:s1") && row.recipe == id("core:smelt"))
        .unwrap();
    assert!(solvency.solvent, "{solvency:?}");

    unsubsidized.collect_automated_trader_requests().unwrap();
    subsidized.collect_automated_trader_requests().unwrap();
    let normal_backlog = unsubsidized
        .world
        .resource::<FleetDynamics>()
        .normalized_unserved_opportunity;
    let subsidized_backlog = subsidized
        .world
        .resource::<FleetDynamics>()
        .normalized_unserved_opportunity;
    assert!(
        subsidized_backlog > normal_backlog,
        "subsidy did not increase canonical fleet routing signal: normal={normal_backlog}, subsidized={subsidized_backlog}"
    );
}

#[test]
fn processor_input_bids_are_non_recursive_and_structurally_solvent() {
    let mut d = definition();
    d.goods.extend([
        GoodDefinition {
            id: id("core:catalyst"),
            name: "Catalyst".into(),
            category: GoodCategory::Raw,
            bootstrap_cost: Energy(2),
        },
        GoodDefinition {
            id: id("core:alloy"),
            name: "Alloy".into(),
            category: GoodCategory::Primary,
            bootstrap_cost: Energy(12),
        },
    ]);
    d.recipes.push(RecipeDefinition {
        id: id("core:smelt"),
        name: "Smelt".into(),
        layer: RecipeLayer::Primary,
        inputs: vec![
            GoodAmount {
                good: id("core:ore"),
                quantity: 2,
            },
            GoodAmount {
                good: id("core:catalyst"),
                quantity: 1,
            },
        ],
        outputs: vec![RecipeOutput {
            good: id("core:alloy"),
            quantity: 1,
            cost_weight: 1,
        }],
        operating_energy: Energy(2),
        margin_percent: Some(20),
    });
    d.systems[0].recipes.push(id("core:smelt"));
    d.systems[0].inventory.insert(id("core:catalyst"), 10);
    d.systems[0].inventory.insert(id("core:alloy"), 10);
    d.systems[0].targets.insert(id("core:catalyst"), 10);
    d.systems[0].targets.insert(id("core:alloy"), 10);
    d.economy.investments.insert(
        InvestmentKind::RouteSubsidy,
        InvestmentShape {
            enabled: true,
            base_cost: Energy(100),
            cost_growth_percent: 150,
            maximum_level: 2,
            cooldown_ticks: 2,
            effect_per_level: 10,
        },
    );
    let mut s = GameSession::new(d).unwrap();
    let baseline_bid = s.quotes(&id("core:s0"), &id("core:ore")).unwrap().0;
    let rows = s.processor_solvency().unwrap();
    let row = rows
        .iter()
        .find(|row| row.recipe == id("core:smelt"))
        .unwrap();
    assert!(row.solvent, "{row:?}");
    assert!(row.expected_input_bids.0 > 0);

    let market = s.market_entity(&id("core:s0")).unwrap();
    s.world
        .get_mut::<Market>(market)
        .unwrap()
        .investment_state
        .levels
        .insert(InvestmentKind::RouteSubsidy, 2);
    assert_eq!(
        s.quotes(&id("core:s0"), &id("core:ore")).unwrap().0,
        baseline_bid,
        "a subsidy cannot raise a processor input above its solvency ceiling"
    );
    let subsidized = s
        .processor_solvency()
        .unwrap()
        .into_iter()
        .find(|row| row.recipe == id("core:smelt"))
        .unwrap();
    assert!(subsidized.solvent, "{subsidized:?}");
}

#[test]
fn runtime_cost_propagates_through_single_multi_output_and_consuming_recipes() {
    let mut d = definition();
    d.economy.life_support_burn_per_capita = Energy::ZERO;
    d.goods.extend([
        GoodDefinition {
            id: id("core:alloy"),
            name: "Alloy".into(),
            category: GoodCategory::Primary,
            bootstrap_cost: Energy(5),
        },
        GoodDefinition {
            id: id("core:slag"),
            name: "Slag".into(),
            category: GoodCategory::Primary,
            bootstrap_cost: Energy(1),
        },
        GoodDefinition {
            id: id("core:machine"),
            name: "Machine".into(),
            category: GoodCategory::Secondary,
            bootstrap_cost: Energy(9),
        },
    ]);
    d.recipes.extend([
        RecipeDefinition {
            id: id("core:split"),
            name: "Split".into(),
            layer: RecipeLayer::Primary,
            inputs: vec![GoodAmount {
                good: id("core:ore"),
                quantity: 1,
            }],
            outputs: vec![
                RecipeOutput {
                    good: id("core:alloy"),
                    quantity: 1,
                    cost_weight: 1,
                },
                RecipeOutput {
                    good: id("core:slag"),
                    quantity: 1,
                    cost_weight: 2,
                },
            ],
            operating_energy: Energy(2),
            margin_percent: None,
        },
        RecipeDefinition {
            id: id("core:forge"),
            name: "Forge".into(),
            layer: RecipeLayer::Secondary,
            inputs: vec![GoodAmount {
                good: id("core:alloy"),
                quantity: 1,
            }],
            outputs: vec![RecipeOutput {
                good: id("core:machine"),
                quantity: 1,
                cost_weight: 1,
            }],
            operating_energy: Energy(3),
            margin_percent: None,
        },
        RecipeDefinition {
            id: id("core:consume"),
            name: "Consume".into(),
            layer: RecipeLayer::Tertiary,
            inputs: vec![GoodAmount {
                good: id("core:machine"),
                quantity: 1,
            }],
            outputs: vec![],
            operating_energy: Energy(1),
            margin_percent: None,
        },
    ]);
    d.systems[0].recipes = vec![id("core:split"), id("core:forge"), id("core:consume")];
    d.systems[0].energy_output_per_tick = Energy::ZERO;
    let mut s = GameSession::new(d).unwrap();
    s.step().unwrap();
    let snapshot = s.snapshot();
    let market = &snapshot.markets[0];
    assert_eq!(
        market.cost_basis[&id("core:slag")].total_embodied_energy,
        Energy(3)
    );
    assert_eq!(market.cost_basis[&id("core:alloy")].stock_quantity, 0);
    assert_eq!(market.cost_basis[&id("core:machine")].stock_quantity, 0);
    assert_eq!(market.energy_flow.production_burned, Energy(6));
    assert_eq!(market.ledger.processor_input_cost, Energy(5));
    assert_eq!(market.ledger.processor_operating_energy, Energy(5));
}

#[test]
fn recipe_margin_override_is_applied_to_runtime_quote() {
    let mut d = definition();
    d.goods.push(GoodDefinition {
        id: id("core:alloy"),
        name: "Alloy".into(),
        category: GoodCategory::Primary,
        bootstrap_cost: Energy(5),
    });
    d.recipes.push(RecipeDefinition {
        id: id("core:smelt"),
        name: "Smelt".into(),
        layer: RecipeLayer::Primary,
        inputs: vec![GoodAmount {
            good: id("core:ore"),
            quantity: 1,
        }],
        outputs: vec![RecipeOutput {
            good: id("core:alloy"),
            quantity: 1,
            cost_weight: 1,
        }],
        operating_energy: Energy(1),
        margin_percent: Some(50),
    });
    d.systems[0].recipes.push(id("core:smelt"));
    d.systems[0].inventory.insert(id("core:alloy"), 10);
    d.systems[0].targets.insert(id("core:alloy"), 10);
    d.systems[0].policy.producer_margin_percent = 0;
    let mut s = GameSession::new(d).unwrap();
    assert_eq!(
        s.quotes(&id("core:s0"), &id("core:alloy")).unwrap().1,
        Energy(8)
    );
}

#[test]
fn source_scaling_controls_runtime_output_burn_and_operating_reserve() {
    let mut d = definition();
    d.economy.source_output_percent = 50;
    d.economy.life_support_burn_per_capita = Energy::ZERO;
    d.systems[0].sources.push(SourceDefinition {
        good: id("core:ore"),
        quantity_per_tick: 3,
        extraction_energy: Energy(1),
    });
    d.systems[0].energy_output_per_tick = Energy::ZERO;
    d.systems[0].policy.operating_reserve_ticks = 1;
    let mut s = GameSession::new(d).unwrap();
    assert_eq!(s.snapshot().markets[0].operating_reserve, Energy(1));
    s.step().unwrap();
    let market = &s.snapshot().markets[0];
    assert_eq!(market.inventory[&id("core:ore")], 101);
    assert_eq!(market.energy_flow.source_burned, Energy(1));
}

#[test]
fn route_burn_sums_each_leg_ceiling_and_global_flow_never_clamps() {
    let a = id("core:a");
    let b = id("core:b");
    let c = id("core:c");
    let graph = SystemGraph {
        positions: BTreeMap::new(),
        edges: BTreeMap::from([
            (a.clone(), vec![(b.clone(), 0.4)]),
            (b.clone(), vec![(a.clone(), 0.4), (c.clone(), 0.4)]),
            (c.clone(), vec![(b.clone(), 0.4)]),
        ]),
    };
    assert_eq!(
        route_travel_energy(&graph, &[a, b, c], Energy(1)).unwrap(),
        Energy(2)
    );
    assert_eq!(travel_energy(0.8, Energy(1)).unwrap(), Energy(1));

    let mut aggregate = GlobalEnergyFlowLedger::default();
    let flow = EnergyFlowLedger {
        generated: Energy(i64::MAX),
        ..EnergyFlowLedger::default()
    };
    aggregate.add_market(flow);
    aggregate.add_market(flow);
    assert_eq!(
        aggregate.generated,
        WideEnergy(WideAmount(i128::from(i64::MAX) * 2))
    );
}

#[test]
fn liquidation_contract_and_threshold_are_deterministic() {
    let reference = Energy(7);
    assert_eq!(liquidation_unit_price(reference, 50).unwrap(), Energy(3));
    assert_eq!(
        liquidation_target_energy(Energy(11), 150).unwrap(),
        Energy(17)
    );
    let dynamic_adversarial_bid = Energy(i64::MAX / 100);
    assert_ne!(
        liquidation_unit_price(reference, 50).unwrap(),
        liquidation_unit_price(dynamic_adversarial_bid, 50).unwrap()
    );
}

#[test]
fn brownout_boundaries_shocks_and_recovery_are_deterministic() {
    let config = BrownoutConfig::default();
    let normal = BrownoutState::default();
    for (runway, expected) in [
        (u32::MAX, BrownoutStage::Normal),
        (13, BrownoutStage::Normal),
        (12, BrownoutStage::Throttled),
        (7, BrownoutStage::Throttled),
        (6, BrownoutStage::Emergency),
        (2, BrownoutStage::Emergency),
        (1, BrownoutStage::Starvation),
        (0, BrownoutStage::Starvation),
    ] {
        assert_eq!(
            classify_brownout(&normal, &config, runway, Energy::ZERO, 10).unwrap(),
            expected,
            "runway {runway}"
        );
    }
    assert_eq!(
        classify_brownout(&normal, &config, 100, Energy(1), 10).unwrap(),
        BrownoutStage::Starvation,
        "unsupplied life support directly crosses all bands"
    );

    let mut state = BrownoutState {
        stage: BrownoutStage::Starvation,
        entered_at_tick: 5,
        ..BrownoutState::default()
    };
    assert_eq!(
        classify_brownout(&state, &config, 100, Energy::ZERO, 5).unwrap(),
        BrownoutStage::Starvation,
        "minimum occupancy blocks same-tick recovery"
    );
    assert_eq!(
        classify_brownout(&state, &config, 3, Energy::ZERO, 6).unwrap(),
        BrownoutStage::Emergency
    );
    state.stage = BrownoutStage::Emergency;
    state.entered_at_tick = 6;
    assert_eq!(
        classify_brownout(&state, &config, 8, Energy::ZERO, 7).unwrap(),
        BrownoutStage::Throttled
    );
    state.stage = BrownoutStage::Throttled;
    state.entered_at_tick = 7;
    assert_eq!(
        classify_brownout(&state, &config, 16, Energy::ZERO, 8).unwrap(),
        BrownoutStage::Normal
    );
}

#[test]
fn triangle_throughput_population_fleet_and_investment_helpers_cover_boundaries() {
    assert_eq!(
        (0..4)
            .map(|tick| triangle_wave_output(Energy(100), 20, 4, 0, tick).unwrap())
            .collect::<Vec<_>>(),
        vec![Energy(80), Energy(100), Energy(120), Energy(100)]
    );
    assert_eq!(
        triangle_wave_output(Energy(i64::MAX), 0, 2, 0, u64::MAX).unwrap(),
        Energy(i64::MAX),
        "zero amplitude is exactly fixed output without tick overflow"
    );
    assert!(triangle_wave_output(Energy(1), 101, 2, 0, 0).is_err());
    assert!(
        triangle_wave_output(Energy(100), 20, 3, 0, 0).is_err(),
        "nonzero seasonal amplitude requires an even period"
    );
    assert_eq!(
        triangle_wave_output(Energy(100), 0, 3, 0, 1).unwrap(),
        Energy(100),
        "odd periods remain harmless for fixed-output seasons"
    );
    let odd_state = SeasonalGenerationState {
        base_output: Energy(100),
        amplitude_percent: 20,
        period_ticks: 3,
        phase_ticks: 0,
        current_effective_output: Energy(100),
    };
    assert_eq!(odd_state.validate(), Err(CoreError::InvalidWorldDynamics));
    assert_eq!(
        triangle_wave_output(Energy(100), 100, 4, 0, 0).unwrap(),
        Energy::ZERO,
        "the maximum permitted amplitude cannot produce negative generation"
    );
    assert_eq!(
        (0..4)
            .map(|tick| triangle_wave_output(Energy(100), 20, 4, 1, tick).unwrap())
            .collect::<Vec<_>>(),
        vec![Energy(100), Energy(120), Energy(100), Energy(80)],
        "an even period reaches exact extrema at the phase-shifted turning points"
    );
    assert_eq!(
        triangle_wave_output(Energy(100), 20, 4, 1, 3).unwrap(),
        triangle_wave_output(Energy(100), 20, 4, 1, 7).unwrap(),
        "phase-shifted output repeats exactly after one period"
    );
    assert_eq!(
        triangle_wave_output(Energy(100), 20, 4, 0, u64::MAX).unwrap(),
        triangle_wave_output(Energy(100), 20, 4, 0, u64::MAX % 4).unwrap(),
        "large ticks wrap before phase addition"
    );
    assert!(triangle_wave_output(Energy(i64::MAX), 100, 2, 0, 1).is_err());
    let phase = seasonal_phase(4, 0, 0).unwrap();
    assert_eq!(phase.trend, SeasonalTrend::Rising);
    assert_eq!(phase.next_turning_point_tick, Some(2));
    assert_eq!(
        seasonal_phase(4, 0, 2).unwrap().trend,
        SeasonalTrend::Falling
    );
    assert_eq!(
        seasonal_phase(4, 0, u64::MAX)
            .unwrap()
            .next_turning_point_tick,
        None,
        "a turning point beyond the clock range is explicit"
    );

    for (stage, labor, expected) in [(0, 100, 0), (1, 100, 1), (100, 100, 100)] {
        let mut production_carry = 0;
        let mut reserve_carry = 0;
        let mut diagnostic_carry = 0;
        assert_eq!(
            composed_throughput(100, stage, labor, &mut production_carry).unwrap(),
            expected
        );
        assert_eq!(
            composed_throughput(100, stage, labor, &mut reserve_carry).unwrap(),
            expected
        );
        assert_eq!(
            composed_throughput(100, stage, labor, &mut diagnostic_carry).unwrap(),
            expected
        );
    }
    let mut carry = 0;
    assert_eq!(
        (0..4)
            .map(|_| composed_throughput(1, 50, 50, &mut carry).unwrap())
            .collect::<Vec<_>>(),
        vec![0, 0, 0, 1],
        "stage and labor are multiplied before one final carry"
    );
    assert_eq!(carry, 0);

    let mut population_carry = LogisticGrowthCarry::default();
    assert_eq!(
        logistic_population_delta(90, 100, 1_000, 1, &mut population_carry).unwrap(),
        9
    );
    assert_eq!(
        logistic_population_delta(100, 100, 1_000, 1, &mut population_carry).unwrap(),
        0
    );
    assert_eq!(update_opportunity_persistence(4, 10, 10).unwrap(), 5);
    assert_eq!(update_opportunity_persistence(4, 9, 10).unwrap(), 0);
    assert!(update_opportunity_persistence(0, 1, 0).is_err());

    let shape = InvestmentShape {
        enabled: true,
        base_cost: Energy(100),
        cost_growth_percent: 150,
        maximum_level: 3,
        cooldown_ticks: 1,
        effect_per_level: 1,
    };
    assert_eq!(investment_cost(&shape, 0).unwrap(), Energy(100));
    assert_eq!(investment_cost(&shape, 1).unwrap(), Energy(150));
    assert_eq!(investment_cost(&shape, 2).unwrap(), Energy(225));
    assert!(investment_cost(&shape, 3).is_err());
}

#[test]
fn investment_max_effect_validation_accepts_boundaries_and_rejects_first_invalid() {
    let base_shape = |effect_per_level| InvestmentShape {
        enabled: true,
        base_cost: Energy(1),
        cost_growth_percent: 100,
        maximum_level: 1,
        cooldown_ticks: 1,
        effect_per_level,
    };
    let mut shapes = default_investment_shapes();
    let mut population = PopulationConfig::default();

    shapes.insert(InvestmentKind::RouteSubsidy, base_shape(u32::MAX - 100));
    validate_investment_shapes(&shapes, &population).unwrap();
    shapes.insert(InvestmentKind::RouteSubsidy, base_shape(u32::MAX - 99));
    assert_eq!(
        validate_investment_shapes(&shapes, &population),
        Err(CoreError::InvalidWorldDynamics)
    );

    shapes.insert(InvestmentKind::RouteSubsidy, base_shape(1));
    population.growth_per_thousand = 200;
    let maximum_growth_bonus = u32::MAX / 2 - 100;
    shapes.insert(
        InvestmentKind::PopulationSupport,
        base_shape(maximum_growth_bonus),
    );
    validate_investment_shapes(&shapes, &population).unwrap();
    shapes.insert(
        InvestmentKind::PopulationSupport,
        base_shape(maximum_growth_bonus + 1),
    );
    assert_eq!(
        validate_investment_shapes(&shapes, &population),
        Err(CoreError::InvalidWorldDynamics)
    );

    population.growth_per_thousand = 1;
    population.maximum_cap = u64::MAX / 1_000;
    shapes.insert(
        InvestmentKind::PopulationSupport,
        base_shape(u32::MAX - 100),
    );
    assert_eq!(
        validate_investment_shapes(&shapes, &population),
        Err(CoreError::InvalidWorldDynamics),
        "the maximum logistic numerator must remain within u128"
    );

    shapes.insert(InvestmentKind::PopulationSupport, base_shape(1));
    shapes.insert(InvestmentKind::Collector, base_shape(1));
    shapes.insert(InvestmentKind::Storage, base_shape(1));
    let maximum_seasonal_base = i64::MAX / 2;
    let seasonal = SeasonalGenerationState {
        base_output: Energy(maximum_seasonal_base - 1),
        amplitude_percent: 100,
        period_ticks: 2,
        phase_ticks: 0,
        current_effective_output: Energy::ZERO,
    };
    validate_market_investment_bounds(&shapes, &seasonal, Energy(i64::MAX - 1)).unwrap();
    let first_invalid_collector = SeasonalGenerationState {
        base_output: Energy(maximum_seasonal_base),
        ..seasonal.clone()
    };
    assert_eq!(
        validate_market_investment_bounds(&shapes, &first_invalid_collector, Energy(i64::MAX - 1),),
        Err(CoreError::InvalidWorldDynamics)
    );
    assert_eq!(
        validate_market_investment_bounds(&shapes, &seasonal, Energy(i64::MAX)),
        Err(CoreError::InvalidWorldDynamics)
    );
}

#[test]
fn maximum_valid_collector_purchase_executes_the_following_consuming_phase() {
    let mut d = definition();
    enable_investments(&mut d);
    d.economy
        .investments
        .get_mut(&InvestmentKind::Collector)
        .unwrap()
        .cooldown_ticks = 1;
    d.systems[0].investment_policy = InvestmentPolicy {
        allocation_percent: BTreeMap::from([(InvestmentKind::Collector, 100)]),
    };
    let mut session = GameSession::new(d).unwrap();
    session.step().unwrap();
    session.step().unwrap();
    assert_eq!(
        session
            .snapshot()
            .markets
            .into_iter()
            .find(|market| market.system_id == id("core:s0"))
            .unwrap()
            .investment_state
            .levels[&InvestmentKind::Collector],
        2
    );
    session.step().unwrap();
    let market = session
        .snapshot()
        .markets
        .into_iter()
        .find(|market| market.system_id == id("core:s0"))
        .unwrap();
    assert_eq!(
        market.seasonal_generation.current_effective_output,
        Energy(2)
    );
    assert_eq!(
        market.investment_state.status[&InvestmentKind::Collector],
        InvestmentStatus::MaximumLevel
    );
}

#[test]
fn seasonal_generation_runs_before_life_support_and_is_projected() {
    let mut d = definition();
    d.systems[0].energy_output_per_tick = Energy(100);
    d.systems[0].seasonal_generation = SeasonalGenerationState {
        base_output: Energy(100),
        amplitude_percent: 20,
        period_ticks: 4,
        phase_ticks: 0,
        current_effective_output: Energy(100),
    };
    d.systems[0].energy_storage_cap = Energy(10_000);
    d.systems[0].inventory.insert(id(ENERGY_ID), 1_000);
    d.systems[0].population = 1;
    let mut session = GameSession::new(d).unwrap();
    session.step().unwrap();
    let events = session.drain_events();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::EnergyGenerated { system, amount: Energy(80), .. }
            if system == &id("core:s0")
    )));
    let market = session
        .snapshot()
        .markets
        .into_iter()
        .find(|market| market.system_id == id("core:s0"))
        .unwrap();
    assert_eq!(market.energy_stock, Energy(1_079));
    assert_eq!(market.seasonal_generation.base_output, Energy(100));
    assert_eq!(
        market.seasonal_generation.current_effective_output,
        Energy(80)
    );
    assert_eq!(market.seasonal_phase.position_ticks, 0);
    assert_eq!(market.seasonal_phase.next_turning_point_tick, Some(2));
}

#[test]
fn recorded_external_delivery_is_atomic_and_reconciles_a_stage_intervention() {
    let mut d = definition();
    d.systems[0].energy_output_per_tick = Energy::ZERO;
    d.systems[0].seasonal_generation.base_output = Energy::ZERO;
    d.systems[0].seasonal_generation.current_effective_output = Energy::ZERO;
    d.systems[0].inventory.insert(id(ENERGY_ID), 7);
    d.systems[0].population = 1;
    let mut baseline = GameSession::new(d.clone()).unwrap();
    let mut intervention = GameSession::new(d).unwrap();
    let initial_physical = physical_energy(&intervention.snapshot());
    intervention
        .submit(GameCommand::RecordExternalDelivery {
            system: id("core:s0"),
            good: id(ENERGY_ID),
            quantity: 10,
        })
        .unwrap();
    baseline.step().unwrap();
    intervention.step().unwrap();
    let baseline_market = baseline.snapshot().markets.remove(0);
    let intervention_snapshot = intervention.snapshot();
    let intervention_market = intervention_snapshot.markets[0].clone();
    assert_eq!(baseline_market.brownout.stage, BrownoutStage::Emergency);
    assert_eq!(intervention_market.brownout.stage, BrownoutStage::Normal);
    assert_eq!(
        i128::from(intervention_snapshot.energy_flow.external_inflow.0),
        10_i128
    );
    assert_eq!(
        i128::from(intervention_snapshot.energy_flow.net_external_delta().0),
        physical_energy(&intervention_snapshot) - initial_physical
    );
    assert_eq!(
        intervention
            .drain_events()
            .iter()
            .filter(|event| matches!(event, GameEvent::ExternalDeliveryRecorded { .. }))
            .count(),
        1
    );

    let before = intervention.snapshot().markets[0].energy_stock;
    assert_eq!(
        intervention.submit(GameCommand::RecordExternalDelivery {
            system: id("core:s0"),
            good: id(ENERGY_ID),
            quantity: 20_000,
        }),
        Err(CoreError::InsufficientCapacity)
    );
    assert_eq!(intervention.snapshot().markets[0].energy_stock, before);
    assert!(
        !intervention
            .drain_events()
            .iter()
            .any(|event| matches!(event, GameEvent::ExternalDeliveryRecorded { .. }))
    );
}

#[test]
fn brownout_runtime_suppresses_demand_caps_price_and_preserves_reservations() {
    let mut d = definition();
    d.systems[0].energy_output_per_tick = Energy::ZERO;
    d.systems[0].inventory.insert(id(ENERGY_ID), 7);
    d.systems[0].population = 1;
    let mut session = GameSession::new(d).unwrap();
    let energy = id(ENERGY_ID);
    let ore = id("core:ore");
    assert_eq!(
        session.quotes(&id("core:s0"), &energy),
        Err(CoreError::EnergyNotTradable)
    );
    let player = session.player_entity().unwrap();
    let reserved_quantity = session
        .create_reservation(player, &id("core:s1"), &ore, 1)
        .unwrap();
    assert_eq!(reserved_quantity, 1);
    let reservation_id = session
        .world
        .get::<Trader>(player)
        .unwrap()
        .reservation
        .unwrap();
    let reserved_before = session.snapshot().markets[1].reserved_energy;

    session.step().unwrap();
    let snapshot = session.snapshot();
    let distressed = snapshot
        .markets
        .iter()
        .find(|market| market.system_id == id("core:s0"))
        .unwrap();
    assert_eq!(distressed.brownout.stage, BrownoutStage::Emergency);
    assert_eq!(distressed.operating_profile.throughput_percent, 0);
    assert_eq!(
        session.quotes(&id("core:s0"), &ore).unwrap().0,
        Energy::ZERO
    );
    assert_eq!(
        session.quotes(&id("core:s0"), &energy),
        Err(CoreError::EnergyNotTradable)
    );
    assert_eq!(distressed.unreserved_energy_for_purchases, Energy::ZERO);
    assert_eq!(distressed.protected_liquidation_budget, Energy(20));
    assert_eq!(snapshot.markets[1].reserved_energy, reserved_before);
    assert_eq!(
        snapshot
            .reservations
            .iter()
            .find(|reservation| reservation.id == reservation_id)
            .unwrap()
            .status,
        ReservationStatus::Active
    );
    let events = session.drain_events();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::BrownoutTransition {
            from: BrownoutStage::Normal,
            to: BrownoutStage::Emergency,
            ..
        }
    )));
    assert!(!events.iter().any(|event| matches!(
        event,
        GameEvent::TraderSpawned { .. } | GameEvent::TraderRetired { .. }
    )));

    session.step().unwrap();
    let steady_events = session.drain_events();
    assert!(
        !steady_events
            .iter()
            .any(|event| matches!(event, GameEvent::BrownoutTransition { .. }))
    );
    let steady = session.snapshot();
    let distressed = steady
        .markets
        .iter()
        .find(|market| market.system_id == id("core:s0"))
        .unwrap();
    assert_eq!(
        distressed.brownout.occupancy_ticks[BrownoutStage::Emergency.index()],
        2
    );
    assert_eq!(distressed.brownout.transition_count, 1);
    assert_eq!(
        steady
            .dynamics_history
            .stage_occupancy_ticks
            .iter()
            .sum::<u64>(),
        4
    );
}

#[test]
fn throttled_recipe_uses_one_deterministic_final_carry() {
    let mut d = definition();
    d.goods.push(GoodDefinition {
        id: id("core:alloy"),
        name: "Alloy".into(),
        category: GoodCategory::Primary,
        bootstrap_cost: Energy(5),
    });
    d.recipes.push(RecipeDefinition {
        id: id("core:smelt"),
        name: "Smelt".into(),
        layer: RecipeLayer::Primary,
        inputs: vec![GoodAmount {
            good: id("core:ore"),
            quantity: 1,
        }],
        outputs: vec![RecipeOutput {
            good: id("core:alloy"),
            quantity: 1,
            cost_weight: 1,
        }],
        operating_energy: Energy(1),
        margin_percent: None,
    });
    d.systems[0].recipes.push(id("core:smelt"));
    d.systems[0].inventory.insert(id("core:alloy"), 0);
    d.systems[0].energy_output_per_tick = Energy::ZERO;
    d.systems[0].inventory.insert(id(ENERGY_ID), 130);
    d.systems[0].population = 10;
    let mut session = GameSession::new(d).unwrap();

    session.step().unwrap();
    let first = session.snapshot();
    assert_eq!(first.markets[0].brownout.stage, BrownoutStage::Throttled);
    assert_eq!(first.markets[0].inventory[&id("core:alloy")], 0);
    session.step().unwrap();
    let second = session.snapshot();
    assert_eq!(second.markets[0].brownout.stage, BrownoutStage::Throttled);
    assert_eq!(second.markets[0].inventory[&id("core:alloy")], 1);
    assert_eq!(second.markets[0].energy_flow.production_burned, Energy(1));
}

#[test]
fn player_policy_changes_require_matching_governance_and_are_atomic() {
    let mut definition = definition();
    definition.systems[1].governance = Governance::default();
    let mut session = GameSession::new(definition).unwrap();

    let before = format!("{:?}", session.snapshot());
    let unauthorized = MarketPolicy {
        producer_margin_percent: 44,
        ..MarketPolicy::default()
    };
    assert_eq!(
        session.submit(GameCommand::SetMarketPolicy {
            system: id("core:s1"),
            policy: unauthorized,
        }),
        Err(CoreError::UnauthorizedMarketPolicy)
    );
    assert_eq!(format!("{:?}", session.snapshot()), before);

    let authorized = MarketPolicy {
        producer_margin_percent: 33,
        ..MarketPolicy::default()
    };
    session
        .submit(GameCommand::SetMarketPolicy {
            system: id("core:s0"),
            policy: authorized.clone(),
        })
        .unwrap();
    assert_eq!(session.snapshot().markets[0].policy, authorized);
}

#[test]
fn canonical_ordinary_market_demand_excludes_energy_in_every_stage() {
    let mut session = GameSession::new(definition()).unwrap();
    let system = id("core:s1");
    let ore = id("core:ore");
    let energy = id(ENERGY_ID);

    let normal = session.market_demand(&system, &ore).unwrap();
    assert_eq!(normal.advertised, 10);
    assert_eq!(session.snapshot().markets[1].demand[&ore], normal);

    let entity = session.market_entity(&system).unwrap();
    session
        .world
        .get_mut::<MarketPolicy>(entity)
        .unwrap()
        .operating_reserve_ticks = 0;
    {
        let mut market = session.world.get_mut::<Market>(entity).unwrap();
        market.set_energy_stock(Energy(40)).unwrap();
        market.reserved_energy = Energy(9);
    }
    let constrained = session.market_demand(&system, &ore).unwrap();
    assert!(constrained.funded < constrained.advertised);
    assert_eq!(session.snapshot().markets[1].demand[&ore], constrained);

    {
        let mut market = session.world.get_mut::<Market>(entity).unwrap();
        market.operating_profile.stage = BrownoutStage::Emergency;
        market.targets.insert(energy.clone(), 100);
    }
    assert_eq!(
        session.market_demand(&system, &ore).unwrap(),
        MarketDemandSnapshot::default()
    );
    assert_eq!(
        session.market_demand(&system, &energy),
        Err(CoreError::EnergyNotTradable)
    );
    let snapshot = session.snapshot();
    assert_eq!(snapshot.markets[1].demand[&ore].advertised, 0);
    assert!(!snapshot.markets[1].demand.contains_key(&energy));
}

#[test]
fn operating_reserve_follows_distinct_source_and_recipe_carry_schedules() {
    let mut definition = definition();
    definition.economy.life_support_burn_per_capita = Energy::ZERO;
    definition.systems[0].sources.push(SourceDefinition {
        good: id("core:ore"),
        quantity_per_tick: 1,
        extraction_energy: Energy(5),
    });
    for (recipe, cost) in [("core:r1", 3), ("core:r2", 7)] {
        definition.recipes.push(RecipeDefinition {
            id: id(recipe),
            name: recipe.into(),
            layer: RecipeLayer::Tertiary,
            inputs: vec![GoodAmount {
                good: id("core:ore"),
                quantity: 1,
            }],
            outputs: vec![],
            operating_energy: Energy(cost),
            margin_percent: None,
        });
        definition.systems[0].recipes.push(id(recipe));
    }
    let mut session = GameSession::new(definition).unwrap();
    let entity = session.market_entity(&id("core:s0")).unwrap();
    let mut policy = session.world.get::<MarketPolicy>(entity).unwrap().clone();
    policy.operating_reserve_ticks = 4;

    {
        let mut market = session.world.get_mut::<Market>(entity).unwrap();
        market.operating_profile.throughput_percent = 0;
    }
    assert_eq!(
        session
            .world
            .get::<Market>(entity)
            .unwrap()
            .operating_reserve(&policy, Energy::ZERO)
            .unwrap(),
        Energy::ZERO
    );
    {
        let mut market = session.world.get_mut::<Market>(entity).unwrap();
        market.operating_profile.throughput_percent = 50;
    }
    assert_eq!(
        session
            .world
            .get::<Market>(entity)
            .unwrap()
            .operating_reserve(&policy, Energy::ZERO)
            .unwrap(),
        Energy(30)
    );
    {
        let mut market = session.world.get_mut::<Market>(entity).unwrap();
        market.operating_profile.throughput_percent = 100;
    }
    assert_eq!(
        session
            .world
            .get::<Market>(entity)
            .unwrap()
            .operating_reserve(&policy, Energy::ZERO)
            .unwrap(),
        Energy(60)
    );

    policy.operating_reserve_ticks = 1;
    {
        let mut market = session.world.get_mut::<Market>(entity).unwrap();
        market.operating_profile.throughput_percent = 50;
        market
            .throughput_carry
            .insert(ThroughputScheduleKey::Source(id("core:ore")), 5_000);
        for recipe in ["core:r1", "core:r2"] {
            market
                .throughput_carry
                .insert(ThroughputScheduleKey::Recipe(id(recipe)), 5_000);
        }
    }
    assert_eq!(
        session
            .world
            .get::<Market>(entity)
            .unwrap()
            .operating_reserve(&policy, Energy::ZERO)
            .unwrap(),
        Energy(15),
        "reserve must begin from each persistent carry without mutating it"
    );
    assert!(
        session
            .world
            .get::<Market>(entity)
            .unwrap()
            .throughput_carry
            .values()
            .all(|carry| *carry == 5_000)
    );
}

#[test]
fn duplicate_market_schedules_are_rejected_by_core() {
    let mut duplicate_source = definition();
    let source = SourceDefinition {
        good: id("core:ore"),
        quantity_per_tick: 1,
        extraction_energy: Energy(1),
    };
    duplicate_source.systems[0].sources = vec![source.clone(), source];
    assert!(matches!(
        GameSession::new(duplicate_source),
        Err(CoreError::InvalidPhysicalDefinition)
    ));

    let mut duplicate_recipe = definition();
    duplicate_recipe.systems[0].recipes = vec![id("core:r"), id("core:r")];
    assert!(matches!(
        GameSession::new(duplicate_recipe),
        Err(CoreError::InvalidPhysicalDefinition)
    ));
}

#[test]
fn energy_import_priority_and_recovery_ladder_validation_are_ordered() {
    let mut invalid_ceiling = definition();
    invalid_ceiling.systems[0]
        .policy
        .import_priorities
        .insert(id(ENERGY_ID), 2_000);
    assert!(matches!(
        GameSession::new(invalid_ceiling),
        Err(CoreError::InvalidWorldDynamics)
    ));

    let mut session = GameSession::new(definition()).unwrap();
    let mut invalid_policy = MarketPolicy::default();
    invalid_policy
        .import_priorities
        .insert(id(ENERGY_ID), 2_000);
    let before = format!("{:?}", session.snapshot());
    assert_eq!(
        session.submit(GameCommand::SetMarketPolicy {
            system: id("core:s0"),
            policy: invalid_policy,
        }),
        Err(CoreError::InvalidPolicy)
    );
    assert_eq!(format!("{:?}", session.snapshot()), before);

    let mut invalid_recovery = BrownoutConfig::default();
    invalid_recovery.starvation_recovery_ticks = invalid_recovery.emergency_recovery_ticks;
    assert_eq!(
        invalid_recovery.validate(),
        Err(CoreError::InvalidWorldDynamics)
    );
    invalid_recovery = BrownoutConfig::default();
    invalid_recovery.emergency_recovery_ticks = invalid_recovery.throttled_recovery_ticks;
    assert_eq!(
        invalid_recovery.validate(),
        Err(CoreError::InvalidWorldDynamics)
    );
}

#[test]
fn population_window_accepts_documented_maximum_and_rejects_first_value_above_it() {
    let mut config = PopulationConfig {
        sufficiency_window: MAX_POPULATION_SUFFICIENCY_WINDOW_TICKS,
        ..PopulationConfig::default()
    };
    assert_eq!(validate_population_config(&config), Ok(()));
    config.sufficiency_window = MAX_POPULATION_SUFFICIENCY_WINDOW_TICKS + 1;
    assert_eq!(
        validate_population_config(&config),
        Err(CoreError::InvalidWorldDynamics)
    );
}

#[test]
fn constructed_population_state_rejects_an_unpaired_growth_remainder() {
    let mut invalid = definition();
    invalid.systems[0].population_state.growth_carry = LogisticGrowthCarry {
        remainder: 1,
        denominator: 3,
    };
    assert!(matches!(
        GameSession::new(invalid),
        Err(CoreError::InvalidPhysicalDefinition)
    ));
}

#[test]
fn incompatible_logistic_rebases_round_ties_to_even() {
    assert_eq!(rebase_fraction_half_even(1, 4, 10).unwrap(), 2);
    assert_eq!(rebase_fraction_half_even(3, 4, 10).unwrap(), 8);
}

#[test]
fn logistic_population_delta_rejects_invalid_inputs_without_mutating_carry() {
    let mut carry = LogisticGrowthCarry {
        remainder: 17,
        denominator: 100_000,
    };
    assert_eq!(
        logistic_population_delta(10, 100, 1, 0, &mut carry),
        Err(CoreError::InvalidWorldDynamics)
    );
    assert_eq!(
        carry,
        LogisticGrowthCarry {
            remainder: 17,
            denominator: 100_000,
        }
    );

    let mut invalid_carry = LogisticGrowthCarry {
        remainder: 100_000,
        denominator: 100_000,
    };
    let before = invalid_carry;
    assert_eq!(
        logistic_population_delta(10, 100, 1, 1, &mut invalid_carry),
        Err(CoreError::InvalidWorldDynamics)
    );
    assert_eq!(invalid_carry, before);

    let mut overflow_carry = LogisticGrowthCarry {
        remainder: 23,
        denominator: 1_000,
    };
    let before = overflow_carry;
    assert_eq!(
        logistic_population_delta(u64::MAX / 2, u64::MAX, u32::MAX, 1, &mut overflow_carry),
        Err(CoreError::Overflow)
    );
    assert_eq!(overflow_carry, before);
}

#[test]
fn logistic_growth_rebases_track_exact_reference_without_capacity_jumps_or_stalls() {
    const COMMON_CAPACITY: u64 = 10_000;
    const COMMON_DENOMINATOR: u128 = 10_000_000;
    const CASES: &[(&str, &[u64], usize)] = &[
        ("alternating extremes", &[2, 10_000], 10_000),
        ("intermittent low caps", &[20, 25, 40, 100], 10_000),
        ("intermittent high caps", &[125, 200, 250, 500], 10_000),
    ];

    for &(name, capacities, ticks) in CASES {
        let mut actual_population = 1_u64;
        let mut actual_carry = LogisticGrowthCarry::default();
        let mut reference_population = 1_u64;
        // All table capacities divide COMMON_CAPACITY, so this is an exact
        // rational accumulator rather than a floating-point approximation.
        let mut reference_remainder = 0_u128;

        for tick in 0..ticks {
            let capacity = capacities[tick % capacities.len()];
            assert_eq!(COMMON_CAPACITY % capacity, 0, "{name}");
            let before = actual_population;
            let raw_numerator = u128::from(before) * u128::from(capacity.saturating_sub(before));
            let delta =
                logistic_population_delta(before, capacity, 1, 1, &mut actual_carry).unwrap();
            if before >= capacity {
                assert_eq!(delta, 0, "capacity changes cannot move population: {name}");
            } else {
                let denominator = u128::from(capacity) * 1_000;
                let maximum_tick_growth = raw_numerator.div_ceil(denominator);
                assert!(
                    u128::from(delta) <= maximum_tick_growth,
                    "capacity rebase caused a discontinuous jump in {name} at tick {tick}"
                );
            }
            actual_population = actual_population.checked_add(delta).unwrap();

            if reference_population < capacity {
                reference_remainder += u128::from(reference_population)
                    * u128::from(capacity - reference_population)
                    * u128::from(COMMON_CAPACITY / capacity);
                let reference_delta = u64::try_from(reference_remainder / COMMON_DENOMINATOR)
                    .unwrap()
                    .min(capacity - reference_population);
                reference_remainder %= COMMON_DENOMINATOR;
                reference_population += reference_delta;
            }
        }

        assert!(
            actual_population > 1,
            "growth permanently stalled in {name}"
        );
        assert!(
            actual_population.abs_diff(reference_population) <= 1,
            "{name}: actual={actual_population}, exact={reference_population}"
        );
    }
}

#[test]
fn alternating_two_and_four_capacity_matches_exact_fraction_and_settlement_tick() {
    const COMMON_DENOMINATOR: u128 = 4_000;
    const EXACT_FIRST_SETTLEMENT_TICK: usize = 1_600;
    const MAX_SETTLEMENT_TICK_ERROR: usize = 1;

    let mut actual_population = 1_u64;
    let mut actual_carry = LogisticGrowthCarry::default();
    let mut exact_population = 1_u64;
    let mut exact_remainder = 0_u128;
    let mut actual_first_settlement = None;
    let mut exact_first_settlement = None;
    let mut maximum_cumulative_population_error = 0_u64;

    for tick in 1..=2_000 {
        let capacity = if tick % 2 == 1 { 2 } else { 4 };
        let actual_delta =
            logistic_population_delta(actual_population, capacity, 1, 1, &mut actual_carry)
                .unwrap();
        actual_population += actual_delta;
        if actual_population > 1 && actual_first_settlement.is_none() {
            actual_first_settlement = Some(tick);
        }

        if exact_population < capacity {
            let active_denominator = u128::from(capacity) * 1_000;
            exact_remainder += u128::from(exact_population)
                * u128::from(capacity - exact_population)
                * (COMMON_DENOMINATOR / active_denominator);
            let exact_delta = u64::try_from(exact_remainder / COMMON_DENOMINATOR)
                .unwrap()
                .min(capacity - exact_population);
            exact_remainder %= COMMON_DENOMINATOR;
            exact_population += exact_delta;
            if exact_population > 1 && exact_first_settlement.is_none() {
                exact_first_settlement = Some(tick);
            }
        }

        assert_eq!(
            COMMON_DENOMINATOR % u128::from(actual_carry.denominator),
            0,
            "tick {tick}: owner carry cannot be compared exactly"
        );
        let owner_progress = u128::from(actual_population) * COMMON_DENOMINATOR
            + u128::from(actual_carry.remainder)
                * (COMMON_DENOMINATOR / u128::from(actual_carry.denominator));
        let exact_progress = u128::from(exact_population) * COMMON_DENOMINATOR + exact_remainder;
        assert_eq!(
            owner_progress, exact_progress,
            "tick {tick}: owner population/fraction diverged from exact rational progress"
        );
        maximum_cumulative_population_error =
            maximum_cumulative_population_error.max(actual_population.abs_diff(exact_population));
    }

    assert!(
        maximum_cumulative_population_error <= 1,
        "owner cumulative population error exceeded one: {maximum_cumulative_population_error}"
    );
    assert_eq!(exact_first_settlement, Some(EXACT_FIRST_SETTLEMENT_TICK));
    let actual_first_settlement = actual_first_settlement.unwrap();
    assert!(
        actual_first_settlement.abs_diff(EXACT_FIRST_SETTLEMENT_TICK) <= MAX_SETTLEMENT_TICK_ERROR,
        "owner settled at tick {actual_first_settlement}, exact tick is {EXACT_FIRST_SETTLEMENT_TICK}"
    );
}

#[test]
fn tiny_population_remainder_progress_survives_repeated_rebases() {
    let mut carry = LogisticGrowthCarry::default();
    let first_growth_tick = (1..=2_000).find(|tick| {
        let capacity = if tick % 2 == 0 { 10_000 } else { 2 };
        logistic_population_delta(1, capacity, 1, 1, &mut carry).unwrap() > 0
    });
    assert!(
        first_growth_tick.is_some(),
        "alternating denominator rebases must not erase tiny-population progress"
    );
}

#[test]
fn logistic_growth_carry_rebases_atomically_across_capacity_changes() {
    let mut carry = LogisticGrowthCarry {
        remainder: 50_000,
        denominator: 100_000,
    };
    assert_eq!(
        logistic_population_delta(5, 10, 1, 1, &mut carry).unwrap(),
        0,
        "a downward cap change accepts an old remainder larger than the new denominator"
    );
    assert_eq!(carry.denominator, 100_000);
    assert_eq!(carry.remainder, 50_250);

    assert_eq!(
        logistic_population_delta(5, 200, 1, 1, &mut carry).unwrap(),
        0,
        "an upward cap change preserves the rebased fractional carry"
    );
    assert_eq!(carry.denominator, 200_000);
    assert_eq!(carry.remainder, 101_475);
}

#[test]
fn population_helpers_cover_rates_remainders_caps_and_zero() {
    assert_eq!(population_labor_percent(0, 10).unwrap(), 0);
    assert_eq!(population_labor_percent(5, 10).unwrap(), 50);
    assert_eq!(population_labor_percent(20, 10).unwrap(), 100);
    assert_eq!(population_demand_target(60, 4, 8, 1).unwrap(), 30);
    assert_eq!(population_demand_target(0, 1, 1, 1).unwrap(), 1);
    assert_eq!(population_tier(0, &[1, 5, 10]), 0);
    assert_eq!(population_tier(5, &[1, 5, 10]), 2);

    let mut decline_remainder = 0;
    let declines = (0..100)
        .map(|_| proportional_population_delta(1, 10, &mut decline_remainder).unwrap())
        .sum::<u64>();
    assert_eq!(
        declines, 1,
        "tiny populations progress through carried decline"
    );
    assert_eq!(decline_remainder, 0);
    let mut zero_remainder = 0;
    assert_eq!(
        proportional_population_delta(0, 10, &mut zero_remainder).unwrap(),
        0,
        "an empty market stays empty"
    );

    let mut growth_carry = LogisticGrowthCarry::default();
    let growth = (0..2)
        .map(|_| logistic_population_delta(10, 20, 100, 1, &mut growth_carry).unwrap())
        .sum::<u64>();
    assert_eq!(growth, 1);
    assert_eq!(growth_carry.remainder, 0);
    let mut cap_carry = LogisticGrowthCarry::default();
    assert_eq!(
        (0..2)
            .map(|_| logistic_population_delta(19, 20, 1_000, 1, &mut cap_carry).unwrap())
            .sum::<u64>(),
        1
    );
    assert_eq!(
        logistic_population_delta(20, 20, 1_000, 1, &mut cap_carry).unwrap(),
        0,
        "logistic growth never overshoots its cap"
    );
}

#[test]
fn moving_sufficiency_window_gates_slow_growth_and_evicts_oldest() {
    let mut d = definition();
    d.economy.population.static_population = false;
    d.economy.population.sufficiency_window = 2;
    d.economy.population.essential_goods = BTreeSet::from([id(ENERGY_ID)]);
    d.economy.population.tertiary_demand_per_thousand.clear();
    d.economy.population.decline_per_thousand = 500;
    d.economy.population.growth_per_thousand = 100;
    d.economy.population.logistic_scale = 1;
    d.systems[0].population = 10;
    d.systems[0].population_state = PopulationState {
        current: 10,
        reference: 10,
        carrying_capacity: 20,
        support_capacity: 20,
        ..PopulationState::default()
    };
    let mut session = GameSession::new(d).unwrap();
    let entity = session.market_entity(&id("core:s0")).unwrap();

    session.update_populations().unwrap();
    assert_eq!(session.world.get::<Market>(entity).unwrap().population, 10);
    assert_eq!(
        session
            .world
            .get::<Market>(entity)
            .unwrap()
            .population_state
            .sufficiency_samples,
        VecDeque::from([100])
    );
    session.update_populations().unwrap();
    assert_eq!(session.world.get::<Market>(entity).unwrap().population, 10);
    session.update_populations().unwrap();
    let state = &session
        .world
        .get::<Market>(entity)
        .unwrap()
        .population_state;
    assert_eq!(
        state.current, 11,
        "growth waits for a full long-average window"
    );
    assert_eq!(state.sufficiency_samples, VecDeque::from([100, 100]));
    assert_eq!(state.sufficiency_sum, 200);

    {
        let mut market = session.world.get_mut::<Market>(entity).unwrap();
        market.last_life_support_unsupplied = Energy(11);
    }
    session.update_populations().unwrap();
    let state = &session
        .world
        .get::<Market>(entity)
        .unwrap()
        .population_state;
    assert_eq!(state.sufficiency_samples, VecDeque::from([100, 0]));
    assert_eq!(state.sufficiency_sum, 100);
    assert_eq!(state.sufficiency_average_percent, 50);
    assert_eq!(state.trend, PopulationTrend::Stable);
}

#[test]
fn population_change_drives_next_tick_burn_labor_and_tertiary_demand() {
    let mut d = definition();
    d.economy.population.static_population = false;
    d.economy.population.sufficiency_window = 2;
    d.economy.population.essential_goods = BTreeSet::from([id(ENERGY_ID)]);
    d.economy
        .population
        .tertiary_demand_per_thousand
        .insert(id("core:ore"), 1_000);
    d.economy.population.decline_per_thousand = 100;
    d.economy.population.growth_per_thousand = 20;
    d.economy.population.logistic_scale = 1;
    d.economy.population.tier_thresholds = vec![1, 50, 100];
    d.systems[0].population = 100;
    d.systems[0].population_state = PopulationState {
        current: 100,
        reference: 100,
        carrying_capacity: 120,
        support_capacity: 120,
        ..PopulationState::default()
    };
    d.systems[0].inventory.insert(id(ENERGY_ID), 0);
    d.systems[0].inventory.insert(id("core:ore"), 100);
    d.systems[0].targets.insert(id("core:ore"), 100);
    d.systems[0].sources = vec![SourceDefinition {
        good: id("core:ore"),
        quantity_per_tick: 100,
        extraction_energy: Energy::ZERO,
    }];
    d.systems[0].energy_output_per_tick = Energy::ZERO;
    d.systems[0].seasonal_generation.base_output = Energy::ZERO;
    d.systems[0].seasonal_generation.current_effective_output = Energy::ZERO;
    let mut session = GameSession::new(d).unwrap();
    let entity = session.market_entity(&id("core:s0")).unwrap();
    {
        let mut market = session.world.get_mut::<Market>(entity).unwrap();
        market.brownout.stage = BrownoutStage::Starvation;
        market.operating_profile.stage = BrownoutStage::Starvation;
        market.last_life_support_unsupplied = Energy(100);
    }
    session.update_populations().unwrap();
    {
        let market = session.world.get::<Market>(entity).unwrap();
        assert_eq!(market.population, 90);
        assert_eq!(market.population_state.current, 90);
        assert_eq!(market.operating_profile.labor_percent, 90);
        assert_eq!(market.targets[&id("core:ore")], 90);
    }

    {
        let mut market = session.world.get_mut::<Market>(entity).unwrap();
        market.seasonal_generation.base_output = Energy(1_000);
        market.energy_output_per_tick = Energy(1_000);
        market.brownout.stage = BrownoutStage::Normal;
        market.brownout.entered_at_tick = 0;
    }
    session.step().unwrap();
    let market = session.world.get::<Market>(entity).unwrap();
    assert_eq!(market.energy_flow.life_support_burned, Energy(90));
    assert_eq!(market.operating_profile.labor_percent, 90);
    assert_eq!(market.targets[&id("core:ore")], 90);
    assert_eq!(market.inventory[&id("core:ore")], 145);
    assert_eq!(market.brownout.stage, BrownoutStage::Throttled);
}

#[test]
fn population_updates_are_atomic_and_insertion_order_invariant() {
    let mut d = definition();
    d.economy.population.static_population = false;
    d.economy.population.sufficiency_window = 1;
    d.economy.population.essential_goods = BTreeSet::from([id(ENERGY_ID)]);
    d.economy.population.tertiary_demand_per_thousand.clear();
    d.economy.population.decline_per_thousand = 10;
    d.economy.population.growth_per_thousand = 1;
    for system in &mut d.systems {
        system.population = 10;
        system.population_state = PopulationState {
            current: 10,
            reference: 10,
            carrying_capacity: 10,
            support_capacity: 10,
            ..PopulationState::default()
        };
    }
    let mut reversed = d.clone();
    reversed.systems.reverse();
    let mut left = GameSession::new(d).unwrap();
    let mut right = GameSession::new(reversed).unwrap();
    for _ in 0..100 {
        left.step().unwrap();
        right.step().unwrap();
        left.drain_events();
        right.drain_events();
    }
    let left_population = left
        .snapshot()
        .markets
        .into_iter()
        .map(|market| (market.system_id, market.population_state))
        .collect::<Vec<_>>();
    let right_population = right
        .snapshot()
        .markets
        .into_iter()
        .map(|market| (market.system_id, market.population_state))
        .collect::<Vec<_>>();
    assert_eq!(left_population, right_population);

    let entity = left.market_entity(&id("core:s0")).unwrap();
    {
        let mut market = left.world.get_mut::<Market>(entity).unwrap();
        market.brownout.stage = BrownoutStage::Starvation;
        market.last_life_support_unsupplied = Energy(10);
        market.population_state.decline_remainder = 990;
    }
    left.world
        .resource_mut::<AggregateDynamicsHistory>()
        .population_changes = u64::MAX;
    let before = format!("{:?}", left.snapshot());
    assert_eq!(left.update_populations(), Err(CoreError::Overflow));
    assert_eq!(format!("{:?}", left.snapshot()), before);
    assert!(left.drain_events().is_empty());
}

fn enable_investments(definition: &mut GameDefinition) {
    definition.economy.life_support_burn_per_capita = Energy::ZERO;
    for (kind, effect) in [
        (InvestmentKind::Collector, 1),
        (InvestmentKind::Storage, 100),
        (InvestmentKind::PopulationSupport, 5),
        (InvestmentKind::RouteSubsidy, 10),
    ] {
        definition.economy.investments.insert(
            kind,
            InvestmentShape {
                enabled: true,
                base_cost: Energy(100),
                cost_growth_percent: 150,
                maximum_level: 2,
                cooldown_ticks: 2,
                effect_per_level: effect,
            },
        );
    }
    for system in &mut definition.systems {
        system.energy_output_per_tick = Energy::ZERO;
        system.seasonal_generation.base_output = Energy::ZERO;
        system.seasonal_generation.current_effective_output = Energy::ZERO;
    }
}

#[test]
fn autonomous_investments_use_stable_ties_exact_costs_cooldowns_caps_and_protection() {
    let mut d = definition();
    enable_investments(&mut d);
    d.systems[0].investment_policy = InvestmentPolicy {
        allocation_percent: BTreeMap::from([
            (InvestmentKind::Collector, 50),
            (InvestmentKind::Storage, 50),
        ]),
    };
    let mut session = GameSession::new(d).unwrap();
    let entity = session.market_entity(&id("core:s0")).unwrap();

    session.execute_autonomous_investments().unwrap();
    let first = session.world.get::<Market>(entity).unwrap();
    assert_eq!(first.investment_state.levels[&InvestmentKind::Collector], 1);
    assert!(
        !first
            .investment_state
            .levels
            .contains_key(&InvestmentKind::Storage)
    );
    assert_eq!(first.energy_stock().unwrap(), Energy(900));
    assert_eq!(first.seasonal_generation.base_output, Energy(1));

    session.execute_autonomous_investments().unwrap();
    let second = session.world.get::<Market>(entity).unwrap();
    assert_eq!(second.investment_state.levels[&InvestmentKind::Storage], 1);
    assert_eq!(second.energy_storage_cap, Energy(2_100));
    assert_eq!(second.energy_stock().unwrap(), Energy(800));
    session.world.resource_mut::<Clock>().0 = 2;
    session.execute_autonomous_investments().unwrap();
    let third = session.world.get::<Market>(entity).unwrap();
    assert_eq!(third.investment_state.levels[&InvestmentKind::Collector], 2);
    assert_eq!(third.energy_stock().unwrap(), Energy(650));
    assert_eq!(third.seasonal_generation.base_output, Energy(2));
    session.world.resource_mut::<Clock>().0 = 4;
    session.execute_autonomous_investments().unwrap();
    assert_eq!(
        session
            .world
            .get::<Market>(entity)
            .unwrap()
            .investment_state
            .status[&InvestmentKind::Collector],
        InvestmentStatus::MaximumLevel
    );

    let mut constrained = definition();
    enable_investments(&mut constrained);
    constrained.systems[0].inventory.insert(id(ENERGY_ID), 119);
    constrained.systems[0].investment_policy = InvestmentPolicy {
        allocation_percent: BTreeMap::from([(InvestmentKind::Collector, 100)]),
    };
    let mut constrained = GameSession::new(constrained).unwrap();
    let constrained_entity = constrained.market_entity(&id("core:s0")).unwrap();
    constrained.execute_autonomous_investments().unwrap();
    let market = constrained.world.get::<Market>(constrained_entity).unwrap();
    assert_eq!(market.energy_stock().unwrap(), Energy(119));
    assert_eq!(market.protected_liquidation_budget, Energy(20));
    assert_eq!(
        market.investment_state.status[&InvestmentKind::Collector],
        InvestmentStatus::InsufficientFunds {
            available: Energy(99),
            cost: Energy(100),
        }
    );

    {
        let mut market = constrained
            .world
            .get_mut::<Market>(constrained_entity)
            .unwrap();
        market.set_energy_stock(Energy(1_000)).unwrap();
        market.operating_profile.stage = BrownoutStage::Emergency;
        market.operating_profile.investment_allowed = false;
    }
    constrained.execute_autonomous_investments().unwrap();
    let market = constrained.world.get::<Market>(constrained_entity).unwrap();
    assert_eq!(market.energy_stock().unwrap(), Energy(1_000));
    assert_eq!(
        market.investment_state.status[&InvestmentKind::Collector],
        InvestmentStatus::DisabledByStage(BrownoutStage::Emergency)
    );
}

#[test]
fn selected_investment_spend_recomputes_other_ready_statuses() {
    let mut d = definition();
    enable_investments(&mut d);
    d.systems[0].investment_policy = InvestmentPolicy {
        allocation_percent: BTreeMap::from([
            (InvestmentKind::Collector, 50),
            (InvestmentKind::Storage, 50),
        ]),
    };
    let mut session = GameSession::new(d).unwrap();
    let entity = session.market_entity(&id("core:s0")).unwrap();
    {
        let mut market = session.world.get_mut::<Market>(entity).unwrap();
        market.protected_liquidation_budget = Energy::ZERO;
        market.set_energy_stock(Energy(100)).unwrap();
    }

    session.execute_autonomous_investments().unwrap();
    let market = session.world.get::<Market>(entity).unwrap();
    assert_eq!(market.energy_stock().unwrap(), Energy::ZERO);
    assert!(matches!(
        market.investment_state.status[&InvestmentKind::Collector],
        InvestmentStatus::Completed {
            tick: 0,
            cost: Energy(100)
        }
    ));
    assert_eq!(
        market.investment_state.status[&InvestmentKind::Storage],
        InvestmentStatus::InsufficientFunds {
            available: Energy::ZERO,
            cost: Energy(100),
        }
    );
}

#[test]
fn investment_effects_are_atomic_and_subsidy_suppression_resumes_without_reauthorization() {
    let mut d = definition();
    enable_investments(&mut d);
    d.economy
        .investments
        .get_mut(&InvestmentKind::Collector)
        .unwrap()
        .effect_per_level = 10;
    d.systems[0].seasonal_generation.amplitude_percent = 20;
    d.systems[0].seasonal_generation.period_ticks = 4;
    d.systems[0].investment_policy = InvestmentPolicy {
        allocation_percent: BTreeMap::from([(InvestmentKind::Collector, 100)]),
    };
    let mut session = GameSession::new(d).unwrap();
    session.step().unwrap();
    let first = session.snapshot().markets.remove(0);
    assert_eq!(first.seasonal_generation.base_output, Energy(10));
    assert_eq!(
        first.seasonal_generation.current_effective_output,
        Energy::ZERO
    );
    session.step().unwrap();
    let second = session.snapshot().markets.remove(0);
    assert_eq!(
        second.seasonal_generation.current_effective_output,
        Energy(10)
    );

    let mut population = definition();
    enable_investments(&mut population);
    population.systems[0].investment_policy = InvestmentPolicy {
        allocation_percent: BTreeMap::from([(InvestmentKind::PopulationSupport, 100)]),
    };
    let mut population = GameSession::new(population).unwrap();
    let population_entity = population.market_entity(&id("core:s0")).unwrap();
    let before_population = population
        .world
        .get::<Market>(population_entity)
        .unwrap()
        .population;
    population.execute_autonomous_investments().unwrap();
    let supported = population.world.get::<Market>(population_entity).unwrap();
    assert_eq!(supported.population, before_population);
    assert_eq!(supported.population_state.support_capacity, 6);
    assert_eq!(supported.population_state.growth_rate_bonus_percent, 5);
    assert_eq!(supported.population_state.carrying_capacity, 1);

    let mut subsidy = definition();
    enable_investments(&mut subsidy);
    subsidy.economy.life_support_burn_per_capita = Energy(1);
    let mut subsidy = GameSession::new(subsidy).unwrap();
    let destination = subsidy.market_entity(&id("core:s1")).unwrap();
    let ore = id("core:ore");
    let normal_bid = subsidy.quotes(&id("core:s1"), &ore).unwrap().0;
    {
        let mut market = subsidy.world.get_mut::<Market>(destination).unwrap();
        market
            .investment_state
            .levels
            .insert(InvestmentKind::RouteSubsidy, 1);
    }
    let premium_bid = subsidy.quotes(&id("core:s1"), &ore).unwrap().0;
    assert!(premium_bid > normal_bid);
    {
        let mut market = subsidy.world.get_mut::<Market>(destination).unwrap();
        market.set_energy_stock(Energy(6)).unwrap();
    }
    subsidy.classify_brownouts().unwrap();
    assert_eq!(
        subsidy
            .world
            .get::<Market>(destination)
            .unwrap()
            .brownout
            .stage,
        BrownoutStage::Emergency
    );
    assert_eq!(
        subsidy.quotes(&id("core:s1"), &ore).unwrap().0,
        Energy::ZERO
    );
    assert_eq!(
        subsidy.market_demand(&id("core:s1"), &ore).unwrap(),
        MarketDemandSnapshot::default()
    );
    assert_eq!(
        subsidy
            .world
            .get::<Market>(destination)
            .unwrap()
            .reserved_energy,
        Energy::ZERO
    );
    {
        let mut market = subsidy.world.get_mut::<Market>(destination).unwrap();
        market.set_energy_stock(Energy(1_000)).unwrap();
    }
    subsidy.world.resource_mut::<Clock>().0 = 1;
    subsidy.classify_brownouts().unwrap();
    assert_eq!(
        subsidy
            .world
            .get::<Market>(destination)
            .unwrap()
            .brownout
            .stage,
        BrownoutStage::Throttled
    );
    assert_eq!(subsidy.quotes(&id("core:s1"), &ore).unwrap().0, premium_bid);
    let player = subsidy.player_entity().unwrap();
    let quantity = subsidy
        .create_reservation(player, &id("core:s1"), &ore, 2)
        .unwrap();
    assert_eq!(
        subsidy
            .world
            .get::<Market>(destination)
            .unwrap()
            .reserved_energy,
        premium_bid.checked_mul(u64::from(quantity)).unwrap()
    );

    let mut opportunity_definition = definition();
    enable_investments(&mut opportunity_definition);
    opportunity_definition.fleet = dynamic_fleet(0, 2, 1, 2);
    let mut without_subsidy = GameSession::new(opportunity_definition.clone()).unwrap();
    let mut with_subsidy = GameSession::new(opportunity_definition).unwrap();
    let opportunity_destination = with_subsidy.market_entity(&id("core:s1")).unwrap();
    with_subsidy
        .world
        .get_mut::<Market>(opportunity_destination)
        .unwrap()
        .investment_state
        .levels
        .insert(InvestmentKind::RouteSubsidy, 1);
    without_subsidy.collect_automated_trader_requests().unwrap();
    with_subsidy.collect_automated_trader_requests().unwrap();
    assert!(
        with_subsidy
            .world
            .resource::<FleetDynamics>()
            .normalized_unserved_opportunity
            > without_subsidy
                .world
                .resource::<FleetDynamics>()
                .normalized_unserved_opportunity
    );

    let mut overflow = definition();
    enable_investments(&mut overflow);
    overflow.systems[0].seasonal_generation.base_output = Energy(i64::MAX);
    overflow.systems[0]
        .seasonal_generation
        .current_effective_output = Energy(i64::MAX);
    overflow.systems[0].energy_output_per_tick = Energy(i64::MAX);
    overflow.systems[0].investment_policy = InvestmentPolicy {
        allocation_percent: BTreeMap::from([(InvestmentKind::Collector, 100)]),
    };
    assert!(matches!(
        GameSession::new(overflow),
        Err(CoreError::InvalidWorldDynamics)
    ));
}

#[test]
fn population_scaled_targets_are_effective_in_the_initial_snapshot() {
    let ore = id("core:ore");
    let mut dynamic = definition();
    dynamic.economy.population.static_population = false;
    dynamic
        .economy
        .population
        .tertiary_demand_per_thousand
        .insert(ore.clone(), 2);
    dynamic.systems[0].population = 10;
    dynamic.systems[0].population_state.current = 10;
    dynamic.systems[0].population_state.reference = 5;
    dynamic.systems[0].population_state.carrying_capacity = 10;
    dynamic.systems[0].population_state.support_capacity = 10;
    dynamic.systems[1].population = 10;
    dynamic.systems[1].population_state.current = 10;
    dynamic.systems[1].population_state.reference = 5;
    dynamic.systems[1].population_state.carrying_capacity = 10;
    dynamic.systems[1].population_state.support_capacity = 10;
    dynamic.systems[1].targets.remove(&ore);
    let mut session = GameSession::new(dynamic).unwrap();
    let snapshot = session.snapshot();
    assert_eq!(snapshot.markets[0].authored_targets[&ore], 10);
    assert_eq!(snapshot.markets[0].targets[&ore], 20);
    assert!(!snapshot.markets[1].authored_targets.contains_key(&ore));
    assert_eq!(snapshot.markets[1].targets[&ore], 1);

    let mut static_definition = definition();
    static_definition
        .economy
        .population
        .tertiary_demand_per_thousand
        .insert(ore.clone(), 2);
    static_definition.systems[0].targets.remove(&ore);
    static_definition.systems[0].population = 10;
    let mut static_session = GameSession::new(static_definition).unwrap();
    assert_eq!(static_session.snapshot().markets[0].targets[&ore], 1);

    let mut zero_population = definition();
    zero_population.economy.population.static_population = false;
    zero_population
        .economy
        .population
        .tertiary_demand_per_thousand
        .insert(ore.clone(), 2);
    zero_population.systems[0].targets.remove(&ore);
    zero_population.systems[0].population = 0;
    zero_population.systems[0].population_state.current = 0;
    zero_population.systems[0].population_state.reference = 1;
    zero_population.systems[0]
        .population_state
        .carrying_capacity = 0;
    zero_population.systems[0].population_state.support_capacity = 0;
    let mut zero_session = GameSession::new(zero_population).unwrap();
    assert_eq!(zero_session.snapshot().markets[0].targets[&ore], 0);
    assert!(zero_session.quotes(&id("core:s0"), &ore).is_ok());
    zero_session.step().unwrap();
}

#[test]
fn governor_market_targets_are_authorized_immediate_and_persistent() {
    let ore = id("core:ore");
    let mut definition = definition();
    definition.systems[1].governance = Governance::default();
    definition
        .economy
        .population
        .tertiary_demand_per_thousand
        .insert(ore.clone(), 1);
    let mut session = GameSession::new(definition).unwrap();

    session
        .submit(GameCommand::SetGovernorMarketTarget {
            system: id("core:s0"),
            good: ore.clone(),
            target: 200,
        })
        .unwrap();
    let market = session
        .snapshot()
        .markets
        .into_iter()
        .find(|market| market.system_id == id("core:s0"))
        .unwrap();
    assert_eq!(market.targets[&ore], 200);
    assert_eq!(market.demand[&ore].advertised, 100);
    assert!(session.drain_events().iter().any(|event| matches!(
        event,
        GameEvent::MarketTargetChanged { system, good, target }
            if system == &id("core:s0") && good == &ore && *target == 200
    )));

    session.step().unwrap();
    let after_step = session
        .snapshot()
        .markets
        .into_iter()
        .find(|market| market.system_id == id("core:s0"))
        .unwrap();
    assert_eq!(after_step.targets[&ore], 200);

    let before = session.snapshot();
    assert_eq!(
        session.submit(GameCommand::SetGovernorMarketTarget {
            system: id("core:s0"),
            good: ore.clone(),
            target: 0,
        }),
        Err(CoreError::InvalidMarketTarget)
    );
    assert_eq!(
        session.submit(GameCommand::SetGovernorMarketTarget {
            system: id("core:s1"),
            good: ore,
            target: 50,
        }),
        Err(CoreError::UnauthorizedMarketPolicy)
    );
    assert!(matches!(
        session.submit(GameCommand::SetGovernorMarketTarget {
            system: id("core:s0"),
            good: id("core:missing"),
            target: 50,
        }),
        Err(CoreError::Unknown { kind: "good", .. })
    ));
    assert!(session.drain_events().iter().any(|event| matches!(
        event,
        GameEvent::GovernorPolicyRejected {
            reason: GovernorRejectionReason::UnknownGood,
            ..
        }
    )));
    assert_eq!(session.snapshot().markets, before.markets);
}

#[test]
fn governor_policy_edits_merge_only_approved_fields() {
    let mut d = definition();
    d.systems[0].policy.pricing_mode = PricingMode::Scarcity;
    d.systems[0].policy.liquidation_threshold_percent = 175;
    d.systems[0].policy.liquidation_discount_percent = 40;
    d.systems[0].policy.default_target = 77;
    let mut session = GameSession::new(d).unwrap();
    session
        .submit(GameCommand::SetGovernorMarketPolicy {
            system: id("core:s0"),
            policy: GovernorMarketPolicy {
                producer_margin_percent: 25,
                operating_reserve_ticks: 4,
                import_priorities: BTreeMap::from([(id("core:ore"), 125)]),
            },
        })
        .unwrap();
    let policy = &session.snapshot().markets[0].policy;
    assert_eq!(policy.producer_margin_percent, 25);
    assert_eq!(policy.operating_reserve_ticks, 4);
    assert_eq!(policy.import_priorities[&id("core:ore")], 125);
    assert_eq!(policy.pricing_mode, PricingMode::Scarcity);
    assert_eq!(policy.liquidation_threshold_percent, 175);
    assert_eq!(policy.liquidation_discount_percent, 40);
    assert_eq!(policy.default_target, 77);
}

#[test]
fn governor_authorization_is_typed_and_ai_and_player_use_the_same_executor() {
    let mut d = definition();
    enable_investments(&mut d);
    d.systems[1].governance = Governance::default();
    for system in &mut d.systems {
        system.investment_policy = InvestmentPolicy {
            allocation_percent: BTreeMap::from([(InvestmentKind::Storage, 100)]),
        };
    }
    let mut session = GameSession::new(d).unwrap();
    assert_eq!(
        session.submit(GameCommand::SetInvestmentPolicy {
            system: id("core:s1"),
            policy: InvestmentPolicy::default(),
        }),
        Err(CoreError::UnauthorizedMarketPolicy)
    );
    assert!(matches!(
        session.drain_events().as_slice(),
        [GameEvent::GovernorPolicyRejected {
            reason: GovernorRejectionReason::Unauthorized,
            ..
        }]
    ));
    session
        .submit(GameCommand::SetInvestmentPolicy {
            system: id("core:s0"),
            policy: InvestmentPolicy {
                allocation_percent: BTreeMap::from([(InvestmentKind::Storage, 100)]),
            },
        })
        .unwrap();
    session.drain_events();
    session.execute_autonomous_investments().unwrap();
    let snapshot = session.snapshot();
    assert!(snapshot.markets.iter().all(|market| {
        market.investment_state.levels.get(&InvestmentKind::Storage) == Some(&1)
    }));
    assert_eq!(
        session
            .drain_events()
            .iter()
            .filter(|event| matches!(event, GameEvent::InvestmentCompleted { .. }))
            .count(),
        2
    );
}

#[test]
fn brownout_history_overflow_is_atomic() {
    let mut session = GameSession::new(definition()).unwrap();
    session
        .world
        .resource_mut::<AggregateDynamicsHistory>()
        .stage_occupancy_ticks[BrownoutStage::Normal.index()] = u64::MAX;
    let before = format!("{:?}", session.snapshot());
    assert_eq!(session.classify_brownouts(), Err(CoreError::Overflow));
    assert_eq!(format!("{:?}", session.snapshot()), before);
    assert!(session.drain_events().is_empty());
}
