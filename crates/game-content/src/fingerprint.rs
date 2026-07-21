use crate::{
    GeneratorTuning, NormalizedProfile, ResourceGenerationTuning, SystemGenerationTuning,
    UnsignedRatio, UnsignedTriangle,
};
use game_core::{ConstructionRecipe, FixedRate, ResourceStore, ShipTravelTuning, WorldTuning};
use sha2::{Digest, Sha256};
use thiserror::Error;

const PREFIX: &[u8; 4] = b"4XFG";
const ENCODING_REVISION: u16 = 1;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CanonicalEncodingError {
    #[error("canonical value has more than u32::MAX elements or bytes")]
    LengthOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Value {
    Unsigned(u64),
    Signed(i64),
    Boolean(bool),
    String(String),
    Sequence(Vec<Value>),
    Map(Vec<(Value, Value)>),
}

/// SHA-256 for canonical normalized profile bytes and source-content provenance.
#[must_use]
pub fn sha256_fingerprint(canonical_bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(canonical_bytes).into()
}

/// Revision-1 canonical encoding of every normalized output-affecting profile value.
pub fn canonical_profile_bytes(
    profile: &NormalizedProfile,
) -> Result<Vec<u8>, CanonicalEncodingError> {
    let value = map([
        (
            "resources",
            Value::Sequence(
                profile
                    .resources()
                    .iter()
                    .map(|resource| {
                        map([
                            ("id", string(resource.id.as_str())),
                            ("name", string(&resource.name)),
                            (
                                "naturally_deposit_bearing",
                                Value::Boolean(resource.naturally_deposit_bearing),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("gameplay", gameplay(profile.gameplay())),
        ("generator", generator(profile.generator())),
    ]);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(PREFIX);
    bytes.extend_from_slice(&ENCODING_REVISION.to_le_bytes());
    encode(&value, &mut bytes)?;
    Ok(bytes)
}

fn gameplay(value: &WorldTuning) -> Value {
    map([
        ("energy_resource", string(value.energy_resource.as_str())),
        ("ore_resource", string(value.ore_resource.as_str())),
        ("alloy_resource", string(value.alloy_resource.as_str())),
        (
            "seasonal_shape",
            Value::Sequence(
                value
                    .seasonal_shape
                    .iter()
                    .copied()
                    .map(Value::Unsigned)
                    .collect(),
            ),
        ),
        (
            "seasonal_baseline_average",
            Value::Unsigned(value.seasonal_baseline_average),
        ),
        (
            "life_support_per_population",
            Value::Unsigned(value.life_support_per_population),
        ),
        (
            "origin_construction_work",
            Value::Unsigned(value.origin_construction_work),
        ),
        (
            "intrinsic_energy_capacity",
            Value::Unsigned(value.intrinsic_energy_capacity),
        ),
        (
            "battery_energy_capacity",
            Value::Unsigned(value.battery_energy_capacity),
        ),
        (
            "habitat_population_energy",
            Value::Unsigned(value.habitat_population_energy),
        ),
        (
            "coordinate_quanta_per_map_unit",
            Value::Unsigned(value.coordinate_quanta_per_map_unit),
        ),
        ("collector_recipe", recipe(&value.collector_recipe)),
        ("battery_recipe", recipe(&value.battery_recipe)),
        ("extractor_recipe", recipe(&value.extractor_recipe)),
        ("refinery_recipe", recipe(&value.refinery_recipe)),
        ("habitat_recipe", recipe(&value.habitat_recipe)),
        ("shipyard_recipe", recipe(&value.shipyard_recipe)),
        (
            "extractor",
            map([
                (
                    "energy_upkeep",
                    Value::Unsigned(value.extractor.energy_upkeep),
                ),
                (
                    "cycle_duration",
                    Value::Unsigned(value.extractor.cycle_duration),
                ),
                ("output", Value::Unsigned(value.extractor.output)),
            ]),
        ),
        (
            "refinery",
            map([
                (
                    "energy_upkeep",
                    Value::Unsigned(value.refinery.energy_upkeep),
                ),
                (
                    "cycle_duration",
                    Value::Unsigned(value.refinery.cycle_duration),
                ),
                ("input", Value::Unsigned(value.refinery.input)),
                ("output", Value::Unsigned(value.refinery.output)),
            ]),
        ),
        (
            "probe_project",
            map([
                (
                    "material_commitment",
                    resource_store(&value.probe_project.material_commitment),
                ),
                (
                    "duration_ticks",
                    Value::Unsigned(value.probe_project.duration_ticks),
                ),
                (
                    "energy_per_progress_tick",
                    Value::Unsigned(value.probe_project.energy_per_progress_tick),
                ),
            ]),
        ),
        (
            "expedition_project",
            map([
                (
                    "hull_material_commitment",
                    resource_store(&value.expedition_project.hull_material_commitment),
                ),
                (
                    "founding_stocks",
                    resource_store(&value.expedition_project.founding_stocks),
                ),
                (
                    "duration_ticks",
                    Value::Unsigned(value.expedition_project.duration_ticks),
                ),
                (
                    "energy_per_progress_tick",
                    Value::Unsigned(value.expedition_project.energy_per_progress_tick),
                ),
            ]),
        ),
        ("probe_travel", travel(value.probe_travel)),
        ("expedition_travel", travel(value.expedition_travel)),
        (
            "probe_reveal_radius_quanta",
            Value::Unsigned(value.probe_reveal_radius_quanta),
        ),
        (
            "communication_delay_per_quantum",
            rate(value.communication_delay_per_quantum),
        ),
        (
            "resource_richness",
            Value::Map(
                value
                    .resource_richness
                    .iter()
                    .map(|(resource, thresholds)| {
                        (
                            string(resource.as_str()),
                            map([
                                ("poor_minimum", Value::Unsigned(thresholds.poor_minimum)),
                                ("poor_maximum", Value::Unsigned(thresholds.poor_maximum)),
                                ("normal_minimum", Value::Unsigned(thresholds.normal_minimum)),
                                ("normal_maximum", Value::Unsigned(thresholds.normal_maximum)),
                                ("rich_minimum", Value::Unsigned(thresholds.rich_minimum)),
                            ]),
                        )
                    })
                    .collect(),
            ),
        ),
    ])
}

fn generator(value: &GeneratorTuning) -> Value {
    map([
        (
            "coordinate_quanta_per_map_unit",
            Value::Unsigned(value.coordinate_quanta_per_map_unit),
        ),
        (
            "target_system_count",
            Value::Unsigned(value.target_system_count),
        ),
        (
            "x_bounds",
            map([
                ("minimum", Value::Signed(value.x_bounds.minimum)),
                (
                    "maximum_exclusive",
                    Value::Signed(value.x_bounds.maximum_exclusive),
                ),
            ]),
        ),
        (
            "y_bounds",
            map([
                ("minimum", Value::Signed(value.y_bounds.minimum)),
                (
                    "maximum_exclusive",
                    Value::Signed(value.y_bounds.maximum_exclusive),
                ),
            ]),
        ),
        ("generated_z", Value::Signed(value.generated_z)),
        (
            "cell_width_quanta",
            Value::Unsigned(value.cell_width_quanta),
        ),
        (
            "cell_height_quanta",
            Value::Unsigned(value.cell_height_quanta),
        ),
        (
            "noise_octaves",
            Value::Unsigned(u64::from(value.noise_octaves)),
        ),
        (
            "base_wavelength_quanta",
            Value::Unsigned(value.base_wavelength_quanta),
        ),
        ("lacunarity", Value::Unsigned(value.lacunarity)),
        ("persistence", ratio(value.persistence)),
        ("full_cell_jitter", Value::Boolean(value.full_cell_jitter)),
        ("origin_system", system(value.origin_system)),
        ("frontier_system", system(value.frontier_system)),
        (
            "resources",
            Value::Map(
                value
                    .resources
                    .iter()
                    .map(|(id, tuning)| (string(id.as_str()), resource_generation(*tuning)))
                    .collect(),
            ),
        ),
    ])
}

fn system(value: SystemGenerationTuning) -> Value {
    map([
        ("strength_hundredths", triangle(value.strength_hundredths)),
        ("body_count", triangle(value.body_count)),
        (
            "eccentricity_hundredths",
            triangle(value.eccentricity_hundredths),
        ),
        ("slots_per_body", triangle(value.slots_per_body)),
    ])
}

fn resource_generation(value: ResourceGenerationTuning) -> Value {
    map([
        (
            "origin",
            map([
                (
                    "resource_bearing_body_count",
                    triangle(value.origin.resource_bearing_body_count),
                ),
                (
                    "quantity_per_body",
                    triangle(value.origin.quantity_per_body),
                ),
            ]),
        ),
        (
            "frontier",
            map([
                (
                    "presence_basis_points",
                    Value::Unsigned(u64::from(value.frontier.presence_basis_points)),
                ),
                (
                    "resource_bearing_body_count",
                    triangle(value.frontier.resource_bearing_body_count),
                ),
                (
                    "quantity_per_body",
                    triangle(value.frontier.quantity_per_body),
                ),
            ]),
        ),
    ])
}

fn triangle(value: UnsignedTriangle) -> Value {
    map([
        ("minimum", Value::Unsigned(value.minimum)),
        ("mode", Value::Unsigned(value.mode)),
        ("maximum", Value::Unsigned(value.maximum)),
    ])
}

fn ratio(value: UnsignedRatio) -> Value {
    map([
        ("numerator", Value::Unsigned(value.numerator)),
        ("denominator", Value::Unsigned(value.denominator)),
    ])
}

fn rate(value: FixedRate) -> Value {
    map([
        ("numerator", Value::Unsigned(value.numerator)),
        ("denominator", Value::Unsigned(value.denominator.get())),
    ])
}

fn travel(value: ShipTravelTuning) -> Value {
    map([
        (
            "maximum_jump_quanta",
            Value::Unsigned(value.maximum_jump_quanta),
        ),
        (
            "speed_quanta_per_tick",
            Value::Unsigned(value.speed_quanta_per_tick),
        ),
        ("energy_per_quantum", rate(value.energy_per_quantum)),
    ])
}

fn recipe(value: &ConstructionRecipe) -> Value {
    map([
        ("cost", resource_store(&value.cost)),
        ("required_work", Value::Unsigned(value.required_work)),
    ])
}

fn resource_store(value: &ResourceStore) -> Value {
    Value::Map(
        value
            .quantities
            .iter()
            .map(|(resource, quantity)| (string(resource.as_str()), Value::Unsigned(*quantity)))
            .collect(),
    )
}

fn string(value: &str) -> Value {
    Value::String(value.to_owned())
}

fn map<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (string(key), value))
            .collect(),
    )
}

fn encode(value: &Value, output: &mut Vec<u8>) -> Result<(), CanonicalEncodingError> {
    match value {
        Value::Unsigned(value) => {
            output.push(0x01);
            output.extend_from_slice(&value.to_le_bytes());
        }
        Value::Signed(value) => {
            output.push(0x02);
            output.extend_from_slice(&value.to_le_bytes());
        }
        Value::Boolean(value) => {
            output.push(0x03);
            output.push(u8::from(*value));
        }
        Value::String(value) => {
            output.push(0x04);
            let length =
                u32::try_from(value.len()).map_err(|_| CanonicalEncodingError::LengthOverflow)?;
            output.extend_from_slice(&length.to_le_bytes());
            output.extend_from_slice(value.as_bytes());
        }
        Value::Sequence(values) => {
            output.push(0x05);
            let length =
                u32::try_from(values.len()).map_err(|_| CanonicalEncodingError::LengthOverflow)?;
            output.extend_from_slice(&length.to_le_bytes());
            for value in values {
                encode(value, output)?;
            }
        }
        Value::Map(entries) => {
            output.push(0x06);
            let length =
                u32::try_from(entries.len()).map_err(|_| CanonicalEncodingError::LengthOverflow)?;
            output.extend_from_slice(&length.to_le_bytes());
            let mut encoded = entries
                .iter()
                .map(|(key, value)| {
                    let mut key_bytes = Vec::new();
                    encode(key, &mut key_bytes)?;
                    Ok((key_bytes, value))
                })
                .collect::<Result<Vec<_>, CanonicalEncodingError>>()?;
            encoded.sort_by(|left, right| left.0.cmp(&right.0));
            for (key, value) in encoded {
                output.extend_from_slice(&key);
                encode(value, output)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_and_container_encoding_vectors_are_fixed() {
        let value = Value::Sequence(vec![
            Value::Unsigned(1),
            Value::Signed(-2),
            Value::Boolean(true),
            string("A"),
            Value::Map(vec![
                (string("b"), Value::Unsigned(2)),
                (string("a"), Value::Unsigned(1)),
            ]),
        ]);
        let mut bytes = Vec::new();
        encode(&value, &mut bytes).expect("small vector encodes");
        assert_eq!(
            bytes,
            vec![
                0x05, 5, 0, 0, 0, 0x01, 1, 0, 0, 0, 0, 0, 0, 0, 0x02, 0xfe, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff, 0xff, 0x03, 1, 0x04, 1, 0, 0, 0, b'A', 0x06, 2, 0, 0, 0, 0x04, 1, 0, 0,
                0, b'a', 0x01, 1, 0, 0, 0, 0, 0, 0, 0, 0x04, 1, 0, 0, 0, b'b', 0x01, 2, 0, 0, 0, 0,
                0, 0, 0,
            ]
        );
    }
}
