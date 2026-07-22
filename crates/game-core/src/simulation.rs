use crate::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SimulationTime {
    pub tick: u64,
}

impl WorldState {
    /// Executes one atomic, phase-major tick over every system in stable system-ID order.
    pub fn advance_tick(&mut self) -> Result<PlayerWorldView, CoreError> {
        let mut candidate = self.clone_full();
        candidate.run_tick()?;
        let view = candidate.player_view()?;
        *self = candidate;
        Ok(view)
    }

    fn run_tick(&mut self) -> Result<(), CoreError> {
        self.validate_runtime_integrity()?;
        let ids = self.systems.keys().cloned().collect::<Vec<_>>();

        // 1. Finalize Habitat generation that became ready on a prior tick.
        for id in &ids {
            self.habitat_phase(id, true)?;
        }
        // 2. Collector production.
        for id in &ids {
            self.collector_phase(id)?;
        }
        // 3. Life support and supported-population work.
        for id in &ids {
            self.life_support_phase(id)?;
        }
        // 4. Extractor operation.
        for id in &ids {
            self.extractor_phase(id)?;
        }
        // 5. Refinery operation.
        for id in &ids {
            self.refinery_phase(id)?;
        }
        // 6. Shipyard project progress.
        for id in &ids {
            let system = self.systems.get_mut(id).expect("stable key exists");
            progress_shipyards(ShipyardPhaseContext {
                time: self.time,
                bodies: &mut system.bodies,
                stocks: &mut system.stocks,
                completed_assets: &mut system.completed_assets,
                accounting: &mut system.accounting,
                tuning: &self.tuning,
            })?;
        }
        // 7. Enabled empty-Habitat accumulation.
        for id in &ids {
            self.habitat_phase(id, false)?;
        }
        // 8. General construction work.
        for id in &ids {
            self.construction_phase(id)?;
        }
        // 9. Movement, arrival/loss, observations, and due transmissions.
        move_ships(MovementPhaseContext {
            time: self.time,
            origin_system: &self.origin_system,
            locations: &self.locations,
            map_systems: &self.map_systems,
            systems: &mut self.systems,
            communities: &mut self.communities,
            populations: &mut self.populations,
            population_accounting: &mut self.population_accounting,
            ships: &mut self.transit,
            knowledge: &mut self.knowledge,
            tuning: &self.tuning,
        })?;
        self.knowledge
            .receive_due(self.time.tick)
            .map_err(|error| CoreError::KnowledgeIntegration(error.to_string()))?;
        self.sync_received_founding_outcomes()?;
        // 10. Energy retention and overflow.
        for id in &ids {
            self.retention_phase(id)?;
        }
        self.validate_runtime_integrity()?;
        self.time.tick = self.time.tick.checked_add(1).ok_or(CoreError::Overflow)?;
        Ok(())
    }

    fn habitat_phase(&mut self, id: &ContentId, finalize: bool) -> Result<(), CoreError> {
        let community_id = self
            .communities
            .values()
            .find(|community| &community.system == id)
            .map(|community| community.id.clone());
        let system = self.systems.get_mut(id).expect("stable key exists");
        let energy_before = system.stocks.quantity(&self.tuning.energy_resource);
        let context = HabitatPhaseContext {
            time: self.time,
            system_id: id,
            community_id: community_id.as_ref(),
            bodies: &mut system.bodies,
            stocks: &mut system.stocks,
            populations: &mut self.populations,
            population_accounting: &mut self.population_accounting,
            next_population_sequence: &mut system.counters.next_population_sequence,
            tuning: &self.tuning,
        };
        if finalize {
            finalize_population_generation(context)
        } else {
            accumulate_population_generation(context)?;
            let spent = energy_before
                .checked_sub(system.stocks.quantity(&self.tuning.energy_resource))
                .ok_or(CoreError::Overflow)?;
            if spent != 0 {
                add(
                    &mut system.accounting.operation_spent,
                    &self.tuning.energy_resource,
                    spent,
                )?;
            }
            Ok(())
        }
    }

