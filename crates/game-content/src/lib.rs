//! Strict RON adapter for Stage 4b world definitions.

mod diagnostics;
mod fingerprint;
mod generator;
mod profile;
mod schema;

pub use diagnostics::{ContentDiagnostic, ContentErrors};
pub use fingerprint::{CanonicalEncodingError, canonical_profile_bytes, sha256_fingerprint};
pub use generator::{
    CompiledProfile, GeneratedWorldArtifact, GenerationError, GenerationIdentity,
    GenerationRequest, GeneratorVersion, SourceProvenance, generate_world,
};
pub use profile::{
    FrontierResourceGenerationTuning, GeneratorTuning, NormalizedProfile,
    OriginResourceGenerationTuning, ProfileValidationError, ResourceGenerationTuning, SignedBounds,
    SystemGenerationTuning, UnsignedRatio, UnsignedTriangle,
};

use diagnostics::push;
use game_core::*;
use schema::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::num::NonZeroU64;
use std::path::Path;

pub fn compile_str(
    source_name: impl AsRef<str>,
    source: &str,
) -> Result<WorldDefinition, ContentErrors> {
    let source_name = source_name.as_ref().to_owned();
    let parsed = ron::from_str::<WorldSource>(source).map_err(|error| {
        ContentErrors::one(source_name.clone(), "document", "parse", error.to_string())
    })?;
    compile_world(&source_name, parsed)
}

pub fn load_file(path: impl AsRef<Path>) -> Result<WorldDefinition, ContentErrors> {
    let path = path.as_ref();
    let source_name = path.display().to_string();
    let source = fs::read_to_string(path).map_err(|error| {
        ContentErrors::one(source_name.clone(), "document", "read", error.to_string())
    })?;
    compile_str(source_name, &source)
}

pub fn compile_profile_str(
    source_name: impl AsRef<str>,
    source: &str,
) -> Result<NormalizedProfile, ContentErrors> {
    let source_name = source_name.as_ref().to_owned();
    let parsed = ron::from_str::<ProfileSource>(source).map_err(|error| {
        ContentErrors::one(source_name.clone(), "document", "parse", error.to_string())
    })?;
    compile_profile(&source_name, parsed)
}

pub fn load_profile_file(path: impl AsRef<Path>) -> Result<NormalizedProfile, ContentErrors> {
    let path = path.as_ref();
    let source_name = path.display().to_string();
    let source = fs::read_to_string(path).map_err(|error| {
        ContentErrors::one(source_name.clone(), "document", "read", error.to_string())
    })?;
    compile_profile_str(source_name, &source)
}

/// Strictly parses, validates, canonically encodes, fingerprints, and records logical provenance.
pub fn compile_generation_profile_str(
    source_identity: impl AsRef<str>,
    source: &str,
) -> Result<CompiledProfile, ContentErrors> {
    let source_identity = source_identity.as_ref();
    let normalized = compile_profile_str(source_identity, source)?;
    let provenance = SourceProvenance::from_source(source_identity, source.as_bytes());
    CompiledProfile::new(normalized, provenance).map_err(|error| {
        ContentErrors::one(
            source_identity.to_owned(),
            "profile",
            "canonical_encoding",
            error.to_string(),
        )
    })
}

/// Loads profile bytes from a machine-local path while retaining only logical source provenance.
pub fn load_generation_profile_file(
    source_identity: impl AsRef<str>,
    path: impl AsRef<Path>,
) -> Result<CompiledProfile, ContentErrors> {
    let source_identity = source_identity.as_ref();
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|error| {
        ContentErrors::one(
            source_identity.to_owned(),
            "document",
            "read",
            error.to_string(),
        )
    })?;
    compile_generation_profile_str(source_identity, &source)
}

fn parse_id(
    source: &str,
    definition: &str,
    field: &str,
    value: &str,
    diagnostics: &mut Vec<ContentDiagnostic>,
) -> Option<ContentId> {
    match ContentId::new(value) {
        Ok(id) => Some(id),
        Err(error) => {
            push(diagnostics, source, definition, field, error.to_string());
            None
        }
    }
}

fn amounts(
    source: &str,
    definition: &str,
    field: &str,
    values: Vec<ResourceAmountSource>,
    known: &BTreeSet<ContentId>,
    require_nonzero: bool,
    diagnostics: &mut Vec<ContentDiagnostic>,
) -> ResourceStore {
    let mut quantities = BTreeMap::new();
    for (index, value) in values.into_iter().enumerate() {
        let item = format!("{definition}/{field}[{index}]");
        let Some(resource) = parse_id(source, &item, "resource", &value.resource, diagnostics)
        else {
            continue;
        };
        if !known.contains(&resource) {
            push(
                diagnostics,
                source,
                &item,
                "resource",
                format!("unknown resource {resource}"),
            );
        }
        if require_nonzero && value.quantity == 0 {
            push(diagnostics, source, &item, "quantity", "must be nonzero");
        }
        if quantities
            .insert(resource.clone(), value.quantity)
            .is_some()
        {
            push(
                diagnostics,
                source,
                &item,
                "resource",
                format!("duplicate resource {resource}"),
            );
        }
    }
    ResourceStore { quantities }
}

fn rate(
    source: &str,
    definition: &str,
    value: RateSource,
    diagnostics: &mut Vec<ContentDiagnostic>,
) -> FixedRate {
    let denominator = NonZeroU64::new(value.denominator).unwrap_or_else(|| {
        push(
            diagnostics,
            source,
            definition,
            "denominator",
            "must be nonzero",
        );
        NonZeroU64::MIN
    });
    let divisor = greatest_common_divisor(value.numerator, denominator.get());
    FixedRate::new(
        value.numerator / divisor,
        NonZeroU64::new(denominator.get() / divisor).expect("reduced denominator is nonzero"),
    )
}

fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

