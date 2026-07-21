//! Source-aware RON loading and validation for Stage 3 substrate and Stage 4 resource-engine worlds.

use game_core::{
    BodyDefinition, ConstructionRecipe, ContentId, DevelopmentCondition, DevelopmentDefinition,
    DevelopmentRole, DevelopmentSlotDefinition, ENERGY_ID, ExtractorParameters, LocationDefinition,
    OriginCommunityDefinition, Position3, ReclaimableSiteDefinition, RefineryParameters,
    ResourceDefinition, ResourceDepositDefinition, ResourceEngineConfig, ResourceEngineDefinition,
    ResourceStore, SystemDefinition, TopologyDefinition, TopologyEdge, WorldDefinition,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;
use thiserror::Error;

/// One actionable content-validation problem, including its source provenance.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ContentDiagnostic {
    pub source: String,
    pub definition: String,
    pub field: String,
    pub message: String,
}

impl Display for ContentDiagnostic {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}:{}:{}: {}",
            self.source, self.definition, self.field, self.message
        )
    }
}

/// Deterministically ordered diagnostics emitted by parsing or compiling content.
#[derive(Debug, Error)]
#[error("content compilation failed:\n{}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"))]
pub struct ContentErrors(pub Vec<ContentDiagnostic>);

impl ContentErrors {
    #[must_use]
    pub fn diagnostics(&self) -> &[ContentDiagnostic] {
        &self.0
    }