    fn collector_phase(&mut self, id: &ContentId) -> Result<(), CoreError> {
        let system = self.systems.get_mut(id).expect("stable key exists");
        let map = self
            .map_systems
            .get(id)
            .expect("runtime system has immutable map definition");
        let phase = usize::try_from(self.time.tick % 10).expect("phase below ten");
        let energy = self.tuning.energy_resource.clone();
        let coordinates = functional_coordinates(system, DevelopmentRole::Collector);
        let mut produced = 0_u64;
        for (body_index, _) in coordinates {
            let eccentricity = map.bodies[body_index].eccentricity_hundredths;
            let profile =
                collector_profile(map.stellar_strength_hundredths, eccentricity, &self.tuning)?;
            produced = produced
                .checked_add(profile[phase])
                .ok_or(CoreError::Overflow)?;
        }
        add(&mut system.stocks, &energy, produced)?;
        add(&mut system.accounting.produced, &energy, produced)
    }

    fn life_support_phase(&mut self, id: &ContentId) -> Result<(), CoreError> {
        let community_id = self
            .communities
            .values()
            .find(|community| &community.system == id)
            .map(|community| &community.id);
        let system = self.systems.get_mut(id).expect("stable key exists");
        system.life_support = derive_life_support(LifeSupportPhaseContext {
            is_origin: id == &self.origin_system,
            community_id,
            bodies: &system.bodies,
            stocks: &mut system.stocks,
            populations: &self.populations,
            resource_accounting: &mut system.accounting,
            tuning: &self.tuning,
        })?;
        Ok(())
    }

    fn extractor_phase(&mut self, id: &ContentId) -> Result<(), CoreError> {
        let system = self.systems.get_mut(id).expect("stable key exists");
        let energy = self.tuning.energy_resource.clone();
        let coordinates = functional_coordinates(system, DevelopmentRole::Extractor);
        for (body_index, slot_index) in coordinates {
            let target = system.bodies[body_index].slots[slot_index]
                .development
                .as_ref()
                .and_then(|development| development.definition.extractor_target.clone())
                .ok_or(CoreError::ExtractorTargetRequired)?;
            if target.body != system.bodies[body_index].id {
                return Err(CoreError::IncompatibleExtractorTarget {
                    body: target.body,
                    resource: target.resource,
                });
            }
            if system.bodies[body_index]
                .remaining_resources
                .quantity(&target.resource)
                < self.tuning.extractor.output
                || system.stocks.quantity(&energy) < self.tuning.extractor.energy_upkeep
            {
                continue;
            }
            sub(
                &mut system.stocks,
                &energy,
                self.tuning.extractor.energy_upkeep,
            )?;
            add(
                &mut system.accounting.operation_spent,
                &energy,
                self.tuning.extractor.energy_upkeep,
            )?;
            let completes = {
                let cycle = &mut system.bodies[body_index].slots[slot_index]
                    .development
                    .as_mut()
                    .expect("coordinate has development")
                    .cycle;
                cycle.progress = cycle.progress.checked_add(1).ok_or(CoreError::Overflow)?;
                cycle.progress == self.tuning.extractor.cycle_duration
            };
            if completes {
                sub(
                    &mut system.bodies[body_index].remaining_resources,
                    &target.resource,
                    self.tuning.extractor.output,
                )?;
                add(
                    &mut system.stocks,
                    &target.resource,
                    self.tuning.extractor.output,
                )?;
                add(
                    &mut system.accounting.produced,
                    &target.resource,
                    self.tuning.extractor.output,
                )?;
                system.bodies[body_index].slots[slot_index]
                    .development
                    .as_mut()
                    .expect("coordinate has development")
                    .cycle
                    .progress = 0;
            }
        }
        Ok(())
    }