fn recipe(
    source: &str,
    name: &str,
    value: RecipeSource,
    known: &BTreeSet<ContentId>,
    diagnostics: &mut Vec<ContentDiagnostic>,
) -> ConstructionRecipe {
    ConstructionRecipe {
        cost: amounts(source, name, "costs", value.costs, known, true, diagnostics),
        required_work: value.required_work,
    }
}

fn compile_tuning(
    source_name: &str,
    value: TuningSource,
    resource_ids: &BTreeSet<ContentId>,
    diagnostics: &mut Vec<ContentDiagnostic>,
) -> WorldTuning {
    let tuning_source = value;
    let mut seasonal_shape = [0_u64; 10];
    if tuning_source.seasonal_shape.len() != 10 {
        push(
            diagnostics,
            source_name,
            "tuning",
            "seasonal_shape",
            "must contain exactly 10 entries",
        );
    }
    for (target, value) in seasonal_shape.iter_mut().zip(&tuning_source.seasonal_shape) {
        *target = *value;
    }
    let mut resource_richness = BTreeMap::new();
    for (index, value) in tuning_source.resource_richness.into_iter().enumerate() {
        let definition = format!("tuning.resource_richness[{index}]");
        let Some(resource) = parse_id(
            source_name,
            &definition,
            "resource",
            &value.resource,
            diagnostics,
        ) else {
            continue;
        };
        if resource_richness
            .insert(
                resource.clone(),
                RichnessThresholds {
                    poor_minimum: value.poor_minimum,
                    poor_maximum: value.poor_maximum,
                    normal_minimum: value.normal_minimum,
                    normal_maximum: value.normal_maximum,
                    rich_minimum: value.rich_minimum,
                },
            )
            .is_some()
        {
            push(
                diagnostics,
                source_name,
                definition,
                "resource",
                format!("duplicate resource {resource}"),
            );
        }
    }
    WorldTuning {
        energy_resource: parse_id(
            source_name,
            "tuning",
            "energy_resource",
            &tuning_source.energy_resource,
            diagnostics,
        )
        .unwrap_or_else(|| ContentId::new(ENERGY_ID).expect("constant valid")),
        ore_resource: parse_id(
            source_name,
            "tuning",
            "ore_resource",
            &tuning_source.ore_resource,
            diagnostics,
        )
        .unwrap_or_else(|| ContentId::new("core:ore").expect("constant valid")),
        alloy_resource: parse_id(
            source_name,
            "tuning",
            "alloy_resource",
            &tuning_source.alloy_resource,
            diagnostics,
        )
        .unwrap_or_else(|| ContentId::new("core:alloy").expect("constant valid")),
        seasonal_shape,
        seasonal_baseline_average: tuning_source.seasonal_baseline_average,
        life_support_per_population: tuning_source.life_support_per_population,
        origin_construction_work: tuning_source.origin_construction_work,
        intrinsic_energy_capacity: tuning_source.intrinsic_energy_capacity,
        battery_energy_capacity: tuning_source.battery_energy_capacity,
        habitat_population_energy: tuning_source.habitat_population_energy,
        coordinate_quanta_per_map_unit: tuning_source.coordinate_quanta_per_map_unit,
        collector_recipe: recipe(
            source_name,
            "tuning.collector_recipe",
            tuning_source.collector_recipe,
            resource_ids,
            diagnostics,
        ),
        battery_recipe: recipe(
            source_name,
            "tuning.battery_recipe",
            tuning_source.battery_recipe,
            resource_ids,
            diagnostics,
        ),
        extractor_recipe: recipe(
            source_name,
            "tuning.extractor_recipe",
            tuning_source.extractor_recipe,
            resource_ids,
            diagnostics,
        ),
        refinery_recipe: recipe(
            source_name,
            "tuning.refinery_recipe",
            tuning_source.refinery_recipe,
            resource_ids,
            diagnostics,
        ),
        habitat_recipe: recipe(
            source_name,
            "tuning.habitat_recipe",
            tuning_source.habitat_recipe,
            resource_ids,
            diagnostics,
        ),
        shipyard_recipe: recipe(
            source_name,
            "tuning.shipyard_recipe",
            tuning_source.shipyard_recipe,
            resource_ids,
            diagnostics,
        ),
        extractor: ExtractorParameters {
            energy_upkeep: tuning_source.extractor.energy_upkeep,
            cycle_duration: tuning_source.extractor.cycle_duration,
            output: tuning_source.extractor.output,
        },
        refinery: RefineryParameters {
            energy_upkeep: tuning_source.refinery.energy_upkeep,
            cycle_duration: tuning_source.refinery.cycle_duration,
            input: tuning_source.refinery.input,
            output: tuning_source.refinery.output,
        },
        probe_project: ProbeProjectTuning {
            material_commitment: amounts(
                source_name,
                "tuning.probe_project",
                "material_commitment",
                tuning_source.probe_project.material_commitment,
                resource_ids,
                true,
                diagnostics,
            ),
            duration_ticks: tuning_source.probe_project.duration_ticks,
            energy_per_progress_tick: tuning_source.probe_project.energy_per_progress_tick,
        },
        expedition_project: ExpeditionProjectTuning {
            hull_material_commitment: amounts(
                source_name,
                "tuning.expedition_project",
                "hull_material_commitment",
                tuning_source.expedition_project.hull_material_commitment,
                resource_ids,
                true,
                diagnostics,
            ),
            founding_stocks: amounts(
                source_name,
                "tuning.expedition_project",
                "founding_stocks",
                tuning_source.expedition_project.founding_stocks,
                resource_ids,
                false,
                diagnostics,
            ),
            duration_ticks: tuning_source.expedition_project.duration_ticks,
            energy_per_progress_tick: tuning_source.expedition_project.energy_per_progress_tick,
        },
        probe_travel: ShipTravelTuning {
            maximum_jump_quanta: tuning_source.probe_travel.maximum_jump_quanta,
            speed_quanta_per_tick: tuning_source.probe_travel.speed_quanta_per_tick,
            energy_per_quantum: rate(
                source_name,
                "tuning.probe_travel.energy_per_quantum",
                tuning_source.probe_travel.energy_per_quantum,
                diagnostics,
            ),
        },
        expedition_travel: ShipTravelTuning {
            maximum_jump_quanta: tuning_source.expedition_travel.maximum_jump_quanta,
            speed_quanta_per_tick: tuning_source.expedition_travel.speed_quanta_per_tick,
            energy_per_quantum: rate(
                source_name,
                "tuning.expedition_travel.energy_per_quantum",
                tuning_source.expedition_travel.energy_per_quantum,
                diagnostics,
            ),
        },
        probe_reveal_radius_quanta: tuning_source.probe_reveal_radius_quanta,
        communication_delay_per_quantum: rate(
            source_name,
            "tuning.communication_delay_per_quantum",
            tuning_source.communication_delay_per_quantum,
            diagnostics,
        ),
        resource_richness,
    }
}

