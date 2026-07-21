use crate::{
    BodyState, ContentId, CoreError, LifeSupportEvidence, PopulationId, ResourceAccounting,
    ResourceStore, ShipId, SimulationTime, WorldTuning, add, sub,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunityDefinition {
    pub id: ContentId,
    pub system: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PopulationState {
    Resident {
        community_id: ContentId,
        habitat_id: ContentId,
    },
    InTransit {
        ship_id: ShipId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopulationToken {
    pub id: PopulationId,
    pub state: PopulationState,
}

/// Sole mutable population authority.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PopulationRegistry {
    pub tokens: BTreeMap<PopulationId, PopulationToken>,
}

impl PopulationRegistry {
    #[must_use]
    pub fn community_population(&self, community: &ContentId) -> u64 {
        u64::try_from(self.tokens.values().filter(|token| matches!(&token.state, PopulationState::Resident { community_id, .. } if community_id == community)).count()).unwrap_or(u64::MAX)
    }

    #[must_use]
    pub fn system_population(
        &self,
        communities: &BTreeMap<ContentId, CommunityDefinition>,
        system: &ContentId,
    ) -> u64 {
        communities
            .values()
            .find(|community| &community.system == system)
            .map_or(0, |community| self.community_population(&community.id))
    }

    #[must_use]
    pub fn habitat_occupant(&self, habitat: &ContentId) -> Option<&PopulationToken> {
        self.tokens.values().find(|token| matches!(&token.state, PopulationState::Resident { habitat_id, .. } if habitat_id == habitat))
    }

    /// Removes a token whose Habitat ceased to provide support and records the loss exactly once.
    pub fn remove_habitat_occupant(
        &mut self,
        habitat: &ContentId,
        time: SimulationTime,
        accounting: &mut PopulationAccounting,
    ) -> Result<Option<PopulationToken>, CoreError> {
        let occupant = self.habitat_occupant(habitat).map(|token| token.id.clone());
        let Some(population_id) = occupant else {
            return Ok(None);
        };
        accounting
            .removed
            .checked_add(1)
            .ok_or(CoreError::Overflow)?;
        let token = self
            .tokens
            .remove(&population_id)
            .expect("occupant was present");
        accounting.record(
            time,
            population_id,
            PopulationTransition::HabitatSupportRemoval {
                habitat_id: habitat.clone(),
            },
        )?;
        Ok(Some(token))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PopulationTransition {
    Generated {
        community_id: ContentId,
        habitat_id: ContentId,
    },
    EnteredTransit {
        ship_id: ShipId,
        source_community_id: ContentId,
        source_habitat_id: ContentId,
    },
    ResidenceTransferred {
        ship_id: ShipId,
        target_community_id: ContentId,
        target_habitat_id: ContentId,
    },
    ExpeditionLost {
        ship_id: ShipId,
    },
    HabitatSupportRemoval {
        habitat_id: ContentId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopulationAccountingEntry {
    pub tick: u64,
    pub population_id: PopulationId,
    pub transition: PopulationTransition,
}

/// Persistent world-owned evidence for token creation, movement, residence, and removal.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PopulationAccounting {
    pub generated: u64,
    pub removed: u64,
    pub entries: Vec<PopulationAccountingEntry>,
}

impl PopulationAccounting {
    pub fn record(
        &mut self,
        time: SimulationTime,
        population_id: PopulationId,
        transition: PopulationTransition,
    ) -> Result<(), CoreError> {
        match transition {
            PopulationTransition::Generated { .. } => {
                self.generated = self.generated.checked_add(1).ok_or(CoreError::Overflow)?;
            }
            PopulationTransition::ExpeditionLost { .. }
            | PopulationTransition::HabitatSupportRemoval { .. } => {
                self.removed = self.removed.checked_add(1).ok_or(CoreError::Overflow)?;
            }
            PopulationTransition::EnteredTransit { .. }
            | PopulationTransition::ResidenceTransferred { .. } => {}
        }
        self.entries.push(PopulationAccountingEntry {
            tick: time.tick,
            population_id,
            transition,
        });
        Ok(())
    }
}

/// Persistent per-Habitat population-generation state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HabitatState {
    pub generation_enabled: bool,
    pub generation_progress: u64,
    pub ready_since_tick: Option<u64>,
}

impl Default for HabitatState {
    fn default() -> Self {
        Self {
            generation_enabled: true,
            generation_progress: 0,
            ready_since_tick: None,
        }
    }
}

pub(crate) struct HabitatPhaseContext<'a> {
    pub time: SimulationTime,
    pub system_id: &'a ContentId,
    pub community_id: Option<&'a ContentId>,
    pub bodies: &'a mut [BodyState],
    pub stocks: &'a mut ResourceStore,
    pub populations: &'a mut PopulationRegistry,
    pub population_accounting: &'a mut PopulationAccounting,
    pub next_population_sequence: &'a mut u64,
    pub tuning: &'a WorldTuning,
}

/// Phase 1: create tokens for enabled, empty, functional Habitats that became ready
/// before the current tick.
pub(crate) fn finalize_population_generation(
    context: HabitatPhaseContext<'_>,
) -> Result<(), CoreError> {
    let Some(community_id) = context.community_id else {
        return Ok(());
    };
    let mut candidates = Vec::new();
    for (body_index, body) in context.bodies.iter().enumerate() {
        for (slot_index, slot) in body.slots.iter().enumerate() {
            let Some(development) = &slot.development else {
                continue;
            };
            let Some(habitat) = &development.habitat else {
                continue;
            };
            if development.definition.role != crate::DevelopmentRole::Habitat
                || development.definition.condition != crate::DevelopmentCondition::Functional
                || !habitat.generation_enabled
                || !habitat
                    .ready_since_tick
                    .is_some_and(|ready_tick| ready_tick < context.time.tick)
                || context
                    .populations
                    .habitat_occupant(&development.definition.id)
                    .is_some()
            {
                continue;
            }
            candidates.push((body_index, slot_index, development.definition.id.clone()));
        }
    }

    let count = u64::try_from(candidates.len()).map_err(|_| CoreError::Overflow)?;
    let final_sequence = context
        .next_population_sequence
        .checked_add(count)
        .ok_or(CoreError::Overflow)?;
    context
        .population_accounting
        .generated
        .checked_add(count)
        .ok_or(CoreError::Overflow)?;
    for (offset, (_, _, _)) in candidates.iter().enumerate() {
        let offset = u64::try_from(offset).map_err(|_| CoreError::Overflow)?;
        let population_id = PopulationId::new(
            context.system_id.clone(),
            context
                .next_population_sequence
                .checked_add(offset)
                .ok_or(CoreError::Overflow)?,
        );
        if context.populations.tokens.contains_key(&population_id) {
            return Err(CoreError::DuplicatePopulationId);
        }
    }

    for (offset, (body_index, slot_index, habitat_id)) in candidates.into_iter().enumerate() {
        let offset = u64::try_from(offset).map_err(|_| CoreError::Overflow)?;
        let population_id = PopulationId::new(
            context.system_id.clone(),
            context
                .next_population_sequence
                .checked_add(offset)
                .ok_or(CoreError::Overflow)?,
        );
        let token = PopulationToken {
            id: population_id.clone(),
            state: PopulationState::Resident {
                community_id: community_id.clone(),
                habitat_id: habitat_id.clone(),
            },
        };
        context
            .populations
            .tokens
            .insert(population_id.clone(), token);
        let habitat = context.bodies[body_index].slots[slot_index]
            .development
            .as_mut()
            .and_then(|development| development.habitat.as_mut())
            .expect("candidate remains a Habitat");
        habitat.generation_progress = 0;
        habitat.ready_since_tick = None;
        context.population_accounting.record(
            context.time,
            population_id,
            PopulationTransition::Generated {
                community_id: community_id.clone(),
                habitat_id,
            },
        )?;
    }
    *context.next_population_sequence = final_sequence;
    Ok(())
}

/// Phase 7: spend available Energy on enabled, empty, functional Habitats in
/// stable body/slot order. Reaching the cost records readiness for a later tick.
pub(crate) fn accumulate_population_generation(
    context: HabitatPhaseContext<'_>,
) -> Result<(), CoreError> {
    let energy = &context.tuning.energy_resource;
    let cost = context.tuning.habitat_population_energy;
    let mut available = context.stocks.quantity(energy);
    let mut updates = Vec::new();
    let mut total_spent = 0_u64;

    for (body_index, body) in context.bodies.iter().enumerate() {
        for (slot_index, slot) in body.slots.iter().enumerate() {
            let Some(development) = &slot.development else {
                continue;
            };
            let Some(habitat) = &development.habitat else {
                continue;
            };
            if development.definition.role != crate::DevelopmentRole::Habitat
                || development.definition.condition != crate::DevelopmentCondition::Functional
                || !habitat.generation_enabled
                || habitat.ready_since_tick.is_some()
                || context
                    .populations
                    .habitat_occupant(&development.definition.id)
                    .is_some()
            {
                continue;
            }
            let remaining = cost
                .checked_sub(habitat.generation_progress)
                .ok_or(CoreError::Overflow)?;
            let spent = available.min(remaining);
            available -= spent;
            total_spent = total_spent.checked_add(spent).ok_or(CoreError::Overflow)?;
            let progress = habitat
                .generation_progress
                .checked_add(spent)
                .ok_or(CoreError::Overflow)?;
            updates.push((body_index, slot_index, progress, progress == cost));
        }
    }

    if total_spent != 0 {
        sub(context.stocks, energy, total_spent)?;
    }
    for (body_index, slot_index, progress, ready) in updates {
        let habitat = context.bodies[body_index].slots[slot_index]
            .development
            .as_mut()
            .and_then(|development| development.habitat.as_mut())
            .expect("eligible coordinate remains a Habitat");
        habitat.generation_progress = progress;
        if ready {
            habitat.ready_since_tick = Some(context.time.tick);
        }
    }
    Ok(())
}

/// Changes automatic generation only for an empty, functional Habitat. Progress and
/// readiness are preserved, and changing the setting never refunds Energy.
pub(crate) fn set_habitat_generation_enabled(
    bodies: &mut [BodyState],
    populations: &PopulationRegistry,
    body_id: &ContentId,
    slot_id: &ContentId,
    enabled: bool,
) -> Result<(), CoreError> {
    let habitat_id = crate::find_slot(bodies, body_id, slot_id)?
        .development
        .as_ref()
        .filter(|development| {
            development.definition.role == crate::DevelopmentRole::Habitat
                && development.definition.condition == crate::DevelopmentCondition::Functional
                && development.habitat.is_some()
        })
        .map(|development| development.definition.id.clone())
        .ok_or_else(|| CoreError::DevelopmentSlotUnavailable {
            body: body_id.clone(),
            slot: slot_id.clone(),
        })?;
    if populations.habitat_occupant(&habitat_id).is_some() {
        return Err(CoreError::DevelopmentSlotUnavailable {
            body: body_id.clone(),
            slot: slot_id.clone(),
        });
    }
    crate::find_slot_mut(bodies, body_id, slot_id)?
        .development
        .as_mut()
        .and_then(|development| development.habitat.as_mut())
        .expect("validated functional Habitat remains present")
        .generation_enabled = enabled;
    Ok(())
}

/// Leaf hook for any validated transition that damages, ruins, or removes a Habitat.
#[allow(dead_code)]
pub(crate) fn invalidate_habitat_support(
    time: SimulationTime,
    habitat_id: &ContentId,
    populations: &mut PopulationRegistry,
    accounting: &mut PopulationAccounting,
) -> Result<Option<PopulationToken>, CoreError> {
    populations.remove_habitat_occupant(habitat_id, time, accounting)
}

pub(crate) struct LifeSupportPhaseContext<'a> {
    pub is_origin: bool,
    pub community_id: Option<&'a ContentId>,
    pub stocks: &'a mut ResourceStore,
    pub populations: &'a PopulationRegistry,
    pub resource_accounting: &'a mut ResourceAccounting,
    pub tuning: &'a WorldTuning,
}

/// Derives support and work only from resident population after paying available Energy.
pub(crate) fn derive_life_support(
    context: LifeSupportPhaseContext<'_>,
) -> Result<LifeSupportEvidence, CoreError> {
    let population = context.community_id.map_or(0, |community| {
        context.populations.community_population(community)
    });
    let energy = &context.tuning.energy_resource;
    let required = context
        .tuning
        .life_support_per_population
        .checked_mul(population)
        .ok_or(CoreError::Overflow)?;
    let paid = context.stocks.quantity(energy).min(required);
    context
        .resource_accounting
        .operation_spent
        .quantity(energy)
        .checked_add(paid)
        .ok_or(CoreError::Overflow)?;
    if paid != 0 {
        sub(context.stocks, energy, paid)?;
        add(
            &mut context.resource_accounting.operation_spent,
            energy,
            paid,
        )?;
    }
    let supported = paid / context.tuning.life_support_per_population;
    let origin_work = if context.is_origin {
        context.tuning.origin_construction_work
    } else {
        0
    };
    Ok(LifeSupportEvidence {
        required_energy: required,
        paid_energy: paid,
        unpaid_energy: required - paid,
        supported_population: supported,
        underserved_population: population - supported,
        construction_work: supported
            .checked_add(origin_work)
            .ok_or(CoreError::Overflow)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BodyDefinition, ConstructionRecipe, DevelopmentCondition, DevelopmentDefinition,
        DevelopmentRole, DevelopmentSlotDefinition, DevelopmentSlotState, DevelopmentState,
        ExpeditionProjectTuning, ExtractorParameters, FixedRate, Position3, ProbeProjectTuning,
        ProductionCycle, RefineryParameters, ResourceDefinition, ShipTravelTuning,
        SystemDefinition, WorldDefinition, WorldState,
    };
    use std::num::NonZeroU64;

    fn id(value: &str) -> ContentId {
        ContentId::new(value).expect("valid id")
    }

    fn tuning(generation_cost: u64) -> WorldTuning {
        let energy = id("core:energy");
        let recipe = ConstructionRecipe {
            cost: [(energy.clone(), 1)].into_iter().collect(),
            required_work: 1,
        };
        WorldTuning {
            energy_resource: energy,
            ore_resource: id("core:ore"),
            alloy_resource: id("core:alloy"),
            seasonal_shape: [1; 10],
            seasonal_baseline_average: 1,
            life_support_per_population: 2,
            origin_construction_work: 3,
            intrinsic_energy_capacity: 100,
            battery_energy_capacity: 100,
            habitat_population_energy: generation_cost,
            coordinate_quanta_per_map_unit: 1,
            collector_recipe: recipe.clone(),
            battery_recipe: recipe.clone(),
            extractor_recipe: recipe.clone(),
            refinery_recipe: recipe.clone(),
            habitat_recipe: recipe.clone(),
            shipyard_recipe: recipe,
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
                material_commitment: ResourceStore::new(),
                duration_ticks: 1,
                energy_per_progress_tick: 1,
            },
            expedition_project: ExpeditionProjectTuning {
                hull_material_commitment: ResourceStore::new(),
                founding_stocks: ResourceStore::new(),
                duration_ticks: 1,
                energy_per_progress_tick: 1,
            },
            probe_travel: ShipTravelTuning {
                maximum_jump_quanta: 1,
                speed_quanta_per_tick: 1,
                energy_per_quantum: FixedRate::new(1, NonZeroU64::new(1).unwrap()),
            },
            expedition_travel: ShipTravelTuning {
                maximum_jump_quanta: 1,
                speed_quanta_per_tick: 1,
                energy_per_quantum: FixedRate::new(1, NonZeroU64::new(1).unwrap()),
            },
            probe_reveal_radius_quanta: 1,
            communication_delay_per_quantum: FixedRate::new(1, NonZeroU64::new(1).unwrap()),
            resource_richness: BTreeMap::new(),
        }
    }

    fn habitat_slot(slot: &str, habitat: &str) -> DevelopmentSlotState {
        DevelopmentSlotState {
            id: id(slot),
            development: Some(DevelopmentState {
                definition: DevelopmentDefinition {
                    id: id(habitat),
                    role: DevelopmentRole::Habitat,
                    condition: DevelopmentCondition::Functional,
                    extractor_target: None,
                },
                cycle: ProductionCycle::default(),
                habitat: Some(HabitatState::default()),
                shipyard: None,
            }),
            reserved_by: None,
        }
    }

    fn body_with_habitats() -> Vec<BodyState> {
        vec![BodyState {
            id: id("core:body"),
            remaining_resources: ResourceStore::new(),
            slots: vec![
                habitat_slot("core:z_first_slot", "core:z_first_habitat"),
                habitat_slot("core:a_second_slot", "core:a_second_habitat"),
            ],
        }]
    }

    #[allow(clippy::too_many_arguments)]
    fn run_habitat_phase(
        finalize: bool,
        tick: u64,
        system: &ContentId,
        community: &ContentId,
        bodies: &mut [BodyState],
        stocks: &mut ResourceStore,
        resource_accounting: &mut ResourceAccounting,
        populations: &mut PopulationRegistry,
        population_accounting: &mut PopulationAccounting,
        next_sequence: &mut u64,
        tuning: &WorldTuning,
    ) -> Result<(), CoreError> {
        let energy_before = stocks.quantity(&tuning.energy_resource);
        let context = HabitatPhaseContext {
            time: SimulationTime { tick },
            system_id: system,
            community_id: Some(community),
            bodies,
            stocks,
            populations,
            population_accounting,
            next_population_sequence: next_sequence,
            tuning,
        };
        if finalize {
            finalize_population_generation(context)
        } else {
            accumulate_population_generation(context)?;
            let spent = energy_before
                .checked_sub(stocks.quantity(&tuning.energy_resource))
                .ok_or(CoreError::Overflow)?;
            if spent != 0 {
                add(
                    &mut resource_accounting.operation_spent,
                    &tuning.energy_resource,
                    spent,
                )?;
            }
            Ok(())
        }
    }

    #[test]
    fn habitats_accumulate_in_slot_order_and_toggle_without_losing_progress() {
        let system = id("core:origin");
        let community = id("core:community");
        let tuning = tuning(5);
        let mut bodies = body_with_habitats();
        let mut stocks: ResourceStore = [(id("core:energy"), 6)].into_iter().collect();
        let mut resource_accounting = ResourceAccounting::default();
        let mut populations = PopulationRegistry::default();
        let mut population_accounting = PopulationAccounting::default();
        let mut next_sequence = 0;

        run_habitat_phase(
            false,
            10,
            &system,
            &community,
            &mut bodies,
            &mut stocks,
            &mut resource_accounting,
            &mut populations,
            &mut population_accounting,
            &mut next_sequence,
            &tuning,
        )
        .unwrap();
        let first = bodies[0].slots[0]
            .development
            .as_ref()
            .unwrap()
            .habitat
            .as_ref()
            .unwrap();
        let second = bodies[0].slots[1]
            .development
            .as_ref()
            .unwrap()
            .habitat
            .as_ref()
            .unwrap();
        assert_eq!(
            (first.generation_progress, first.ready_since_tick),
            (5, Some(10))
        );
        assert_eq!(
            (second.generation_progress, second.ready_since_tick),
            (1, None)
        );
        assert_eq!(stocks.quantity(&id("core:energy")), 0);
        assert_eq!(
            resource_accounting
                .operation_spent
                .quantity(&id("core:energy")),
            6
        );

        set_habitat_generation_enabled(
            &mut bodies,
            &populations,
            &id("core:body"),
            &id("core:a_second_slot"),
            false,
        )
        .unwrap();
        stocks.set(id("core:energy"), 4);
        run_habitat_phase(
            false,
            11,
            &system,
            &community,
            &mut bodies,
            &mut stocks,
            &mut resource_accounting,
            &mut populations,
            &mut population_accounting,
            &mut next_sequence,
            &tuning,
        )
        .unwrap();
        assert_eq!(stocks.quantity(&id("core:energy")), 4);
        assert_eq!(
            bodies[0].slots[1]
                .development
                .as_ref()
                .unwrap()
                .habitat
                .as_ref()
                .unwrap()
                .generation_progress,
            1
        );

        set_habitat_generation_enabled(
            &mut bodies,
            &populations,
            &id("core:body"),
            &id("core:a_second_slot"),
            true,
        )
        .unwrap();
        run_habitat_phase(
            false,
            12,
            &system,
            &community,
            &mut bodies,
            &mut stocks,
            &mut resource_accounting,
            &mut populations,
            &mut population_accounting,
            &mut next_sequence,
            &tuning,
        )
        .unwrap();
        let second = bodies[0].slots[1]
            .development
            .as_ref()
            .unwrap()
            .habitat
            .as_ref()
            .unwrap();
        assert_eq!(
            (second.generation_progress, second.ready_since_tick),
            (5, Some(12))
        );
        assert_eq!(stocks.quantity(&id("core:energy")), 0);
    }

    #[test]
    fn ready_habitat_creates_on_a_following_tick_and_never_reuses_ids() {
        let system = id("core:origin");
        let community = id("core:community");
        let tuning = tuning(5);
        let mut bodies = body_with_habitats();
        let mut stocks: ResourceStore = [(id("core:energy"), 5)].into_iter().collect();
        let mut resource_accounting = ResourceAccounting::default();
        let mut populations = PopulationRegistry::default();
        let mut population_accounting = PopulationAccounting::default();
        let mut next_sequence = 7;

        run_habitat_phase(
            false,
            3,
            &system,
            &community,
            &mut bodies,
            &mut stocks,
            &mut resource_accounting,
            &mut populations,
            &mut population_accounting,
            &mut next_sequence,
            &tuning,
        )
        .unwrap();
        run_habitat_phase(
            true,
            3,
            &system,
            &community,
            &mut bodies,
            &mut stocks,
            &mut resource_accounting,
            &mut populations,
            &mut population_accounting,
            &mut next_sequence,
            &tuning,
        )
        .unwrap();
        assert!(populations.tokens.is_empty());

        run_habitat_phase(
            true,
            4,
            &system,
            &community,
            &mut bodies,
            &mut stocks,
            &mut resource_accounting,
            &mut populations,
            &mut population_accounting,
            &mut next_sequence,
            &tuning,
        )
        .unwrap();
        let first_id = PopulationId::new(system.clone(), 7);
        assert!(populations.tokens.contains_key(&first_id));
        assert_eq!(next_sequence, 8);
        assert_eq!(population_accounting.generated, 1);
        assert!(matches!(
            set_habitat_generation_enabled(
                &mut bodies,
                &populations,
                &id("core:body"),
                &id("core:z_first_slot"),
                false,
            ),
            Err(CoreError::DevelopmentSlotUnavailable { .. })
        ));
        let habitat = id("core:z_first_habitat");
        invalidate_habitat_support(
            SimulationTime { tick: 4 },
            &habitat,
            &mut populations,
            &mut population_accounting,
        )
        .unwrap();

        stocks.set(id("core:energy"), 5);
        run_habitat_phase(
            false,
            4,
            &system,
            &community,
            &mut bodies,
            &mut stocks,
            &mut resource_accounting,
            &mut populations,
            &mut population_accounting,
            &mut next_sequence,
            &tuning,
        )
        .unwrap();
        run_habitat_phase(
            true,
            5,
            &system,
            &community,
            &mut bodies,
            &mut stocks,
            &mut resource_accounting,
            &mut populations,
            &mut population_accounting,
            &mut next_sequence,
            &tuning,
        )
        .unwrap();
        assert!(!populations.tokens.contains_key(&first_id));
        assert!(
            populations
                .tokens
                .contains_key(&PopulationId::new(system, 8))
        );
        assert_eq!(next_sequence, 9);
        assert_eq!(
            (
                population_accounting.generated,
                population_accounting.removed
            ),
            (2, 1)
        );
    }

    #[test]
    fn life_support_and_remote_work_are_derived_from_resident_tokens() {
        let community = id("core:community");
        let registry = PopulationRegistry {
            tokens: (0..2)
                .map(|sequence| {
                    let population_id = PopulationId::new(id("core:origin"), sequence);
                    (
                        population_id.clone(),
                        PopulationToken {
                            id: population_id,
                            state: PopulationState::Resident {
                                community_id: community.clone(),
                                habitat_id: id(&format!("core:habitat_{sequence}")),
                            },
                        },
                    )
                })
                .collect(),
        };
        let tuning = tuning(5);
        let mut stocks: ResourceStore = [(id("core:energy"), 3)].into_iter().collect();
        let mut accounting = ResourceAccounting::default();
        let evidence = derive_life_support(LifeSupportPhaseContext {
            is_origin: false,
            community_id: Some(&community),
            stocks: &mut stocks,
            populations: &registry,
            resource_accounting: &mut accounting,
            tuning: &tuning,
        })
        .unwrap();
        assert_eq!(evidence.required_energy, 4);
        assert_eq!(evidence.paid_energy, 3);
        assert_eq!(evidence.supported_population, 1);
        assert_eq!(evidence.underserved_population, 1);
        assert_eq!(evidence.construction_work, 1);
        assert_eq!(stocks.quantity(&id("core:energy")), 0);
        assert_eq!(accounting.operation_spent.quantity(&id("core:energy")), 3);
    }

    #[test]
    fn zero_population_commandability_is_enforced_for_habitat_commands() {
        let systems = [
            ("core:origin", true, true),
            ("core:remote", true, true),
            ("core:awaiting", true, false),
            ("core:neutral", false, false),
        ];
        let definition = WorldDefinition {
            resources: ["core:energy", "core:ore", "core:alloy"]
                .into_iter()
                .map(|value| ResourceDefinition {
                    id: id(value),
                    name: value.into(),
                    naturally_deposit_bearing: false,
                })
                .collect(),
            locations: systems
                .iter()
                .enumerate()
                .map(|(index, (system, _, _))| crate::LocationDefinition {
                    id: id(system),
                    name: (*system).into(),
                    position: Position3::from_quanta(i64::try_from(index).unwrap(), 0, 0),
                })
                .collect(),
            origin_system: id("core:origin"),
            origin_community: id("core:community"),
            communities: vec![
                CommunityDefinition {
                    id: id("core:community"),
                    system: id("core:origin"),
                },
                CommunityDefinition {
                    id: id("core:remote_community"),
                    system: id("core:remote"),
                },
                CommunityDefinition {
                    id: id("core:awaiting_community"),
                    system: id("core:awaiting"),
                },
            ],
            population_tokens: Vec::new(),
            systems: systems
                .iter()
                .map(|(system, founded, unlocked)| SystemDefinition {
                    location: id(system),
                    stellar_strength_hundredths: 100,
                    bodies: vec![BodyDefinition {
                        id: id(&format!("core:{}_body", system.split(':').nth(1).unwrap())),
                        name: "Body".into(),
                        eccentricity_hundredths: 0,
                        initial_resources: ResourceStore::new(),
                        slots: vec![DevelopmentSlotDefinition {
                            id: id("core:slot"),
                            development: (*system == "core:origin").then(|| {
                                DevelopmentDefinition {
                                    id: id("core:origin_habitat"),
                                    role: DevelopmentRole::Habitat,
                                    condition: DevelopmentCondition::Functional,
                                    extractor_target: None,
                                }
                            }),
                        }],
                    }],
                    stocks: ResourceStore::new(),
                    player_founded: *founded,
                    command_unlock_received: *unlocked,
                })
                .collect(),
            sites: Vec::new(),
            tuning: tuning(5),
        };
        let mut with_initial_population = definition.clone();
        with_initial_population
            .population_tokens
            .push(PopulationToken {
                id: PopulationId::new(id("core:origin"), 0),
                state: PopulationState::InTransit {
                    ship_id: ShipId::new(id("core:origin"), 0),
                },
            });
        assert_eq!(
            WorldState::new(with_initial_population),
            Err(CoreError::InitialPopulationTokensNotAllowed)
        );

        let mut missing_founded_community = definition.clone();
        missing_founded_community
            .communities
            .retain(|community| community.system != id("core:remote"));
        assert!(matches!(
            WorldState::new(missing_founded_community),
            Err(CoreError::FoundedSystemMissingCommunity(system))
                if system == id("core:remote")
        ));

        let mut neutral_with_community = definition.clone();
        neutral_with_community
            .communities
            .push(CommunityDefinition {
                id: id("core:neutral_community"),
                system: id("core:neutral"),
            });
        assert!(matches!(
            WorldState::new(neutral_with_community),
            Err(CoreError::NeutralSystemHasCommunity(system))
                if system == id("core:neutral")
        ));

        let mut world = WorldState::new(definition).unwrap();

        assert_eq!(
            world.commandability(&id("core:origin")),
            Ok(crate::Commandability::Origin)
        );
        assert_eq!(
            world.commandability(&id("core:remote")),
            Ok(crate::Commandability::Depopulated)
        );
        assert_eq!(
            world.commandability(&id("core:awaiting")),
            Ok(crate::Commandability::AwaitingFoundingOutcome)
        );
        assert_eq!(
            world.commandability(&id("core:neutral")),
            Ok(crate::Commandability::Neutral)
        );
        assert!(matches!(
            world.set_habitat_generation_enabled(
                &id("core:remote"),
                &id("core:remote_body"),
                &id("core:slot"),
                false,
            ),
            Err(CoreError::SystemNotCommandable(system)) if system == id("core:remote")
        ));
        world
            .set_habitat_generation_enabled(
                &id("core:origin"),
                &id("core:origin_body"),
                &id("core:slot"),
                false,
            )
            .unwrap();
        world.advance_tick().unwrap();
        let snapshot = world.debug_system_snapshot(&id("core:origin")).unwrap();
        assert!(
            !snapshot.bodies[0].slots[0]
                .development
                .as_ref()
                .unwrap()
                .habitat
                .as_ref()
                .unwrap()
                .generation_enabled
        );
    }

    #[test]
    fn population_registry_is_the_derived_population_and_occupancy_authority() {
        let system = id("core:origin");
        let community = id("core:community");
        let habitat = id("core:habitat");
        let population_id = PopulationId::new(system, 7);
        let token = PopulationToken {
            id: population_id.clone(),
            state: PopulationState::Resident {
                community_id: community.clone(),
                habitat_id: habitat.clone(),
            },
        };
        let transit_id = PopulationId::new(id("core:origin"), 8);
        let registry = PopulationRegistry {
            tokens: [
                (population_id, token),
                (
                    transit_id.clone(),
                    PopulationToken {
                        id: transit_id,
                        state: PopulationState::InTransit {
                            ship_id: ShipId::new(id("core:origin"), 2),
                        },
                    },
                ),
            ]
            .into_iter()
            .collect(),
        };
        assert_eq!(registry.community_population(&community), 1);
        assert!(registry.habitat_occupant(&habitat).is_some());
        assert_eq!(registry.community_population(&id("core:other")), 0);
    }

    #[test]
    fn habitat_support_loss_removes_and_accounts_for_the_token_once() {
        let habitat = id("core:habitat");
        let population_id = PopulationId::new(id("core:origin"), 0);
        let mut registry = PopulationRegistry {
            tokens: [(
                population_id.clone(),
                PopulationToken {
                    id: population_id.clone(),
                    state: PopulationState::Resident {
                        community_id: id("core:community"),
                        habitat_id: habitat.clone(),
                    },
                },
            )]
            .into_iter()
            .collect(),
        };
        let mut accounting = PopulationAccounting::default();

        assert!(
            invalidate_habitat_support(
                SimulationTime { tick: 7 },
                &habitat,
                &mut registry,
                &mut accounting,
            )
            .expect("valid removal")
            .is_some()
        );
        assert!(registry.tokens.is_empty());
        assert_eq!(accounting.removed, 1);
        assert_eq!(accounting.entries[0].population_id, population_id);
        assert_eq!(
            invalidate_habitat_support(
                SimulationTime { tick: 8 },
                &habitat,
                &mut registry,
                &mut accounting,
            ),
            Ok(None)
        );
        assert_eq!(accounting.removed, 1);
    }
}