    fn refinery_phase(&mut self, id: &ContentId) -> Result<(), CoreError> {
        let system = self.systems.get_mut(id).expect("stable key exists");
        let energy = self.tuning.energy_resource.clone();
        let input = self.tuning.ore_resource.clone();
        let output = self.tuning.alloy_resource.clone();
        for (body_index, slot_index) in functional_coordinates(system, DevelopmentRole::Refinery) {
            let cycle = &system.bodies[body_index].slots[slot_index]
                .development
                .as_ref()
                .expect("coordinate has development")
                .cycle;
            let idle = cycle.progress == 0 && cycle.committed_inputs.quantity(&input) == 0;
            if system.stocks.quantity(&energy) < self.tuning.refinery.energy_upkeep
                || (idle && system.stocks.quantity(&input) < self.tuning.refinery.input)
            {
                continue;
            }
            sub(
                &mut system.stocks,
                &energy,
                self.tuning.refinery.energy_upkeep,
            )?;
            add(
                &mut system.accounting.operation_spent,
                &energy,
                self.tuning.refinery.energy_upkeep,
            )?;
            let cycle = &mut system.bodies[body_index].slots[slot_index]
                .development
                .as_mut()
                .expect("coordinate has development")
                .cycle;
            if idle {
                sub(&mut system.stocks, &input, self.tuning.refinery.input)?;
                cycle
                    .committed_inputs
                    .set(input.clone(), self.tuning.refinery.input);
            }
            cycle.progress = cycle.progress.checked_add(1).ok_or(CoreError::Overflow)?;
            if cycle.progress == self.tuning.refinery.cycle_duration {
                add(
                    &mut system.accounting.operation_spent,
                    &input,
                    self.tuning.refinery.input,
                )?;
                add(&mut system.stocks, &output, self.tuning.refinery.output)?;
                add(
                    &mut system.accounting.produced,
                    &output,
                    self.tuning.refinery.output,
                )?;
                cycle.progress = 0;
                cycle.committed_inputs = ResourceStore::new();
            }
        }
        Ok(())
    }

    fn construction_phase(&mut self, id: &ContentId) -> Result<(), CoreError> {
        let system = self.systems.get_mut(id).expect("stable key exists");
        let mut work = system.life_support.construction_work;
        while work != 0 && !system.construction_queue.is_empty() {
            let needed = system.construction_queue[0].required_work
                - system.construction_queue[0].work_applied;
            let applied = needed.min(work);
            system.construction_queue[0].work_applied = system.construction_queue[0]
                .work_applied
                .checked_add(applied)
                .ok_or(CoreError::Overflow)?;
            work -= applied;
            if system.construction_queue[0].work_applied
                == system.construction_queue[0].required_work
            {
                let item = system.construction_queue.remove(0);
                for (resource, quantity) in &item.committed_resources.quantities {
                    add(
                        &mut system.accounting.construction_spent,
                        resource,
                        *quantity,
                    )?;
                }
                let slot = find_slot_mut(&mut system.bodies, &item.body, &item.slot)?;
                if slot.reserved_by != Some(ReservationOwner::Construction(item.id.clone()))
                    || slot.development.is_some()
                {
                    return Err(CoreError::InvalidConstructionReservation(item.id));
                }
                let definition = DevelopmentDefinition {
                    id: item.development_id,
                    role: item.role,
                    condition: DevelopmentCondition::Functional,
                    extractor_target: item.extractor_target,
                };
                slot.development = Some(DevelopmentState {
                    enabled: true,
                    habitat: (definition.role == DevelopmentRole::Habitat)
                        .then(HabitatState::default),
                    shipyard: (definition.role == DevelopmentRole::Shipyard)
                        .then(ShipyardState::default),
                    definition,
                    cycle: ProductionCycle::default(),
                });
                slot.reserved_by = None;
            }
        }
        Ok(())
    }

    fn sync_received_founding_outcomes(&mut self) -> Result<(), CoreError> {
        let founded_targets = self
            .knowledge
            .mission_states
            .values()
            .filter_map(|state| match state {
                MissionState::Founded { target, .. } => Some(target.clone()),
                MissionState::AwaitingOutcome { .. } | MissionState::FoundingLost { .. } => None,
            })
            .collect::<Vec<_>>();
        for target in founded_targets {
            self.unlock_remote_commands(&target)?;
        }
        Ok(())
    }

    fn retention_phase(&mut self, id: &ContentId) -> Result<(), CoreError> {
        let system = self.systems.get_mut(id).expect("stable key exists");
        system.overflow.last_tick_retention = 0;
        let capacity = energy_capacity(system, &self.tuning)?;
        let available = system.stocks.quantity(&self.tuning.energy_resource);
        if available > capacity {
            let overflow = available - capacity;
            system
                .stocks
                .set(self.tuning.energy_resource.clone(), capacity);
            system.overflow.last_tick_retention = overflow;
            record_overflow(
                &mut system.overflow,
                self.time,
                EnergyOverflowCause::Retention,
                overflow,
            )?;
        }
        Ok(())
    }
}