fn triangle(value: TriangleSource) -> UnsignedTriangle {
    UnsignedTriangle {
        minimum: value.minimum,
        mode: value.mode,
        maximum: value.maximum,
    }
}

fn system_generation(value: SystemGenerationSource) -> SystemGenerationTuning {
    SystemGenerationTuning {
        strength_hundredths: triangle(value.strength_hundredths),
        body_count: triangle(value.body_count),
        eccentricity_hundredths: triangle(value.eccentricity_hundredths),
        slots_per_body: triangle(value.slots_per_body),
    }
}

fn compile_profile(
    source_name: &str,
    source: ProfileSource,
) -> Result<NormalizedProfile, ContentErrors> {
    let mut diagnostics = Vec::new();
    let mut resources = BTreeMap::new();
    for (index, item) in source.resources.into_iter().enumerate() {
        let definition = format!("resources[{index}]");
        let Some(id) = parse_id(source_name, &definition, "id", &item.id, &mut diagnostics) else {
            continue;
        };
        if resources
            .insert(
                id.clone(),
                ResourceDefinition {
                    id: id.clone(),
                    name: item.name,
                    naturally_deposit_bearing: item.naturally_deposit_bearing,
                },
            )
            .is_some()
        {
            push(
                &mut diagnostics,
                source_name,
                definition,
                "id",
                format!("duplicate id {id}"),
            );
        }
    }
    let resource_ids = resources.keys().cloned().collect::<BTreeSet<_>>();
    let gameplay = compile_tuning(
        source_name,
        source.gameplay,
        &resource_ids,
        &mut diagnostics,
    );
    let mut generated_resources = BTreeMap::new();
    for (index, value) in source.generator.resources.into_iter().enumerate() {
        let definition = format!("generator.resources[{index}]");
        let Some(resource) = parse_id(
            source_name,
            &definition,
            "resource",
            &value.resource,
            &mut diagnostics,
        ) else {
            continue;
        };
        let tuning = ResourceGenerationTuning {
            origin: OriginResourceGenerationTuning {
                resource_bearing_body_count: triangle(value.origin.resource_bearing_body_count),
                quantity_per_body: triangle(value.origin.quantity_per_body),
            },
            frontier: FrontierResourceGenerationTuning {
                presence_basis_points: value.frontier.presence_basis_points,
                resource_bearing_body_count: triangle(value.frontier.resource_bearing_body_count),
                quantity_per_body: triangle(value.frontier.quantity_per_body),
            },
        };
        if generated_resources
            .insert(resource.clone(), tuning)
            .is_some()
        {
            push(
                &mut diagnostics,
                source_name,
                definition,
                "resource",
                format!("duplicate resource {resource}"),
            );
        }
    }
    let persistence_divisor = greatest_common_divisor(
        source.generator.persistence.numerator,
        source.generator.persistence.denominator,
    );
    let generator = GeneratorTuning {
        coordinate_quanta_per_map_unit: source.generator.coordinate_quanta_per_map_unit,
        target_system_count: source.generator.target_system_count,
        x_bounds: SignedBounds {
            minimum: source.generator.x_bounds.minimum,
            maximum_exclusive: source.generator.x_bounds.maximum_exclusive,
        },
        y_bounds: SignedBounds {
            minimum: source.generator.y_bounds.minimum,
            maximum_exclusive: source.generator.y_bounds.maximum_exclusive,
        },
        generated_z: source.generator.generated_z,
        cell_width_quanta: source.generator.cell_width_quanta,
        cell_height_quanta: source.generator.cell_height_quanta,
        noise_octaves: source.generator.noise_octaves,
        base_wavelength_quanta: source.generator.base_wavelength_quanta,
        lacunarity: source.generator.lacunarity,
        persistence: UnsignedRatio {
            numerator: source.generator.persistence.numerator / persistence_divisor,
            denominator: source.generator.persistence.denominator / persistence_divisor,
        },
        full_cell_jitter: source.generator.full_cell_jitter,
        origin_system: system_generation(source.generator.origin_system),
        frontier_system: system_generation(source.generator.frontier_system),
        resources: generated_resources,
    };
    diagnostics.sort();
    if !diagnostics.is_empty() {
        return Err(ContentErrors(diagnostics));
    }
    NormalizedProfile::new(resources.into_values().collect(), gameplay, generator).map_err(
        |error| {
            ContentErrors::one(
                source_name.into(),
                "profile",
                "validation",
                error.to_string(),
            )
        },
    )
}

