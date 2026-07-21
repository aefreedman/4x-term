use game_content::{
    GenerationRequest, GeneratorVersion, compile_generation_profile_str, generate_world,
};
use game_core::{ContentId, DevelopmentCondition, DevelopmentRole, Position3, WorldState};
use std::collections::BTreeSet;

const STARTER: &str = include_str!("../../../content/profiles/starter.ron");

fn id(value: &str) -> ContentId {
    ContentId::new(value).expect("test ID")
}

fn request(seed: u64) -> GenerationRequest {
    GenerationRequest {
        version: GeneratorVersion::frontier_revision_1(),
        seed,
        configuration: compile_generation_profile_str("core:starter", STARTER)
            .expect("shipped starter profile compiles"),
    }
}

#[test]
fn strict_profile_errors_retain_logical_provenance() {
    let unknown = STARTER.replacen(
        "target_system_count: 128,",
        "target_system_count: 128, unexpected_field: true,",
        1,
    );
    let error = compile_generation_profile_str("core:strict_test", &unknown)
        .expect_err("unknown field is rejected");
    assert_eq!(error.diagnostics()[0].source, "core:strict_test");
    assert_eq!(error.diagnostics()[0].definition, "document");
    assert_eq!(error.diagnostics()[0].field, "parse");

    let missing = STARTER.replacen("target_system_count: 128,", "", 1);
    let error = compile_generation_profile_str("core:missing_test", &missing)
        .expect_err("missing field is rejected");
    assert_eq!(error.diagnostics()[0].source, "core:missing_test");
    assert!(
        error.diagnostics()[0]
            .message
            .contains("target_system_count")
    );
}

#[test]
fn semantic_input_permutations_have_identical_canonical_bytes_and_fingerprint() {
    let permuted = STARTER.replacen(
        concat!(
            "        (id: \"core:energy\", name: \"Energy\", naturally_deposit_bearing: false),\n",
            "        (id: \"core:ore\", name: \"Ore\", naturally_deposit_bearing: true),\n",
            "        (id: \"core:alloy\", name: \"Alloy\", naturally_deposit_bearing: false),"
        ),
        concat!(
            "        (id: \"core:alloy\", name: \"Alloy\", naturally_deposit_bearing: false),\n",
            "        (id: \"core:energy\", name: \"Energy\", naturally_deposit_bearing: false),\n",
            "        (id: \"core:ore\", name: \"Ore\", naturally_deposit_bearing: true),"
        ),
        1,
    );
    let permuted = permuted
        .replacen(
            "energy_per_quantum: (numerator: 1, denominator: 200)",
            "energy_per_quantum: (numerator: 2, denominator: 400)",
            1,
        )
        .replacen(
            "persistence: (numerator: 1, denominator: 2)",
            "persistence: (numerator: 2, denominator: 4)",
            1,
        );
    assert_ne!(permuted, STARTER);
    let first = compile_generation_profile_str("core:first", STARTER).expect("first compiles");
    let second =
        compile_generation_profile_str("core:second", &permuted).expect("permutation compiles");
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_ne!(first.provenance(), second.provenance());
}

#[test]
fn one_output_affecting_profile_change_changes_fingerprint() {
    let changed = STARTER.replacen("target_system_count: 128", "target_system_count: 127", 1);
    let first = compile_generation_profile_str("core:starter", STARTER).expect("starter compiles");
    let second = compile_generation_profile_str("core:starter", &changed).expect("change compiles");
    assert_ne!(first.fingerprint(), second.fingerprint());
}

#[test]
fn equal_identity_reproduces_equal_normalized_world() {
    let first = generate_world(&request(0x1234_5678)).expect("generation succeeds");
    let second = generate_world(&request(0x1234_5678)).expect("generation succeeds");
    assert_eq!(first.identity, second.identity);
    assert_eq!(first.definition, second.definition);
    assert_eq!(
        WorldState::new(first.definition)
            .expect("artifact instantiates")
            .debug_snapshot(),
        WorldState::new(second.definition)
            .expect("artifact instantiates")
            .debug_snapshot()
    );
}

#[test]
fn generated_world_has_exact_constructive_origin_and_bounded_frontier_facts() {
    let artifact = generate_world(&request(77)).expect("generation succeeds");
    let definition = &artifact.definition;
    assert_eq!(
        artifact.identity.version,
        GeneratorVersion::frontier_revision_1()
    );
    assert_eq!(definition.origin_system, id("core:origin"));
    assert_eq!(definition.origin_community, id("core:origin_community"));
    assert!(definition.population_tokens.is_empty());

    let origin_location = definition
        .locations
        .iter()
        .find(|location| location.id == definition.origin_system)
        .expect("origin location");
    assert_eq!(origin_location.position, Position3::from_quanta(0, 0, 0));
    let origin = definition
        .systems
        .iter()
        .find(|system| system.location == definition.origin_system)
        .expect("origin system");
    assert_eq!(origin.stellar_strength_hundredths, 100);
    assert!((4..=12).contains(&origin.bodies.len()));
    assert_eq!(origin.stocks.quantity(&id("core:energy")), 10);
    assert_eq!(origin.stocks.quantity(&id("core:ore")), 10);
    assert_eq!(origin.stocks.quantity(&id("core:alloy")), 0);
    assert!(origin.player_founded);
    assert!(origin.bodies.iter().all(|body| {
        body.eccentricity_hundredths == 100 && (3..=8).contains(&body.slots.len())
    }));
    assert!(
        origin
            .bodies
            .iter()
            .any(|body| body.initial_resources.quantity(&id("core:ore")) > 0)
    );
    let developments = origin
        .bodies
        .iter()
        .flat_map(|body| &body.slots)
        .filter_map(|slot| slot.development.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(developments.len(), 1);
    assert_eq!(developments[0].role, DevelopmentRole::Collector);
    assert_eq!(developments[0].condition, DevelopmentCondition::Functional);
    assert_eq!(
        origin.bodies[0].slots[0].development.as_ref(),
        Some(developments[0])
    );

    let positions = definition
        .locations
        .iter()
        .map(|location| {
            (
                location.position.x.0,
                location.position.y.0,
                location.position.z.0,
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(positions.len(), definition.locations.len());
    for system in definition
        .systems
        .iter()
        .filter(|system| system.location != definition.origin_system)
    {
        assert!((10..=300).contains(&system.stellar_strength_hundredths));
        assert!((1..=12).contains(&system.bodies.len()));
        for body in &system.bodies {
            assert!((0..=150).contains(&body.eccentricity_hundredths));
            assert!((1..=8).contains(&body.slots.len()));
            assert!(body.slots.iter().all(|slot| slot.development.is_none()));
        }
    }
}

#[test]
fn invalid_configuration_returns_no_artifact() {
    let invalid = STARTER.replacen("generated_z: 0", "generated_z: 1", 1);
    assert!(compile_generation_profile_str("core:invalid", &invalid).is_err());
}