fn functional_coordinates(system: &SystemState, role: DevelopmentRole) -> Vec<(usize, usize)> {
    system
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
                            development.definition.role == role
                                && development.definition.condition
                                    == DevelopmentCondition::Functional
                                && development.enabled
                        })
                        .then_some((body_index, slot_index))
                })
        })
        .collect()
}

/// Exact strength budget plus largest-remainder eccentricity apportionment.
pub(crate) fn collector_profile(
    strength_hundredths: u16,
    eccentricity_hundredths: u16,
    tuning: &WorldTuning,
) -> Result<[u64; 10], CoreError> {
    let baseline = tuning.seasonal_baseline_average;
    let cycle_numerator = 280_u128
        .checked_mul(u128::from(strength_hundredths))
        .ok_or(CoreError::Overflow)?;
    let mut budget = cycle_numerator / 100;
    if cycle_numerator % 100 >= 80 {
        budget = budget.checked_add(1).ok_or(CoreError::Overflow)?;
    }
    let denominator = u128::from(baseline)
        .checked_mul(1000)
        .ok_or(CoreError::Overflow)?;
    let mut result = [0_u64; 10];
    let mut remainders = Vec::with_capacity(10);
    let mut assigned = 0_u128;
    for (phase, shape) in tuning.seasonal_shape.iter().enumerate() {
        let deviation = i128::from(*shape) - i128::from(baseline);
        let weight = i128::from(baseline.checked_mul(100).ok_or(CoreError::Overflow)?)
            + i128::from(eccentricity_hundredths) * deviation;
        if weight < 0 {
            return Err(CoreError::InvalidTuning(
                "seasonal weight must be nonnegative".into(),
            ));
        }
        let numerator = budget
            .checked_mul(u128::try_from(weight).map_err(|_| CoreError::Overflow)?)
            .ok_or(CoreError::Overflow)?;
        let floor = numerator / denominator;
        result[phase] = u64::try_from(floor).map_err(|_| CoreError::Overflow)?;
        assigned = assigned.checked_add(floor).ok_or(CoreError::Overflow)?;
        remainders.push((numerator % denominator, phase));
    }
    let remaining = usize::try_from(budget.checked_sub(assigned).ok_or(CoreError::Overflow)?)
        .map_err(|_| CoreError::Overflow)?;
    remainders.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    for (_, phase) in remainders.into_iter().take(remaining) {
        result[phase] = result[phase].checked_add(1).ok_or(CoreError::Overflow)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
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
                speed_quanta_per_tick: 10,
                energy_per_quantum: FixedRate::new(1, NonZeroU64::new(1).unwrap()),
            },
            expedition_travel: ShipTravelTuning {
                maximum_jump_quanta: 10,
                speed_quanta_per_tick: 10,
                energy_per_quantum: FixedRate::new(1, NonZeroU64::new(1).unwrap()),
            },
            probe_reveal_radius_quanta: 10,
            communication_delay_per_quantum: FixedRate::new(1, NonZeroU64::new(1).unwrap()),
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

    fn fixture() -> WorldState {
        let origin = id("core:origin");
        let target = id("core:target");
        let community = id("core:origin_community");
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
                    position: Position3::from_quanta(1, 0, 0),
                },
            ],
            origin_system: origin.clone(),
            origin_community: community.clone(),
            communities: vec![CommunityDefinition {
                id: community.clone(),
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
                        slots: vec![
                            slot(
                                "core:collector_slot",
                                Some(development(
                                    "core:origin_collector",
                                    DevelopmentRole::Collector,
                                )),
                            ),
                            slot(
                                "core:habitat_slot",
                                Some(development("core:origin_habitat", DevelopmentRole::Habitat)),
                            ),
                            slot(
                                "core:yard_slot",
                                Some(development("core:origin_yard", DevelopmentRole::Shipyard)),
                            ),
                        ],
                    }],
                    stocks: store(&[
                        ("core:energy", 1_000),
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
        world.advance_tick().unwrap();
        world.advance_tick().unwrap();
        world
    }

    #[test]
    fn forced_movement_failure_rolls_back_earlier_phases_clock_and_counters() {
        let mut world = fixture();
        let ids = world
            .enqueue_ship_project(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:yard_slot"),
                ShipProjectKind::Expedition,
            )
            .unwrap();
        world.advance_tick().unwrap();
        world
            .launch_expedition(&id("core:origin"), &ids.ship_id, &id("core:target"), None)
            .unwrap();
        let TransitKind::Expedition { reservations, .. } = &mut world.transit[0].kind else {
            panic!("expected expedition");
        };
        *reservations = Some(ExpeditionReservations {
            habitat: SlotCoordinate {
                body: id("core:target_body"),
                slot: id("core:target_slot_0"),
            },
            collector: SlotCoordinate {
                body: id("core:target_body"),
                slot: id("core:target_slot_1"),
            },
        });

        let before = world.debug_snapshot();
        assert!(matches!(
            world.advance_tick(),
            Err(CoreError::InvalidExpeditionReservation(_))
        ));
        assert_eq!(world.debug_snapshot(), before);
        assert_eq!(world.time(), SimulationTime { tick: 3 });
    }

    #[test]
    fn forced_due_message_failure_rolls_back_movement_clock_and_observer_counter() {
        let mut world = fixture();
        let ids = world
            .enqueue_ship_project(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:yard_slot"),
                ShipProjectKind::Probe,
            )
            .unwrap();
        world.advance_tick().unwrap();
        world
            .launch_probe(&id("core:origin"), &ids.ship_id, &id("core:target"), 10)
            .unwrap();
        let transmission = PendingTransmission {
            id: TransmissionId {
                observer: ObserverId::Ship(ShipId::new(id("core:origin"), 99)),
                sequence: 7,
            },
            tick_observed: 1,
            tick_received: 1,
            facts: vec![ObservedFact {
                system: id("core:target"),
                key: FactKey::SystemStrength,
                value: FactValue::Unsigned(999),
                detail: FactDetail::Complete,
            }],
        };
        world
            .knowledge
            .pending_transmissions
            .insert(transmission.id.clone(), transmission);

        let before = world.debug_snapshot();
        assert_eq!(
            before.transit[0]
                .observer_counters
                .next_transmission_sequence,
            0
        );
        assert!(matches!(
            world.advance_tick(),
            Err(CoreError::KnowledgeIntegration(_))
        ));
        assert_eq!(world.debug_snapshot(), before);
        assert_eq!(world.time(), SimulationTime { tick: 3 });
    }

    #[test]
    fn runtime_requires_a_bijection_between_transit_tokens_and_expeditions() {
        let mut world = fixture();
        let ids = world
            .enqueue_ship_project(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:yard_slot"),
                ShipProjectKind::Expedition,
            )
            .unwrap();
        world.advance_tick().unwrap();
        world
            .launch_expedition(&id("core:origin"), &ids.ship_id, &id("core:target"), None)
            .unwrap();
        let population_id = PopulationId::new(id("core:origin"), 0);
        world
            .populations
            .tokens
            .get_mut(&population_id)
            .unwrap()
            .state = PopulationState::InTransit {
            ship_id: ShipId::new(id("core:origin"), 999),
        };

        let before = world.debug_snapshot();
        assert!(matches!(
            world.advance_tick(),
            Err(CoreError::InvalidTransitPopulationBijection(_))
        ));
        assert_eq!(world.debug_snapshot(), before);
    }

    #[test]
    fn forced_retention_failure_rolls_back_population_id_allocation_and_clock() {
        let mut world = fixture();
        world.populations.tokens.clear();
        world.population_accounting.removed = world.population_accounting.generated;
        world.time.tick = 1;
        let origin = world.systems.get_mut(&id("core:origin")).unwrap();
        origin.overflow.cumulative = u64::MAX;
        origin.stocks.set(id("core:energy"), 1_000);
        origin.bodies[0].slots[1]
            .development
            .as_mut()
            .unwrap()
            .habitat
            .as_mut()
            .unwrap()
            .ready_since_tick = Some(0);
        origin.bodies[0].slots[1]
            .development
            .as_mut()
            .unwrap()
            .habitat
            .as_mut()
            .unwrap()
            .generation_progress = 500;

        let before = world.debug_snapshot();
        assert_eq!(before.systems[0].counters.next_population_sequence, 1);
        assert_eq!(world.advance_tick(), Err(CoreError::Overflow));
        assert_eq!(world.debug_snapshot(), before);
        assert_eq!(world.time(), SimulationTime { tick: 1 });
    }
}