    fn from_one(diagnostic: ContentDiagnostic) -> Self {
        Self(vec![diagnostic])
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldSource {
    resources: Vec<ResourceSource>,
    locations: Vec<LocationSource>,
    origin: OriginSource,
    #[serde(default)]
    systems: Vec<SystemSource>,
    #[serde(default)]
    deposits: Vec<DepositSource>,
    #[serde(default)]
    sites: Vec<SiteSource>,
    #[serde(default)]
    topology: TopologySource,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceSource {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocationSource {
    id: String,
    name: String,
    position: PositionSource,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PositionSource {
    x: f64,
    y: f64,
    z: f64,
}

/// Community source data deliberately contains no physical stock ledger.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OriginSource {
    id: String,
    location: String,
    population: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SystemSource {
    location: String,
    #[serde(default)]
    stocks: Vec<ResourceAmountSource>,
    #[serde(default)]
    resource_engine: Option<ResourceEngineSource>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceAmountSource {
    resource: String,
    quantity: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceEngineSource {
    collector_energy_profile: Vec<u64>,
    bodies: Vec<BodySource>,
    config: ResourceEngineConfigSource,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BodySource {
    id: String,
    name: String,
    slots: Vec<SlotSource>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SlotSource {
    id: String,
    #[serde(default)]
    development: Option<DevelopmentSource>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum DevelopmentRoleSource {
    Collector,
    Battery,
    Extractor,
    Refinery,
}

impl From<DevelopmentRoleSource> for DevelopmentRole {
    fn from(value: DevelopmentRoleSource) -> Self {
        match value {
            DevelopmentRoleSource::Collector => Self::Collector,
            DevelopmentRoleSource::Battery => Self::Battery,
            DevelopmentRoleSource::Extractor => Self::Extractor,
            DevelopmentRoleSource::Refinery => Self::Refinery,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum DevelopmentConditionSource {
    Functional,
    Damaged,
    Ruined,
}

impl From<DevelopmentConditionSource> for DevelopmentCondition {
    fn from(value: DevelopmentConditionSource) -> Self {
        match value {
            DevelopmentConditionSource::Functional => Self::Functional,
            DevelopmentConditionSource::Damaged => Self::Damaged,
            DevelopmentConditionSource::Ruined => Self::Ruined,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DevelopmentSource {
    id: String,
    role: DevelopmentRoleSource,
    condition: DevelopmentConditionSource,
    #[serde(default)]
    extractor_deposit: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceEngineConfigSource {
    energy_resource: String,
    ore_resource: String,
    alloy_resource: String,
    life_support_per_population: u64,
    origin_construction_work: u64,
    intrinsic_energy_capacity: u64,
    battery_energy_capacity: u64,
    collector_recipe: ConstructionRecipeSource,
    battery_recipe: ConstructionRecipeSource,
    extractor_recipe: ConstructionRecipeSource,
    refinery_recipe: ConstructionRecipeSource,
    extractor: ExtractorParametersSource,
    refinery: RefineryParametersSource,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstructionRecipeSource {
    costs: Vec<ResourceAmountSource>,
    required_work: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtractorParametersSource {
    energy_upkeep: u64,
    cycle_duration: u64,
    ore_output: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefineryParametersSource {
    energy_upkeep: u64,
    cycle_duration: u64,
    ore_input: u64,
    alloy_output: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DepositSource {
    id: String,
    location: String,
    resource: String,
    quantity: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteSource {
    id: String,
    location: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct TopologySource {
    #[serde(default)]
    edges: Vec<EdgeSource>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeSource {
    from: String,
    to: String,
}

/// Compiles one RON world source into a format-independent definition.
///
/// Parsing errors have document provenance. Semantic errors are collected before
/// any core definition is returned, then sorted by source, definition, and field.
pub fn compile_str(
    source_name: impl AsRef<str>,
    source: &str,
) -> Result<WorldDefinition, ContentErrors> {
    let source_name = source_name.as_ref().to_owned();
    let parsed = ron::from_str::<WorldSource>(source).map_err(|error| {
        ContentErrors::from_one(ContentDiagnostic {
            source: source_name.clone(),
            definition: "document".into(),
            field: "parse".into(),
            message: error.to_string(),
        })
    })?;
    compile_world(source_name, parsed)
}

/// Loads and compiles exactly one world source file; it has no repository-bundle convention.
pub fn load_file(path: impl AsRef<Path>) -> Result<WorldDefinition, ContentErrors> {
    let path = path.as_ref();
    let source_name = path.display().to_string();
    let source = fs::read_to_string(path).map_err(|error| {
        ContentErrors::from_one(ContentDiagnostic {
            source: source_name.clone(),
            definition: "document".into(),
            field: "read".into(),
            message: error.to_string(),
        })
    })?;
    compile_str(source_name, &source)
}

fn compile_world(
    source_name: String,
    source: WorldSource,
) -> Result<WorldDefinition, ContentErrors> {
    let mut diagnostics = Vec::new();

    let mut resources = BTreeMap::new();
    for (index, item) in source.resources.into_iter().enumerate() {
        let definition = definition_name("resources", index, &item.id);
        let Some(id) = parse_id(&source_name, &definition, "id", &item.id, &mut diagnostics) else {
            continue;
        };
        if resources.contains_key(&id) {
            push(
                &mut diagnostics,
                &source_name,
                definition,
                "id",
                format!("duplicate id {id}"),
            );
            continue;
        }
        resources.insert(
            id.clone(),
            ResourceDefinition {
                id,
                name: item.name,
            },
        );
    }

    let mut locations = BTreeMap::new();
    for (index, item) in source.locations.into_iter().enumerate() {
        let definition = definition_name("locations", index, &item.id);
        let id = parse_id(&source_name, &definition, "id", &item.id, &mut diagnostics);
        let position = Position3 {
            x: item.position.x,
            y: item.position.y,
            z: item.position.z,
        };
        if !position.is_finite() {
            push(
                &mut diagnostics,
                &source_name,
                definition.clone(),
                "position",
                "coordinates must be finite",
            );
        }
        let Some(id) = id else { continue };
        if locations.contains_key(&id) {
            push(
                &mut diagnostics,
                &source_name,
                definition,
                "id",
                format!("duplicate id {id}"),
            );
            continue;
        }
        locations.insert(
            id.clone(),
            LocationDefinition {
                id,
                name: item.name,
                position,
            },
        );
    }

    let origin_definition = definition_name("origin", 0, &source.origin.id);
    let origin_id = parse_id(
        &source_name,
        &origin_definition,
        "id",
        &source.origin.id,
        &mut diagnostics,
    );
    let origin_location = parse_id(
        &source_name,
        &origin_definition,
        "location",
        &source.origin.location,
        &mut diagnostics,
    );
    if let Some(location) = &origin_location
        && !locations.contains_key(location)
    {
        push(
            &mut diagnostics,
            &source_name,
            origin_definition.clone(),
            "location",
            format!("unknown location {location}"),
        );
    }

    let mut deposits = BTreeMap::new();
    for (index, item) in source.deposits.into_iter().enumerate() {
        let definition = definition_name("deposits", index, &item.id);
        let id = parse_id(&source_name, &definition, "id", &item.id, &mut diagnostics);
        let location = parse_id(
            &source_name,
            &definition,
            "location",
            &item.location,
            &mut diagnostics,
        );
        let resource = parse_id(
            &source_name,
            &definition,
            "resource",
            &item.resource,
            &mut diagnostics,
        );
        if let Some(location) = &location
            && !locations.contains_key(location)
        {
            push(
                &mut diagnostics,
                &source_name,
                definition.clone(),
                "location",
                format!("unknown location {location}"),
            );
        }
        if let Some(resource) = &resource
            && !resources.contains_key(resource)
        {
            push(
                &mut diagnostics,
                &source_name,
                definition.clone(),
                "resource",
                format!("unknown resource {resource}"),
            );
        }
        require_nonzero(
            &source_name,
            &definition,
            "quantity",
            item.quantity,
            &mut diagnostics,
        );
        let Some((id, location, resource)) = id
            .zip(location)
            .zip(resource)
            .map(|((id, location), resource)| (id, location, resource))
        else {
            continue;
        };
        if deposits.contains_key(&id) {
            push(
                &mut diagnostics,
                &source_name,
                definition,
                "id",
                format!("duplicate id {id}"),
            );
            continue;
        }
        deposits.insert(
            id.clone(),
            ResourceDepositDefinition {
                id,
                location,
                resource,
                quantity: item.quantity,
            },
        );
    }

    let mut sites = BTreeMap::new();
    for (index, item) in source.sites.into_iter().enumerate() {
        let definition = definition_name("sites", index, &item.id);
        let id = parse_id(&source_name, &definition, "id", &item.id, &mut diagnostics);
        let location = parse_id(
            &source_name,
            &definition,
            "location",
            &item.location,
            &mut diagnostics,
        );
        if let Some(location) = &location
            && !locations.contains_key(location)
        {
            push(
                &mut diagnostics,
                &source_name,
                definition.clone(),
                "location",
                format!("unknown location {location}"),
            );
        }
        let Some((id, location)) = id.zip(location) else {
            continue;
        };
        if sites.contains_key(&id) {
            push(
                &mut diagnostics,
                &source_name,
                definition,
                "id",
                format!("duplicate id {id}"),
            );
            continue;
        }
        sites.insert(id.clone(), ReclaimableSiteDefinition { id, location });
    }

    let mut systems = BTreeMap::new();
    for (index, item) in source.systems.into_iter().enumerate() {
        let definition = definition_name("systems", index, &item.location);
        let location = parse_id(
            &source_name,
            &definition,
            "location",
            &item.location,
            &mut diagnostics,
        );
        if let Some(location) = &location
            && !locations.contains_key(location)
        {
            push(
                &mut diagnostics,
                &source_name,
                definition.clone(),
                "location",
                format!("unknown location {location}"),
            );
        }
        let stocks = compile_amounts(
            &source_name,
            &definition,
            "stocks",
            item.stocks,
            &resources,
            false,
            &mut diagnostics,
        );
        let resource_engine = item.resource_engine.and_then(|engine| {
            compile_resource_engine(
                &source_name,
                &definition,
                location.as_ref(),
                engine,
                &resources,
                &deposits,
                &stocks,
                &mut diagnostics,
            )
        });
        let Some(location) = location else { continue };
        if systems.contains_key(&location) {
            push(
                &mut diagnostics,
                &source_name,
                definition,
                "location",
                format!("duplicate system for location {location}"),
            );
            continue;
        }
        systems.insert(
            location.clone(),
            SystemDefinition {
                location,
                stocks,
                resource_engine,
            },
        );
    }

    let mut edges = BTreeMap::new();
    let mut seen_edges = BTreeSet::new();
    for (index, item) in source.topology.edges.into_iter().enumerate() {
        let fallback = format!("topology.edges[{index}]");
        let from = parse_id(
            &source_name,
            &fallback,
            "from",
            &item.from,
            &mut diagnostics,
        );
        let to = parse_id(&source_name, &fallback, "to", &item.to, &mut diagnostics);
        let Some((mut from, mut to)) = from.zip(to) else {
            continue;
        };
        if to < from {
            std::mem::swap(&mut from, &mut to);
        }
        let definition = format!("topology:{from}/{to}");
        if from == to {
            push(
                &mut diagnostics,
                &source_name,
                definition.clone(),
                "endpoints",
                "self edge is not allowed",
            );
        }
        for endpoint in [&from, &to] {
            if !locations.contains_key(endpoint) {
                push(
                    &mut diagnostics,
                    &source_name,
                    definition.clone(),
                    "endpoints",
                    format!("unknown location {endpoint}"),
                );
            }
        }
        if !seen_edges.insert((from.clone(), to.clone())) {
            push(
                &mut diagnostics,
                &source_name,
                definition,
                "endpoints",
                "duplicate edge",
            );
            continue;
        }
        let Some((from_location, to_location)) = locations.get(&from).zip(locations.get(&to))
        else {
            continue;
        };
        if from == to || !from_location.position.is_finite() || !to_location.position.is_finite() {
            continue;
        }
        if !from_location
            .position
            .distance(to_location.position)
            .is_finite()
        {
            push(
                &mut diagnostics,
                &source_name,
                definition,
                "distance",
                "derived distance must be finite",
            );
            continue;
        }
        edges.insert((from.clone(), to.clone()), TopologyEdge { from, to });
    }

    diagnostics.sort();
    if !diagnostics.is_empty() {
        return Err(ContentErrors(diagnostics));
    }

    Ok(WorldDefinition {
        resources: resources.into_values().collect(),
        locations: locations.into_values().collect(),
        origin: OriginCommunityDefinition {
            id: origin_id.expect("valid origin id after successful validation"),
            location: origin_location.expect("valid origin location after successful validation"),
            population: source.origin.population,
        },
        systems: systems.into_values().collect(),
        deposits: deposits.into_values().collect(),
        sites: sites.into_values().collect(),
        topology: TopologyDefinition {
            edges: edges.into_values().collect(),
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn compile_resource_engine(
    source_name: &str,
    system_definition: &str,
    system_location: Option<&ContentId>,
    source: ResourceEngineSource,
    resources: &BTreeMap<ContentId, ResourceDefinition>,
    deposits: &BTreeMap<ContentId, ResourceDepositDefinition>,
    stocks: &ResourceStore,
    diagnostics: &mut Vec<ContentDiagnostic>,
) -> Option<ResourceEngineDefinition> {
    let ResourceEngineSource {
        collector_energy_profile,
        bodies: body_sources,
        config: config_source,
    } = source;
    let engine_definition = format!("{system_definition}/resource_engine");

    if collector_energy_profile.len() != 10 {
        push(
            diagnostics,
            source_name,
            engine_definition.clone(),
            "collector_energy_profile",
            format!(
                "must contain exactly 10 entries, found {}",
                collector_energy_profile.len()
            ),
        );
    }
    let mut profile = [0_u64; 10];
    for (target, value) in profile.iter_mut().zip(&collector_energy_profile) {
        *target = *value;
    }

    let config = compile_engine_config(
        source_name,
        &engine_definition,
        config_source,
        resources,
        diagnostics,
    );
    let ore_resource = config.as_ref().map(|config| &config.ore_resource);

    let mut bodies = BTreeMap::new();
    let mut development_ids = BTreeSet::new();
    let mut assigned_deposits = BTreeSet::new();
    for (body_index, body_source) in body_sources.into_iter().enumerate() {
        let body_definition =
            nested_definition(&engine_definition, "body", body_index, &body_source.id);
        let body_id = parse_id(
            source_name,
            &body_definition,
            "id",
            &body_source.id,
            diagnostics,
        );
        let mut slots = BTreeMap::new();
        for (slot_index, slot_source) in body_source.slots.into_iter().enumerate() {
            let slot_definition =
                nested_definition(&body_definition, "slot", slot_index, &slot_source.id);
            let slot_id = parse_id(
                source_name,
                &slot_definition,
                "id",
                &slot_source.id,
                diagnostics,
            );
            let development = slot_source.development.and_then(|development| {
                compile_development(
                    source_name,
                    &slot_definition,
                    system_location,
                    development,
                    ore_resource,
                    deposits,
                    &mut development_ids,
                    &mut assigned_deposits,
                    diagnostics,
                )
            });
            let Some(slot_id) = slot_id else { continue };
            if slots.contains_key(&slot_id) {
                push(
                    diagnostics,
                    source_name,
                    slot_definition,
                    "id",
                    format!("duplicate slot id {slot_id}"),
                );
                continue;
            }
            slots.insert(
                slot_id.clone(),
                DevelopmentSlotDefinition {
                    id: slot_id,
                    development,
                },
            );
        }
        let Some(body_id) = body_id else { continue };
        if bodies.contains_key(&body_id) {
            push(
                diagnostics,
                source_name,
                body_definition,
                "id",
                format!("duplicate body id {body_id}"),
            );
            continue;
        }
        bodies.insert(
            body_id.clone(),
            BodyDefinition {
                id: body_id,
                name: body_source.name,
                slots: slots.into_values().collect(),
            },
        );
    }

    if let Some(config) = &config {
        match config
            .battery_energy_capacity
            .checked_mul(functional_battery_count(&bodies))
            .and_then(|added| config.intrinsic_energy_capacity.checked_add(added))
        {
            Some(capacity) => {
                let available = stocks.quantity(&config.energy_resource);
                if available > capacity {
                    push(
                        diagnostics,
                        source_name,
                        engine_definition.clone(),
                        "stocks",
                        format!("available Energy {available} exceeds capacity {capacity}"),
                    );
                }
            }
            None => push(
                diagnostics,
                source_name,
                engine_definition.clone(),
                "energy_capacity",
                "derived Energy capacity overflows",
            ),
        }
    }

    config.map(|config| ResourceEngineDefinition {
        collector_energy_profile: profile,
        bodies: bodies.into_values().collect(),
        config,
    })
}

#[allow(clippy::too_many_arguments)]
fn compile_development(
    source_name: &str,
    slot_definition: &str,
    system_location: Option<&ContentId>,
    source: DevelopmentSource,
    ore_resource: Option<&ContentId>,
    deposits: &BTreeMap<ContentId, ResourceDepositDefinition>,
    development_ids: &mut BTreeSet<ContentId>,
    assigned_deposits: &mut BTreeSet<ContentId>,
    diagnostics: &mut Vec<ContentDiagnostic>,
) -> Option<DevelopmentDefinition> {
    let definition = nested_definition(slot_definition, "development", 0, &source.id);
    let id = parse_id(source_name, &definition, "id", &source.id, diagnostics);
    if let Some(id) = &id
        && !development_ids.insert(id.clone())
    {
        push(
            diagnostics,
            source_name,
            definition.clone(),
            "id",
            format!("duplicate development id {id}"),
        );
    }
    let role = DevelopmentRole::from(source.role);
    let condition = DevelopmentCondition::from(source.condition);
    let extractor_deposit = source.extractor_deposit.as_deref().and_then(|raw| {
        parse_id(
            source_name,
            &definition,
            "extractor_deposit",
            raw,
            diagnostics,
        )
    });

    match (role, extractor_deposit.as_ref()) {
        (DevelopmentRole::Extractor, None) => push(
            diagnostics,
            source_name,
            definition.clone(),
            "extractor_deposit",
            "Extractor requires a deposit assignment",
        ),
        (DevelopmentRole::Extractor, Some(deposit_id)) => {
            if let Some(deposit) = deposits.get(deposit_id) {
                if system_location.is_some_and(|location| &deposit.location != location)
                    || ore_resource.is_some_and(|ore| &deposit.resource != ore)
                {
                    push(
                        diagnostics,
                        source_name,
                        definition.clone(),
                        "extractor_deposit",
                        format!("deposit {deposit_id} is not a compatible same-system Ore deposit"),
                    );
                }
            } else {
                push(
                    diagnostics,
                    source_name,
                    definition.clone(),
                    "extractor_deposit",
                    format!("unknown deposit {deposit_id}"),
                );
            }
            if !assigned_deposits.insert(deposit_id.clone()) {
                push(
                    diagnostics,
                    source_name,
                    definition.clone(),
                    "extractor_deposit",
                    format!("deposit {deposit_id} is assigned more than once"),
                );
            }
        }
        (_, Some(_)) => push(
            diagnostics,
            source_name,
            definition.clone(),
            "extractor_deposit",
            "only an Extractor may have a deposit assignment",
        ),
        (_, None) => {}
    }

    id.map(|id| DevelopmentDefinition {
        id,
        role,
        condition,
        extractor_deposit,
    })
}

fn compile_engine_config(
    source_name: &str,
    engine_definition: &str,
    source: ResourceEngineConfigSource,
    resources: &BTreeMap<ContentId, ResourceDefinition>,
    diagnostics: &mut Vec<ContentDiagnostic>,
) -> Option<ResourceEngineConfig> {
    let definition = format!("{engine_definition}/config");
    let energy_resource = compile_resource_reference(
        source_name,
        &definition,
        "energy_resource",
        &source.energy_resource,
        resources,
        diagnostics,
    );
    let ore_resource = compile_resource_reference(
        source_name,
        &definition,
        "ore_resource",
        &source.ore_resource,
        resources,
        diagnostics,
    );
    let alloy_resource = compile_resource_reference(
        source_name,
        &definition,
        "alloy_resource",
        &source.alloy_resource,
        resources,
        diagnostics,
    );

    if energy_resource
        .as_ref()
        .is_some_and(|resource| resource.as_str() != ENERGY_ID)
    {
        push(
            diagnostics,
            source_name,
            definition.clone(),
            "energy_resource",
            format!("must reference {ENERGY_ID}"),
        );
    }
    if let (Some(energy), Some(ore), Some(alloy)) =
        (&energy_resource, &ore_resource, &alloy_resource)
        && (energy == ore || energy == alloy || ore == alloy)
    {
        push(
            diagnostics,
            source_name,
            definition.clone(),
            "resources",
            "energy, ore, and alloy resources must be distinct",
        );
    }

    for (field, value) in [
        (
            "life_support_per_population",
            source.life_support_per_population,
        ),
        ("origin_construction_work", source.origin_construction_work),
        (
            "intrinsic_energy_capacity",
            source.intrinsic_energy_capacity,
        ),
        ("battery_energy_capacity", source.battery_energy_capacity),
    ] {
        require_nonzero(source_name, &definition, field, value, diagnostics);
    }

    let collector_recipe = compile_recipe(
        source_name,
        &format!("{definition}/collector_recipe"),
        source.collector_recipe,
        resources,
        diagnostics,
    );
    let battery_recipe = compile_recipe(
        source_name,
        &format!("{definition}/battery_recipe"),
        source.battery_recipe,
        resources,
        diagnostics,
    );
    let extractor_recipe = compile_recipe(
        source_name,
        &format!("{definition}/extractor_recipe"),
        source.extractor_recipe,
        resources,
        diagnostics,
    );
    let refinery_recipe = compile_recipe(
        source_name,
        &format!("{definition}/refinery_recipe"),
        source.refinery_recipe,
        resources,
        diagnostics,
    );

    let extractor_definition = format!("{definition}/extractor");
    for (field, value) in [
        ("energy_upkeep", source.extractor.energy_upkeep),
        ("cycle_duration", source.extractor.cycle_duration),
    ] {
        require_nonzero(
            source_name,
            &extractor_definition,
            field,
            value,
            diagnostics,
        );
    }
    if source.extractor.ore_output != 1 {
        push(
            diagnostics,
            source_name,
            &extractor_definition,
            "ore_output",
            "must equal 1",
        );
    }
    let refinery_definition = format!("{definition}/refinery");
    for (field, value) in [
        ("energy_upkeep", source.refinery.energy_upkeep),
        ("cycle_duration", source.refinery.cycle_duration),
        ("ore_input", source.refinery.ore_input),
        ("alloy_output", source.refinery.alloy_output),
    ] {
        require_nonzero(source_name, &refinery_definition, field, value, diagnostics);
    }

    if let (Some(energy), Some(ore), Some(alloy)) =
        (&energy_resource, &ore_resource, &alloy_resource)
    {
        for (name, role, recipe) in [
            (
                "collector_recipe",
                DevelopmentRole::Collector,
                &collector_recipe,
            ),
            ("battery_recipe", DevelopmentRole::Battery, &battery_recipe),
            (
                "extractor_recipe",
                DevelopmentRole::Extractor,
                &extractor_recipe,
            ),
            (
                "refinery_recipe",
                DevelopmentRole::Refinery,
                &refinery_recipe,
            ),
        ] {
            let recipe_definition = format!("{definition}/{name}");
            if recipe.cost.quantity(energy) == 0 {
                push(
                    diagnostics,
                    source_name,
                    recipe_definition.clone(),
                    "costs",
                    "Energy cost must be nonzero",
                );
            }
            match role {
                DevelopmentRole::Collector
                | DevelopmentRole::Battery
                | DevelopmentRole::Extractor => {
                    if recipe.cost.quantity(alloy) == 0 || recipe.cost.quantity(ore) != 0 {
                        push(
                            diagnostics,
                            source_name,
                            recipe_definition,
                            "costs",
                            "must consume Alloy and never Ore",
                        );
                    }
                }
                DevelopmentRole::Refinery => {
                    if recipe.cost.quantity(ore) == 0 || recipe.cost.quantity(alloy) != 0 {
                        push(
                            diagnostics,
                            source_name,
                            recipe_definition,
                            "costs",
                            "must consume Ore and never Alloy",
                        );
                    }
                }
            }
        }
    }

    energy_resource.zip(ore_resource).zip(alloy_resource).map(
        |((energy_resource, ore_resource), alloy_resource)| ResourceEngineConfig {
            energy_resource,
            ore_resource,
            alloy_resource,
            life_support_per_population: source.life_support_per_population,
            origin_construction_work: source.origin_construction_work,
            intrinsic_energy_capacity: source.intrinsic_energy_capacity,
            battery_energy_capacity: source.battery_energy_capacity,
            collector_recipe,
            battery_recipe,
            extractor_recipe,
            refinery_recipe,
            extractor: ExtractorParameters {
                energy_upkeep: source.extractor.energy_upkeep,
                cycle_duration: source.extractor.cycle_duration,
                ore_output: source.extractor.ore_output,
            },
            refinery: RefineryParameters {
                energy_upkeep: source.refinery.energy_upkeep,
                cycle_duration: source.refinery.cycle_duration,
                ore_input: source.refinery.ore_input,
                alloy_output: source.refinery.alloy_output,
            },
        },
    )
}

fn compile_recipe(
    source_name: &str,
    definition: &str,
    source: ConstructionRecipeSource,
    resources: &BTreeMap<ContentId, ResourceDefinition>,
    diagnostics: &mut Vec<ContentDiagnostic>,
) -> ConstructionRecipe {
    require_nonzero(
        source_name,
        definition,
        "required_work",
        source.required_work,
        diagnostics,
    );
    ConstructionRecipe {
        cost: compile_amounts(
            source_name,
            definition,
            "costs",
            source.costs,
            resources,
            true,
            diagnostics,
        ),
        required_work: source.required_work,
    }
}

fn compile_amounts(
    source_name: &str,
    definition: &str,
    field_prefix: &str,
    amounts: Vec<ResourceAmountSource>,
    resources: &BTreeMap<ContentId, ResourceDefinition>,
    quantities_must_be_nonzero: bool,
    diagnostics: &mut Vec<ContentDiagnostic>,
) -> ResourceStore {
    let mut store = BTreeMap::new();
    for (index, amount) in amounts.into_iter().enumerate() {
        let field = format!("{field_prefix}[{index}].resource");
        let Some(resource) = parse_id(
            source_name,
            definition,
            &field,
            &amount.resource,
            diagnostics,
        ) else {
            continue;
        };
        if !resources.contains_key(&resource) {
            push(
                diagnostics,
                source_name,
                definition,
                field.clone(),
                format!("unknown resource {resource}"),
            );
        }
        if quantities_must_be_nonzero {
            require_nonzero(
                source_name,
                definition,
                &format!("{field_prefix}[{index}].quantity"),
                amount.quantity,
                diagnostics,
            );
        }
        if store.insert(resource.clone(), amount.quantity).is_some() {
            push(
                diagnostics,
                source_name,
                definition,
                field,
                format!("duplicate resource {resource}"),
            );
        }
    }
    resource_store(store)
}

fn compile_resource_reference(
    source_name: &str,
    definition: &str,
    field: &str,
    raw: &str,
    resources: &BTreeMap<ContentId, ResourceDefinition>,
    diagnostics: &mut Vec<ContentDiagnostic>,
) -> Option<ContentId> {
    let resource = parse_id(source_name, definition, field, raw, diagnostics)?;
    if !resources.contains_key(&resource) {
        push(
            diagnostics,
            source_name,
            definition,
            field,
            format!("unknown resource {resource}"),
        );
    }
    Some(resource)
}

fn functional_battery_count(bodies: &BTreeMap<ContentId, BodyDefinition>) -> u64 {
    bodies
        .values()
        .flat_map(|body| &body.slots)
        .filter(|slot| {
            slot.development.as_ref().is_some_and(|development| {
                development.role == DevelopmentRole::Battery
                    && development.condition == DevelopmentCondition::Functional
            })
        })
        .count()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn resource_store(stocks: BTreeMap<ContentId, u64>) -> ResourceStore {
    stocks.into_iter().collect()
}

fn parse_id(
    source: &str,
    definition: &str,
    field: &str,
    raw: &str,
    diagnostics: &mut Vec<ContentDiagnostic>,
) -> Option<ContentId> {
    match ContentId::new(raw) {
        Ok(id) => Some(id),
        Err(error) => {
            push(diagnostics, source, definition, field, error.to_string());
            None
        }
    }
}

fn definition_name(kind: &str, index: usize, raw_id: &str) -> String {
    ContentId::new(raw_id)
        .map(|id| format!("{kind}:{id}"))
        .unwrap_or_else(|_| format!("{kind}[{index}]"))
}

fn nested_definition(parent: &str, kind: &str, index: usize, raw_id: &str) -> String {
    ContentId::new(raw_id)
        .map(|id| format!("{parent}/{kind}:{id}"))
        .unwrap_or_else(|_| format!("{parent}/{kind}[{index}]"))
}

fn require_nonzero(
    source: &str,
    definition: &str,
    field: &str,
    value: u64,
    diagnostics: &mut Vec<ContentDiagnostic>,
) {
    if value == 0 {
        push(diagnostics, source, definition, field, "must be nonzero");
    }
}

fn push(
    diagnostics: &mut Vec<ContentDiagnostic>,
    source: &str,
    definition: impl Into<String>,
    field: impl Into<String>,
    message: impl Into<String>,
) {
    diagnostics.push(ContentDiagnostic {
        source: source.to_owned(),
        definition: definition.into(),
        field: field.into(),
        message: message.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{CoreError, DevelopmentRole, WorldState};

    const SUBSTRATE: &str = include_str!("../tests/fixtures/three_locations.ron");
    const STAGE4: &str = include_str!("../tests/fixtures/stage4_origin.ron");
    const INVALID_STAGE4: &str = include_str!("../tests/fixtures/invalid_stage4.ron");

    fn id(value: &str) -> ContentId {
        ContentId::new(value).unwrap()
    }

    fn stocks(snapshot: &game_core::ResourceEngineSnapshot) -> (u64, u64, u64) {
        (
            snapshot.stocks.quantity(&id(ENERGY_ID)),
            snapshot.stocks.quantity(&id("core:ore")),
            snapshot.stocks.quantity(&id("core:alloy")),
        )
    }

    #[test]
    fn compiles_stage3_system_stocks_without_enabling_the_engine() {
        let definition = compile_str("three_locations.ron", SUBSTRATE).expect("valid fixture");
        assert_eq!(definition.locations.len(), 3);
        assert_eq!(definition.systems.len(), 3);
        assert!(
            definition
                .systems
                .iter()
                .all(|system| system.resource_engine.is_none())
        );
        assert_eq!(
            definition
                .systems
                .iter()
                .find(|system| system.location == id("core:origin"))
                .unwrap()
                .stocks
                .quantity(&id(ENERGY_ID)),
            8
        );
        let mut state = WorldState::new(definition).expect("substrate instantiates");
        let before = state.snapshot();
        assert_eq!(
            state.advance_tick(),
            Err(CoreError::MissingResourceEnginePrerequisite)
        );
        assert_eq!(state.snapshot(), before);
    }

    #[test]
    fn compiles_the_exact_authored_stage4_origin() {
        let definition = compile_str("stage4_origin.ron", STAGE4).expect("valid Stage 4 fixture");
        assert_eq!(definition.origin.population, 0);
        assert_eq!(definition.resources.len(), 3);
        assert_eq!(definition.systems.len(), 1);
        assert_eq!(definition.deposits[0].quantity, 200);
        let engine = definition.systems[0].resource_engine.as_ref().unwrap();
        assert_eq!(
            engine.collector_energy_profile,
            [40, 40, 30, 20, 10, 10, 20, 30, 40, 40]
        );
        assert_eq!(engine.bodies.len(), 1);
        assert_eq!(engine.bodies[0].slots.len(), 6);
        let installed = engine.bodies[0]
            .slots
            .iter()
            .filter_map(|slot| slot.development.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].role, DevelopmentRole::Collector);
        assert_eq!(installed[0].condition, DevelopmentCondition::Functional);
        let _state = WorldState::new(definition).expect("compiled definition satisfies core");
    }

    #[test]
    fn game_content_fixture_drives_the_approved_twenty_tick_bootstrap() {
        let definition = compile_str("stage4_origin.ron", STAGE4).expect("valid fixture");
        let mut state = WorldState::new(definition).expect("fixture instantiates");
        let system = id("core:origin");
        let body = id("core:origin_body");
        state
            .enqueue_construction(
                &system,
                &body,
                &id("core:slot_1"),
                DevelopmentRole::Refinery,
                None,
            )
            .unwrap();
        assert_eq!(
            stocks(&state.strategic_snapshot(&system).unwrap()),
            (0, 8, 0)
        );

        let mut checkpoints = BTreeMap::new();
        for tick in 1..=20 {
            let snapshot = state.advance_tick().unwrap();
            if [4, 8, 12, 16, 20].contains(&tick) {
                checkpoints.insert(tick, snapshot);
            }
            if tick == 8 {
                state
                    .enqueue_construction(
                        &system,
                        &body,
                        &id("core:slot_2"),
                        DevelopmentRole::Battery,
                        None,
                    )
                    .unwrap();
                assert_eq!(
                    stocks(&state.strategic_snapshot(&system).unwrap()),
                    (0, 0, 2)
                );
            }
            if tick == 12 {
                state
                    .enqueue_construction(
                        &system,
                        &body,
                        &id("core:slot_3"),
                        DevelopmentRole::Extractor,
                        Some(&id("core:ore_deposit")),
                    )
                    .unwrap();
                assert_eq!(
                    stocks(&state.strategic_snapshot(&system).unwrap()),
                    (40, 0, 0)
                );
            }
        }

        for (tick, expected_stocks, deposit, overflow) in [
            (4, (10, 8, 0), 200, 120),
            (8, (10, 0, 4), 200, 150),
            (12, (50, 0, 2), 200, 260),
            (16, (110, 0, 0), 200, 260),
            (20, (110, 0, 2), 196, 330),
        ] {
            let snapshot = &checkpoints[&tick];
            assert_eq!(stocks(snapshot), expected_stocks);
            assert_eq!(snapshot.deposits[0].quantity, deposit);
            assert_eq!(snapshot.energy_overflow.cumulative, overflow);
        }
    }

    #[test]
    fn zero_seasonal_profile_phases_have_content_and_core_parity() {
        let zero_profile = STAGE4.replace(
            "collector_energy_profile: [40, 40, 30, 20, 10, 10, 20, 30, 40, 40]",
            "collector_energy_profile: [0, 40, 30, 20, 10, 10, 20, 30, 40, 0]",
        );
        let definition = compile_str("zero_profile.ron", &zero_profile).unwrap();
        assert_eq!(
            definition.systems[0]
                .resource_engine
                .as_ref()
                .unwrap()
                .collector_energy_profile,
            [0, 40, 30, 20, 10, 10, 20, 30, 40, 0]
        );
        let snapshot = WorldState::new(definition)
            .unwrap()
            .strategic_snapshot(&id("core:origin"))
            .unwrap();
        assert_eq!(
            snapshot.collector_energy_profile,
            [0, 40, 30, 20, 10, 10, 20, 30, 40, 0]
        );
    }

    #[test]
    fn non_unit_extractor_output_is_rejected_by_content() {
        let non_unit = STAGE4.replace("ore_output: 1", "ore_output: 2");
        let errors = compile_str("non_unit.ron", &non_unit).expect_err("non-unit output fails");
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic.field == "ore_output" && diagnostic.message == "must equal 1"
        }));
    }

    #[test]
    fn stage4_input_permutations_compile_to_the_same_definition() {
        let permuted = STAGE4
            .replace(
                "(id: \"core:energy\", name: \"Energy\"),\n        (id: \"core:ore\", name: \"Ore\"),\n        (id: \"core:alloy\", name: \"Alloy\"),",
                "(id: \"core:alloy\", name: \"Alloy\"),\n        (id: \"core:ore\", name: \"Ore\"),\n        (id: \"core:energy\", name: \"Energy\"),",
            )
            .replace(
                "(id: \"core:slot_0\", development: Some((id: \"core:initial_collector\", role: Collector, condition: Functional))),\n                            (id: \"core:slot_1\"),\n                            (id: \"core:slot_2\"),\n                            (id: \"core:slot_3\"),\n                            (id: \"core:slot_4\"),\n                            (id: \"core:slot_5\"),",
                "(id: \"core:slot_5\"),\n                            (id: \"core:slot_4\"),\n                            (id: \"core:slot_3\"),\n                            (id: \"core:slot_2\"),\n                            (id: \"core:slot_1\"),\n                            (id: \"core:slot_0\", development: Some((id: \"core:initial_collector\", role: Collector, condition: Functional))),",
            )
            .replace(
                "(resource: \"core:energy\", quantity: 10), (resource: \"core:alloy\", quantity: 2)",
                "(resource: \"core:alloy\", quantity: 2), (resource: \"core:energy\", quantity: 10)",
            );
        assert!(
            permuted.find("core:slot_5").unwrap() < permuted.find("core:slot_0").unwrap(),
            "slot order was actually permuted"
        );
        let first = compile_str("first.ron", STAGE4).unwrap();
        let second = compile_str("second.ron", &permuted).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            WorldState::new(first).unwrap().snapshot(),
            WorldState::new(second).unwrap().snapshot()
        );
    }

    #[test]
    fn invalid_stage4_content_reports_complete_sorted_source_aware_diagnostics() {
        let errors = compile_str("invalid_stage4.ron", INVALID_STAGE4).expect_err("invalid");
        let diagnostics = errors.diagnostics();
        assert!(diagnostics.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.source == "invalid_stage4.ron")
        );
        for expected in [
            ("collector_energy_profile", "exactly 10 entries"),
            ("life_support_per_population", "must be nonzero"),
            ("origin_construction_work", "must be nonzero"),
            ("intrinsic_energy_capacity", "must be nonzero"),
            ("battery_energy_capacity", "must be nonzero"),
            ("required_work", "must be nonzero"),
            ("costs[1].resource", "duplicate resource core:energy"),
            ("energy_upkeep", "must be nonzero"),
            ("cycle_duration", "must be nonzero"),
            ("ore_output", "must equal 1"),
            ("ore_input", "must be nonzero"),
            ("alloy_output", "must be nonzero"),
            (
                "extractor_deposit",
                "Extractor requires a deposit assignment",
            ),
        ] {
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.field == expected.0 && diagnostic.message.contains(expected.1)
                }),
                "missing diagnostic {expected:?}: {diagnostics:#?}"
            );
        }
        assert!(
            diagnostics.len() >= 20,
            "expected aggregation, got {diagnostics:#?}"
        );
    }

    #[test]
    fn unknown_fields_are_rejected_at_top_level_and_deeply_nested() {
        let top_level = STAGE4.replacen("resources:", "depostis: [], resources:", 1);
        let nested = STAGE4.replacen("cycle_duration: 1,", "cycle_duration: 1, cadence: 1,", 1);
        for (name, text, unknown) in [
            ("top.ron", top_level, "depostis"),
            ("nested.ron", nested, "cadence"),
        ] {
            let error = compile_str(name, &text).expect_err("unknown fields fail");
            assert_eq!(error.diagnostics()[0].definition, "document");
            assert_eq!(error.diagnostics()[0].field, "parse");
            assert!(error.diagnostics()[0].message.contains(unknown));
        }
    }

    #[test]
    fn parse_errors_include_document_provenance() {
        let errors = compile_str("broken.ron", "not valid world RON").expect_err("invalid RON");
        assert_eq!(errors.diagnostics()[0].source, "broken.ron");
        assert_eq!(errors.diagnostics()[0].definition, "document");
        assert_eq!(errors.diagnostics()[0].field, "parse");
    }
}
