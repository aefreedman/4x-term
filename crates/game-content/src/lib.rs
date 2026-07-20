//! Source-aware RON loading and validation for the origin-and-frontier substrate.

use game_core::{
    ContentId, LocationDefinition, OriginCommunityDefinition, Position3, ReclaimableSiteDefinition,
    ResourceDefinition, ResourceDepositDefinition, ResourceStore, TopologyDefinition, TopologyEdge,
    WorldDefinition,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OriginSource {
    id: String,
    location: String,
    population: u64,
    #[serde(default)]
    stocks: Vec<ResourceAmountSource>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceAmountSource {
    resource: String,
    quantity: u64,
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
        let Some(id) = id else {
            continue;
        };
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
    if source.origin.population == 0 {
        push(
            &mut diagnostics,
            &source_name,
            origin_definition.clone(),
            "population",
            "must be nonzero",
        );
    }
    let mut stocks = BTreeMap::new();
    for (index, amount) in source.origin.stocks.into_iter().enumerate() {
        let field = format!("stocks[{index}].resource");
        let Some(resource) = parse_id(
            &source_name,
            &origin_definition,
            &field,
            &amount.resource,
            &mut diagnostics,
        ) else {
            continue;
        };
        if !resources.contains_key(&resource) {
            push(
                &mut diagnostics,
                &source_name,
                origin_definition.clone(),
                field,
                format!("unknown resource {resource}"),
            );
        }
        if stocks.insert(resource.clone(), amount.quantity).is_some() {
            push(
                &mut diagnostics,
                &source_name,
                origin_definition.clone(),
                format!("stocks[{index}].resource"),
                format!("duplicate resource {resource}"),
            );
        }
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
        if item.quantity == 0 {
            push(
                &mut diagnostics,
                &source_name,
                definition.clone(),
                "quantity",
                "must be nonzero",
            );
        }
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
        let distance = from_location.position.distance(to_location.position);
        if !distance.is_finite() {
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

    // All references and numeric constraints have been checked before these
    // constructors receive the normalized definition.
    let origin = OriginCommunityDefinition {
        id: origin_id.expect("valid origin id after successful validation"),
        location: origin_location.expect("valid origin location after successful validation"),
        population: source.origin.population,
        stocks: resource_store(stocks),
    };
    Ok(WorldDefinition {
        resources: resources.into_values().collect(),
        locations: locations.into_values().collect(),
        origin,
        deposits: deposits.into_values().collect(),
        sites: sites.into_values().collect(),
        topology: TopologyDefinition {
            edges: edges.into_values().collect(),
        },
    })
}

fn resource_store(stocks: BTreeMap<ContentId, u64>) -> ResourceStore {
    let mut store = ResourceStore::new();
    for (resource, quantity) in stocks {
        store.set(resource, quantity);
    }
    store
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
    use game_core::WorldState;

    const VALID: &str = include_str!("../tests/fixtures/three_locations.ron");
    const INVALID: &str = include_str!("../tests/fixtures/invalid_world.ron");

    #[test]
    fn compiles_a_dead_isolated_location_and_instantiates_world_state() {
        let definition = compile_str("three_locations.ron", VALID).expect("valid Tier 1 fixture");
        assert_eq!(definition.locations.len(), 3);
        assert_eq!(definition.deposits.len(), 1);
        assert_eq!(definition.sites.len(), 1);
        assert_eq!(definition.topology.edges.len(), 1);
        let _state = WorldState::new(definition).expect("validated definition instantiates");
    }

    #[test]
    fn normalizes_permuted_input() {
        let first = compile_str("first.ron", VALID).expect("valid fixture");
        let permuted = r#"(
            resources: [(id: "core:water", name: "Water"), (id: "core:energy", name: "Energy")],
            locations: [
                (id: "frontier:isolated", name: "Isolated", position: (x: 12.0, y: 0.0, z: 0.0)),
                (id: "frontier:site", name: "Site", position: (x: 3.0, y: 4.0, z: 0.0)),
                (id: "core:origin", name: "Origin", position: (x: 0.0, y: 0.0, z: 0.0)),
            ],
            origin: (id: "core:community", location: "core:origin", population: 12, stocks: [(resource: "core:energy", quantity: 8)]),
            deposits: [(id: "frontier:water", location: "frontier:site", resource: "core:water", quantity: 5)],
            sites: [(id: "frontier:ruin", location: "frontier:site")],
            topology: (edges: [(from: "frontier:site", to: "core:origin")]),
        )"#;
        let second = compile_str("second.ron", permuted).expect("permuted valid fixture");
        assert_eq!(first, second);
    }

    #[test]
    fn parse_errors_include_document_provenance() {
        let errors = compile_str("broken.ron", "not valid world RON").expect_err("invalid RON");
        assert_eq!(errors.diagnostics()[0].source, "broken.ron");
        assert_eq!(errors.diagnostics()[0].definition, "document");
        assert_eq!(errors.diagnostics()[0].field, "parse");
    }

    #[test]
    fn unknown_fields_are_rejected_in_top_level_and_nested_sources() {
        let top_level = VALID.replacen("resources:", "depostis: [], resources:", 1);
        let nested = VALID.replacen("population: 12,", "population: 12, stock: [],", 1);

        for (source, text, unknown) in [
            ("top_level.ron", top_level, "depostis"),
            ("nested.ron", nested, "stock"),
        ] {
            let errors = compile_str(source, &text).expect_err("unknown field must be rejected");
            assert_eq!(errors.diagnostics()[0].source, source);
            assert_eq!(errors.diagnostics()[0].definition, "document");
            assert_eq!(errors.diagnostics()[0].field, "parse");
            assert!(errors.diagnostics()[0].message.contains(unknown));
        }
    }

    #[test]
    fn location_diagnostics_are_complete_and_permutation_independent() {
        fn invalid_locations(first_is_nonfinite: bool) -> String {
            let finite =
                r#"(id: "core:origin", name: "Finite", position: (x: 0.0, y: 0.0, z: 0.0))"#;
            let nonfinite =
                r#"(id: "core:origin", name: "Invalid", position: (x: NaN, y: 0.0, z: 0.0))"#;
            let locations = if first_is_nonfinite {
                format!("{nonfinite}, {finite}")
            } else {
                format!("{finite}, {nonfinite}")
            };
            format!(
                r#"(
                    resources: [],
                    locations: [{locations}],
                    origin: (id: "core:community", location: "core:origin", population: 1),
                )"#
            )
        }

        let diagnostics = |source: &str| {
            compile_str("locations.ron", source)
                .expect_err("duplicate non-finite locations must fail")
                .diagnostics()
                .iter()
                .map(|diagnostic| {
                    (
                        diagnostic.definition.clone(),
                        diagnostic.field.clone(),
                        diagnostic.message.clone(),
                    )
                })
                .collect::<Vec<_>>()
        };

        let first = diagnostics(&invalid_locations(true));
        let second = diagnostics(&invalid_locations(false));
        assert_eq!(first, second);
        assert_eq!(
            first,
            vec![
                (
                    "locations:core:origin".into(),
                    "id".into(),
                    "duplicate id core:origin".into(),
                ),
                (
                    "locations:core:origin".into(),
                    "position".into(),
                    "coordinates must be finite".into(),
                ),
            ]
        );

        let malformed = invalid_locations(true).replace("core:origin", "BAD ID");
        let malformed_diagnostics = diagnostics(&malformed);
        assert!(
            malformed_diagnostics
                .iter()
                .any(|(_, field, message)| field == "id" && message.contains("invalid content id"))
        );
        assert!(
            malformed_diagnostics
                .iter()
                .any(|(_, field, message)| field == "position"
                    && message == "coordinates must be finite")
        );
    }

    #[test]
    fn aggregates_exact_source_aware_diagnostics() {
        let errors = compile_str("invalid_world.ron", INVALID).expect_err("fixture is invalid");
        let actual = errors
            .diagnostics()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![
                "invalid_world.ron:deposits:frontier:ore:location: unknown location frontier:missing",
                "invalid_world.ron:deposits:frontier:ore:quantity: must be nonzero",
                "invalid_world.ron:deposits:frontier:ore:resource: unknown resource frontier:missing",
                "invalid_world.ron:locations:core:origin:position: coordinates must be finite",
                "invalid_world.ron:origin:core:community:location: unknown location frontier:missing",
                "invalid_world.ron:origin:core:community:population: must be nonzero",
                "invalid_world.ron:origin:core:community:stocks[0].resource: unknown resource frontier:missing",
                "invalid_world.ron:resources:core:water:id: duplicate id core:water",
                "invalid_world.ron:resources[0]:id: invalid content id: BAD ID",
                "invalid_world.ron:sites:frontier:ruin:location: unknown location frontier:missing",
                "invalid_world.ron:topology:core:origin/core:origin:endpoints: self edge is not allowed",
                "invalid_world.ron:topology:core:origin/core:outpost:endpoints: duplicate edge",
                "invalid_world.ron:topology:core:origin/frontier:missing:endpoints: unknown location frontier:missing",
            ]
        );
    }
}
