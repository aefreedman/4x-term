use game_content::{
    GenerationRequest, GeneratorVersion, compile_generation_profile_str, generate_world,
};
use game_core::WorldState;

const STARTER: &str = include_str!("../../../content/profiles/starter.ron");

#[test]
fn default_feature_external_api_reads_only_validated_artifacts_through_accessors() {
    let request = GenerationRequest {
        version: GeneratorVersion::frontier_revision_1(),
        seed: 17,
        configuration: compile_generation_profile_str("core:starter", STARTER)
            .expect("starter profile compiles"),
    };

    let artifact = generate_world(&request).expect("validated generation succeeds");
    assert_eq!(artifact.identity().seed, 17);
    assert_eq!(artifact.provenance().source_identity, "core:starter");
    WorldState::new(artifact.definition().clone()).expect("read-only definition is valid");
}