fn compile_world(source_name: &str, source: WorldSource) -> Result<WorldDefinition, ContentErrors> {
    let mut diagnostics = Vec::new();
    let mut resources = BTreeMap::new();
    for (index, item) in source.resources.into_iter().enumerate() {
        let definition = format!("resources[{index}]");
        let Some(id) = parse_id(source_name, &definition, "id", &item.id, &mut diagnostics) else {
            continue;
        };
        if resources
            .insert(
                id.clone(),
                ResourceDefinition {
                    id: id.clone(),
                    name: item.name,
                    naturally_deposit_bearing: item.naturally_deposit_bearing,
                },
            )
            .is_some()
        {
            push(
                &mut diagnostics,
                source_name,
                definition,
                "id",
                format!("duplicate id {id}"),
            );
        }
    }
    let resource_ids = resources.keys().cloned().collect::<BTreeSet<_>>();

    let mut locations = BTreeMap::new();
    for (index, item) in source.locations.into_iter().enumerate() {
        let definition = format!("locations[{index}]");
        let Some(id) = parse_id(source_name, &definition, "id", &item.id, &mut diagnostics) else {
            continue;
        };
        if locations
            .insert(
                id.clone(),
                LocationDefinition {
                    id: id.clone(),
                    name: item.name,
                    position: Position3::from_quanta(
                        item.position.x,
                        item.position.y,
                        item.position.z,
                    ),
                },
            )
            .is_some()
        {
            push(
                &mut diagnostics,
                source_name,
                definition,
                "id",
                format!("duplicate id {id}"),
            );
        }
    }
    let location_ids = locations.keys().cloned().collect::<BTreeSet<_>>();

    let mut communities = BTreeMap::new();
    for (index, item) in source.communities.into_iter().enumerate() {
        let definition = format!("communities[{index}]");
        let id = parse_id(source_name, &definition, "id", &item.id, &mut diagnostics);
        let system = parse_id(
            source_name,
            &definition,
            "system",
            &item.system,
            &mut diagnostics,
        );
        let Some((id, system)) = id.zip(system) else {
            continue;
        };
        if !location_ids.contains(&system) {
            push(
                &mut diagnostics,
                source_name,
                &definition,
                "system",
                format!("unknown system {system}"),
            );
        }
        if communities
            .insert(
                id.clone(),
                CommunityDefinition {
                    id: id.clone(),
                    system,
                },
            )
            .is_some()
        {
            push(
                &mut diagnostics,
                source_name,
                definition,
                "id",
                format!("duplicate id {id}"),
            );
        }
    }

    let mut development_ids = BTreeSet::new();
    let mut body_ids = BTreeSet::new();
    let mut systems = BTreeMap::new();
    for (system_index, item) in source.systems.into_iter().enumerate() {
        let definition = format!("systems[{system_index}]");
        let Some(location) = parse_id(
            source_name,
            &definition,
            "location",
            &item.location,
            &mut diagnostics,
        ) else {
            continue;
        };
        if !location_ids.contains(&location) {
            push(
                &mut diagnostics,
                source_name,
                &definition,
                "location",
                format!("unknown location {location}"),
            );
        }
        let stocks = amounts(
            source_name,
            &definition,
            "stocks",
            item.stocks,
            &resource_ids,
            false,
            &mut diagnostics,
        );
        let mut bodies = Vec::new();
        for (body_index, body) in item.bodies.into_iter().enumerate() {
            let body_definition = format!("{definition}/bodies[{body_index}]");
            let Some(body_id) = parse_id(
                source_name,
                &body_definition,
                "id",
                &body.id,
                &mut diagnostics,
            ) else {
                continue;
            };
            if !body_ids.insert(body_id.clone()) {
                push(
                    &mut diagnostics,
                    source_name,
                    &body_definition,
                    "id",
                    format!("duplicate body id {body_id}"),
                );
            }
            let initial_resources = amounts(
                source_name,
                &body_definition,
                "resources",
                body.resources,
                &resource_ids,
                true,
                &mut diagnostics,
            );
            let mut slot_ids = BTreeSet::new();
            let mut slots = Vec::new();
            for (slot_index, slot) in body.slots.into_iter().enumerate() {
                let slot_definition = format!("{body_definition}/slots[{slot_index}]");
                let Some(slot_id) = parse_id(
                    source_name,
                    &slot_definition,
                    "id",
                    &slot.id,
                    &mut diagnostics,
                ) else {
                    continue;
                };
                if !slot_ids.insert(slot_id.clone()) {
                    push(
                        &mut diagnostics,
                        source_name,
                        &slot_definition,
                        "id",
                        format!("duplicate slot id {slot_id}"),
                    );
                }
                let development = slot.development.and_then(|development| {
                    let id = parse_id(
                        source_name,
                        &slot_definition,
                        "development.id",
                        &development.id,
                        &mut diagnostics,
                    )?;
                    if !development_ids.insert(id.clone()) {
                        push(
                            &mut diagnostics,
                            source_name,
                            &slot_definition,
                            "development.id",
                            format!("duplicate development id {id}"),
                        );
                    }
                    let role = match development.role {
                        DevelopmentRoleSource::Collector => DevelopmentRole::Collector,
                        DevelopmentRoleSource::Battery => DevelopmentRole::Battery,
                        DevelopmentRoleSource::Extractor => DevelopmentRole::Extractor,
                        DevelopmentRoleSource::Refinery => DevelopmentRole::Refinery,
                        DevelopmentRoleSource::Habitat => DevelopmentRole::Habitat,
                        DevelopmentRoleSource::Shipyard => DevelopmentRole::Shipyard,
                    };
                    let condition = match development.condition {
                        DevelopmentConditionSource::Functional => DevelopmentCondition::Functional,
                        DevelopmentConditionSource::Damaged => DevelopmentCondition::Damaged,
                        DevelopmentConditionSource::Ruined => DevelopmentCondition::Ruined,
                    };
                    let extractor_target = development
                        .extractor_resource
                        .and_then(|raw| {
                            parse_id(
                                source_name,
                                &slot_definition,
                                "extractor_resource",
                                &raw,
                                &mut diagnostics,
                            )
                        })
                        .map(|resource| BodyResourceTarget {
                            body: body_id.clone(),
                            resource,
                        });
                    Some(DevelopmentDefinition {
                        id,
                        role,
                        condition,
                        extractor_target,
                    })
                });
                slots.push(DevelopmentSlotDefinition {
                    id: slot_id,
                    development,
                });
            }
            bodies.push(BodyDefinition {
                id: body_id,
                name: body.name,
                eccentricity_hundredths: body.eccentricity_hundredths,
                initial_resources,
                slots,
            });
        }
        if systems
            .insert(
                location.clone(),
                SystemDefinition {
                    location: location.clone(),
                    stellar_strength_hundredths: item.stellar_strength_hundredths,
                    bodies,
                    stocks,
                    player_founded: item.player_founded,
                    command_unlock_received: item.command_unlock_received,
                },
            )
            .is_some()
        {
            push(
                &mut diagnostics,
                source_name,
                definition,
                "location",
                format!("duplicate system {location}"),
            );
        }
    }

    let mut sites = BTreeMap::new();
    for (index, item) in source.sites.into_iter().enumerate() {
        let definition = format!("sites[{index}]");
        let id = parse_id(source_name, &definition, "id", &item.id, &mut diagnostics);
        let location = parse_id(
            source_name,
            &definition,
            "location",
            &item.location,
            &mut diagnostics,
        );
        let Some((id, location)) = id.zip(location) else {
            continue;
        };
        if !location_ids.contains(&location) {
            push(
                &mut diagnostics,
                source_name,
                &definition,
                "location",
                format!("unknown location {location}"),
            );
        }
        if sites
            .insert(
                id.clone(),
                ReclaimableSiteDefinition {
                    id: id.clone(),
                    location,
                },
            )
            .is_some()
        {
            push(
                &mut diagnostics,
                source_name,
                definition,
                "id",
                format!("duplicate id {id}"),
            );
        }
    }

    let origin_system = parse_id(
        source_name,
        "origin",
        "system",
        &source.origin.system,
        &mut diagnostics,
    );
    let origin_community = parse_id(
        source_name,
        "origin",
        "community",
        &source.origin.community,
        &mut diagnostics,
    );
    if let Some(value) = &origin_system
        && !location_ids.contains(value)
    {
        push(
            &mut diagnostics,
            source_name,
            "origin",
            "system",
            format!("unknown system {value}"),
        );
    }
    if let Some(value) = &origin_community
        && !communities.contains_key(value)
    {
        push(
            &mut diagnostics,
            source_name,
            "origin",
            "community",
            format!("unknown community {value}"),
        );
    }

    let tuning = compile_tuning(source_name, source.tuning, &resource_ids, &mut diagnostics);

    diagnostics.sort();
    if !diagnostics.is_empty() {
        return Err(ContentErrors(diagnostics));
    }
    let definition = WorldDefinition {
        resources: resources.into_values().collect(),
        locations: locations.into_values().collect(),
        origin_system: origin_system.expect("validated"),
        origin_community: origin_community.expect("validated"),
        communities: communities.into_values().collect(),
        population_tokens: Vec::new(),
        systems: systems.into_values().collect(),
        sites: sites.into_values().collect(),
        tuning,
    };
    if let Err(error) = WorldState::new(definition.clone()) {
        return Err(ContentErrors::one(
            source_name.into(),
            "world",
            "validation",
            error.to_string(),
        ));
    }
    Ok(definition)
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;

    fn id(value: &str) -> ContentId {
        ContentId::new(value).expect("test id is valid")
    }

    #[test]
    fn retained_stage4_bootstrap_runs_on_body_resources_and_global_time() {
        let definition = compile_str(
            "stage4_origin.ron",
            include_str!("../tests/fixtures/stage4_origin.ron"),
        )
        .expect("fixture compiles");
        let mut state = WorldState::new(definition).expect("fixture instantiates");
        let origin = id("core:origin");
        let body = id("core:origin_body_0");

        state
            .enqueue_construction(
                &origin,
                &body,
                &id("core:slot_1"),
                DevelopmentRole::Refinery,
                None,
            )
            .expect("Refinery enqueues");
        let after_enqueue = state.debug_system_snapshot(&origin).expect("snapshot");
        assert_eq!(after_enqueue.stocks.quantity(&id(ENERGY_ID)), 0);
        assert_eq!(after_enqueue.stocks.quantity(&id("core:ore")), 8);

        for _ in 0..4 {
            state.advance_tick().expect("tick succeeds");
        }
        let tick4 = state.debug_system_snapshot(&origin).expect("snapshot");
        assert_eq!(
            (
                tick4.stocks.quantity(&id(ENERGY_ID)),
                tick4.stocks.quantity(&id("core:ore")),
                tick4.stocks.quantity(&id("core:alloy"))
            ),
            (10, 8, 0)
        );
        assert_eq!(tick4.energy_overflow.cumulative, 120);

        for _ in 4..8 {
            state.advance_tick().expect("tick succeeds");
        }
        let tick8 = state.debug_system_snapshot(&origin).expect("snapshot");
        assert_eq!(
            (
                tick8.stocks.quantity(&id(ENERGY_ID)),
                tick8.stocks.quantity(&id("core:ore")),
                tick8.stocks.quantity(&id("core:alloy"))
            ),
            (10, 0, 4)
        );
        assert_eq!(tick8.energy_overflow.cumulative, 150);

        state
            .enqueue_construction(
                &origin,
                &body,
                &id("core:slot_2"),
                DevelopmentRole::Battery,
                None,
            )
            .expect("Battery enqueues");
        for _ in 8..12 {
            state.advance_tick().expect("tick succeeds");
        }
        let tick12 = state.debug_system_snapshot(&origin).expect("snapshot");
        assert_eq!(
            (
                tick12.stocks.quantity(&id(ENERGY_ID)),
                tick12.stocks.quantity(&id("core:ore")),
                tick12.stocks.quantity(&id("core:alloy"))
            ),
            (50, 0, 2)
        );
        assert_eq!(tick12.energy_overflow.cumulative, 260);

        state
            .enqueue_construction(
                &origin,
                &body,
                &id("core:slot_3"),
                DevelopmentRole::Extractor,
                Some(&id("core:ore")),
            )
            .expect("Extractor enqueues");
        for _ in 12..16 {
            state.advance_tick().expect("tick succeeds");
        }
        let tick16 = state.debug_system_snapshot(&origin).expect("snapshot");
        assert_eq!(
            (
                tick16.stocks.quantity(&id(ENERGY_ID)),
                tick16.stocks.quantity(&id("core:ore")),
                tick16.stocks.quantity(&id("core:alloy"))
            ),
            (110, 0, 0)
        );

        for _ in 16..20 {
            state.advance_tick().expect("tick succeeds");
        }
        let tick20 = state.debug_system_snapshot(&origin).expect("snapshot");
        assert_eq!(
            (
                tick20.stocks.quantity(&id(ENERGY_ID)),
                tick20.stocks.quantity(&id("core:ore")),
                tick20.stocks.quantity(&id("core:alloy"))
            ),
            (110, 0, 2)
        );
        assert_eq!(
            tick20.bodies[0]
                .remaining_resources
                .quantity(&id("core:ore")),
            196
        );
        assert_eq!(tick20.energy_overflow.cumulative, 330);
        assert_eq!(state.time().tick, 20);
    }

    #[test]
    fn authored_origin_bootstraps_its_first_population_through_a_habitat() {
        let definition = compile_str(
            "stage4_origin.ron",
            include_str!("../tests/fixtures/stage4_origin.ron"),
        )
        .expect("fixture compiles");
        let mut state = WorldState::new(definition).expect("fixture instantiates");
        let origin = id("core:origin");
        let body = id("core:origin_body_0");

        state
            .enqueue_construction(
                &origin,
                &body,
                &id("core:slot_1"),
                DevelopmentRole::Refinery,
                None,
            )
            .unwrap();
        for _ in 0..8 {
            state.advance_tick().unwrap();
        }
        state
            .enqueue_construction(
                &origin,
                &body,
                &id("core:slot_2"),
                DevelopmentRole::Battery,
                None,
            )
            .unwrap();
        for _ in 0..4 {
            state.advance_tick().unwrap();
        }
        state
            .enqueue_construction(
                &origin,
                &body,
                &id("core:slot_3"),
                DevelopmentRole::Extractor,
                Some(&id("core:ore")),
            )
            .unwrap();
        for _ in 0..12 {
            state.advance_tick().unwrap();
        }
        state
            .enqueue_construction(
                &origin,
                &body,
                &id("core:slot_4"),
                DevelopmentRole::Habitat,
                None,
            )
            .expect("authored infrastructure can fund a Habitat");
        for _ in 0..8 {
            state.advance_tick().unwrap();
        }

        for _ in 0..100 {
            if state.debug_snapshot().populations.tokens.len() == 1 {
                break;
            }
            state.advance_tick().unwrap();
        }
        let snapshot = state.debug_snapshot();
        assert_eq!(snapshot.populations.tokens.len(), 1);
        assert_eq!(snapshot.population_accounting.generated, 1);
        assert_eq!(state.commandability(&origin), Ok(Commandability::Origin));
        assert!(matches!(
            snapshot.populations.tokens.values().next().unwrap().state,
            PopulationState::Resident { .. }
        ));
    }

    #[test]
    fn body_resources_keep_distinct_initial_and_remaining_authorities() {
        let definition = compile_str(
            "stage4_origin.ron",
            include_str!("../tests/fixtures/stage4_origin.ron"),
        )
        .expect("fixture compiles");
        let state = WorldState::new(definition).expect("fixture instantiates");
        let snapshot = state
            .debug_system_snapshot(&id("core:origin"))
            .expect("snapshot");
        assert_eq!(snapshot.initial_resource_total(&id("core:ore")), Ok(200));
        assert_eq!(snapshot.remaining_resource_total(&id("core:ore")), Ok(200));
        assert_eq!(
            snapshot.bodies[0]
                .initial_resources
                .quantity(&id("core:ore")),
            200
        );
    }

    #[test]
    fn construction_cancellation_refunds_and_never_reuses_project_ids() {
        let definition = compile_str(
            "stage4_origin.ron",
            include_str!("../tests/fixtures/stage4_origin.ron"),
        )
        .expect("fixture compiles");
        let mut state = WorldState::new(definition).expect("fixture instantiates");
        let origin = id("core:origin");
        let project = state
            .enqueue_construction(
                &origin,
                &id("core:origin_body_0"),
                &id("core:slot_1"),
                DevelopmentRole::Refinery,
                None,
            )
            .expect("enqueue");
        state
            .cancel_construction(&project)
            .expect("unbegun cancellation");
        let snapshot = state.debug_system_snapshot(&origin).expect("snapshot");
        assert_eq!(snapshot.stocks.quantity(&id(ENERGY_ID)), 10);
        assert_eq!(snapshot.stocks.quantity(&id("core:ore")), 10);
        assert!(snapshot.construction_queue.is_empty());
        assert_eq!(snapshot.counters.next_project_sequence, 1);
        assert!(snapshot.bodies[0].slots[1].reserved_by.is_none());
    }

    #[test]
    fn same_body_extractors_contend_in_stable_slot_order() {
        let mut definition = compile_str(
            "stage4_origin.ron",
            include_str!("../tests/fixtures/stage4_origin.ron"),
        )
        .expect("fixture compiles");
        let body = &mut definition.systems[0].bodies[0];
        body.initial_resources.set(id("core:ore"), 1);
        body.slots[1].id = id("core:z_first_slot");
        body.slots[2].id = id("core:a_second_slot");
        definition.tuning.extractor.cycle_duration = 2;
        for (slot_index, name) in [(1_usize, "core:extractor_z"), (2, "core:extractor_a")] {
            body.slots[slot_index].development = Some(DevelopmentDefinition {
                id: id(name),
                role: DevelopmentRole::Extractor,
                condition: DevelopmentCondition::Functional,
                extractor_target: Some(BodyResourceTarget {
                    body: body.id.clone(),
                    resource: id("core:ore"),
                }),
            });
        }
        let mut state =
            WorldState::new(definition).expect("two Extractors may share a body resource");
        state.advance_tick().expect("first progress tick succeeds");
        state.advance_tick().expect("contention tick succeeds");
        let snapshot = state
            .debug_system_snapshot(&id("core:origin"))
            .expect("snapshot");
        assert_eq!(snapshot.remaining_resource_total(&id("core:ore")), Ok(0));
        assert_eq!(snapshot.stocks.quantity(&id("core:ore")), 11);
        assert_eq!(
            snapshot.bodies[0].slots[1]
                .development
                .as_ref()
                .unwrap()
                .cycle
                .progress,
            0,
            "authored-first lexical-z slot wins"
        );
        assert_eq!(
            snapshot.bodies[0].slots[2]
                .development
                .as_ref()
                .unwrap()
                .cycle
                .progress,
            1,
            "lexical-a slot remains behind despite its lower ID"
        );
    }

    #[test]
    fn unordered_definition_collection_permutations_produce_equal_snapshots() {
        let definition = compile_str(
            "stage4_origin.ron",
            include_str!("../tests/fixtures/stage4_origin.ron"),
        )
        .expect("fixture compiles");
        let mut permuted = definition.clone();
        permuted.resources.reverse();
        permuted.locations.reverse();
        permuted.systems.reverse();
        assert_eq!(
            WorldState::new(definition).expect("valid").debug_snapshot(),
            WorldState::new(permuted).expect("valid").debug_snapshot()
        );
    }

    #[test]
    fn normalized_map_retains_semantic_body_and_slot_order_separately_from_runtime() {
        let mut definition = compile_str(
            "stage4_origin.ron",
            include_str!("../tests/fixtures/stage4_origin.ron"),
        )
        .expect("fixture compiles");
        let bodies = &mut definition.systems[0].bodies;
        bodies[0].id = id("core:z_first_body");
        bodies[1].id = id("core:a_second_body");
        bodies[0].slots[0].id = id("core:z_first_slot");
        bodies[0].slots[1].id = id("core:a_second_slot");

        let state = WorldState::new(definition).expect("semantic vector order is valid");
        let debug = state.debug_snapshot();
        let map = debug
            .map_systems
            .iter()
            .find(|system| system.location == id("core:origin"))
            .unwrap();
        assert_eq!(map.bodies[0].id, id("core:z_first_body"));
        assert_eq!(map.bodies[1].id, id("core:a_second_body"));
        assert_eq!(
            map.bodies[0].slots[..2],
            [id("core:z_first_slot"), id("core:a_second_slot")]
        );
        let runtime = state.debug_system_snapshot(&id("core:origin")).unwrap();
        assert_eq!(runtime.bodies[0].id, map.bodies[0].id);
        assert_eq!(
            runtime.bodies[0].initial_resources,
            map.bodies[0].initial_resources
        );
    }

    #[test]
    fn global_tick_persists_and_advances_neutral_systems() {
        let mut definition = compile_str(
            "stage4_origin.ron",
            include_str!("../tests/fixtures/stage4_origin.ron"),
        )
        .expect("fixture compiles");
        let frontier = id("core:frontier");
        definition.locations.push(LocationDefinition {
            id: frontier.clone(),
            name: "Frontier".into(),
            position: Position3::from_quanta(300, 400, 0),
        });
        definition.systems.push(SystemDefinition {
            location: frontier.clone(),
            stellar_strength_hundredths: 100,
            bodies: vec![BodyDefinition {
                id: id("core:frontier_body"),
                name: "Frontier Body".into(),
                eccentricity_hundredths: 100,
                initial_resources: ResourceStore::new(),
                slots: vec![DevelopmentSlotDefinition {
                    id: id("core:frontier_slot"),
                    development: Some(DevelopmentDefinition {
                        id: id("core:frontier_collector"),
                        role: DevelopmentRole::Collector,
                        condition: DevelopmentCondition::Functional,
                        extractor_target: None,
                    }),
                }],
            }],
            stocks: [(id(ENERGY_ID), 0)].into_iter().collect(),
            player_founded: false,
            command_unlock_received: false,
        });
        let mut state = WorldState::new(definition).expect("two-system definition is valid");
        state.advance_tick().expect("global tick succeeds");
        let snapshot = state
            .debug_system_snapshot(&frontier)
            .expect("frontier snapshot");
        assert_eq!(snapshot.stocks.quantity(&id(ENERGY_ID)), 10);
        assert_eq!(snapshot.energy_overflow.cumulative, 30);
        assert!(!snapshot.player_founded);
        assert_eq!(snapshot.commandability, Commandability::Neutral);
        assert_eq!(
            state.debug_snapshot().knowledge.level(&frontier),
            KnowledgeLevel::IdentifiedSummary
        );
        assert_eq!(
            state.commandability(&id("core:origin")),
            Ok(Commandability::Origin)
        );
    }

    #[test]
    fn late_system_failure_rolls_back_the_whole_world_and_clock() {
        let mut definition = compile_str(
            "stage4_origin.ron",
            include_str!("../tests/fixtures/stage4_origin.ron"),
        )
        .expect("fixture compiles");
        definition.tuning.intrinsic_energy_capacity = u64::MAX;
        let failing = id("core:failing");
        definition.locations.push(LocationDefinition {
            id: failing.clone(),
            name: "Failing".into(),
            position: Position3::from_quanta(1, 0, 0),
        });
        definition.systems.push(SystemDefinition {
            location: failing.clone(),
            stellar_strength_hundredths: 100,
            bodies: vec![BodyDefinition {
                id: id("core:failing_body"),
                name: "Failing Body".into(),
                eccentricity_hundredths: 100,
                initial_resources: ResourceStore::new(),
                slots: vec![DevelopmentSlotDefinition {
                    id: id("core:failing_slot"),
                    development: Some(DevelopmentDefinition {
                        id: id("core:failing_collector"),
                        role: DevelopmentRole::Collector,
                        condition: DevelopmentCondition::Functional,
                        extractor_target: None,
                    }),
                }],
            }],
            stocks: [(id(ENERGY_ID), u64::MAX)].into_iter().collect(),
            player_founded: false,
            command_unlock_received: false,
        });
        let mut state = WorldState::new(definition).expect("overflow setup is structurally valid");
        let before = state.debug_snapshot();
        assert_eq!(state.advance_tick(), Err(CoreError::Overflow));
        assert_eq!(state.debug_snapshot(), before);
        assert_eq!(state.time().tick, 0);
    }

    #[test]
    fn strict_schema_rejects_removed_topology_and_population_fields() {
        let old = "(resources: [], locations: [], origin: (id: \"core:c\", location: \"core:o\", population: 0), systems: [], topology: (edges: []), tuning: ())";
        let error = compile_str("old.ron", old).expect_err("removed fields are rejected");
        assert_eq!(error.diagnostics()[0].definition, "document");
        assert_eq!(error.diagnostics()[0].field, "parse");
    }

    #[test]
    fn tuning_derives_the_complete_expedition_commitment() {
        let definition = compile_str(
            "stage4_origin.ron",
            include_str!("../tests/fixtures/stage4_origin.ron"),
        )
        .expect("fixture compiles");
        let commitment = definition
            .tuning
            .expedition_enqueue_commitment()
            .expect("commitment adds without overflow");
        assert_eq!(commitment.quantity(&id(ENERGY_ID)), 60);
        assert_eq!(commitment.quantity(&id("core:ore")), 10);
        assert_eq!(commitment.quantity(&id("core:alloy")), 8);
    }

    #[test]
    fn generation_request_carries_normalized_configuration_and_artifact_provenance() {
        let definition = compile_str(
            "stage4_origin.ron",
            include_str!("../tests/fixtures/stage4_origin.ron"),
        )
        .expect("fixture compiles");
        let origin_system = SystemGenerationTuning {
            strength_hundredths: UnsignedTriangle {
                minimum: 100,
                mode: 100,
                maximum: 100,
            },
            body_count: UnsignedTriangle {
                minimum: 4,
                mode: 4,
                maximum: 12,
            },
            eccentricity_hundredths: UnsignedTriangle {
                minimum: 100,
                mode: 100,
                maximum: 100,
            },
            slots_per_body: UnsignedTriangle {
                minimum: 3,
                mode: 3,
                maximum: 8,
            },
        };
        let frontier_system = SystemGenerationTuning {
            strength_hundredths: UnsignedTriangle {
                minimum: 10,
                mode: 100,
                maximum: 300,
            },
            body_count: UnsignedTriangle {
                minimum: 1,
                mode: 4,
                maximum: 12,
            },
            eccentricity_hundredths: UnsignedTriangle {
                minimum: 0,
                mode: 100,
                maximum: 150,
            },
            slots_per_body: UnsignedTriangle {
                minimum: 1,
                mode: 3,
                maximum: 8,
            },
        };
        let normalized = NormalizedProfile::new(
            definition.resources.clone(),
            definition.tuning.clone(),
            GeneratorTuning {
                coordinate_quanta_per_map_unit: 100,
                target_system_count: 128,
                x_bounds: SignedBounds {
                    minimum: -5_000,
                    maximum_exclusive: 5_000,
                },
                y_bounds: SignedBounds {
                    minimum: -5_000,
                    maximum_exclusive: 5_000,
                },
                generated_z: 0,
                cell_width_quanta: 500,
                cell_height_quanta: 500,
                noise_octaves: 4,
                base_wavelength_quanta: 4_000,
                lacunarity: 2,
                persistence: UnsignedRatio {
                    numerator: 1,
                    denominator: 2,
                },
                full_cell_jitter: true,
                origin_system,
                frontier_system,
                resources: BTreeMap::from([(
                    id("core:ore"),
                    ResourceGenerationTuning {
                        origin: OriginResourceGenerationTuning {
                            resource_bearing_body_count: UnsignedTriangle {
                                minimum: 1,
                                mode: 2,
                                maximum: 4,
                            },
                            quantity_per_body: UnsignedTriangle {
                                minimum: 200,
                                mode: 300,
                                maximum: 500,
                            },
                        },
                        frontier: FrontierResourceGenerationTuning {
                            presence_basis_points: 6_500,
                            resource_bearing_body_count: UnsignedTriangle {
                                minimum: 1,
                                mode: 1,
                                maximum: 4,
                            },
                            quantity_per_body: UnsignedTriangle {
                                minimum: 50,
                                mode: 200,
                                maximum: 500,
                            },
                        },
                    },
                )]),
            },
        )
        .expect("approved starter values normalize");
        let provenance = SourceProvenance::from_source("core:starter", b"source");
        let compiled = CompiledProfile::new(normalized, provenance.clone())
            .expect("normalized profile canonically encodes");
        let expected_fingerprint = compiled.fingerprint();
        let request = GenerationRequest {
            version: GeneratorVersion::frontier_revision_1(),
            seed: 42,
            configuration: compiled,
        };
        let artifact = GeneratedWorldArtifact::from_generated_definition(&request, definition);
        assert_eq!(artifact.identity.seed, 42);
        assert_eq!(artifact.provenance, provenance);
        assert_eq!(
            artifact.identity.configuration_fingerprint,
            expected_fingerprint
        );
        assert_eq!(
            request
                .configuration
                .normalized()
                .generator()
                .target_system_count,
            128
        );
    }

    #[test]
    fn sha256_hook_is_stable() {
        assert_eq!(
            sha256_fingerprint(b"abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad
            ]
        );
    }
}
