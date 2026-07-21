use game_core::{ResourceDefinition, WorldTuning, validate_world_tuning};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsignedTriangle {
    pub minimum: u64,
    pub mode: u64,
    pub maximum: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedBounds {
    pub minimum: i64,
    pub maximum_exclusive: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsignedRatio {
    pub numerator: u64,
    pub denominator: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemGenerationTuning {
    pub strength_hundredths: UnsignedTriangle,
    pub body_count: UnsignedTriangle,
    pub eccentricity_hundredths: UnsignedTriangle,
    pub slots_per_body: UnsignedTriangle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OriginResourceGenerationTuning {
    pub resource_bearing_body_count: UnsignedTriangle,
    pub quantity_per_body: UnsignedTriangle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrontierResourceGenerationTuning {
    pub presence_basis_points: u16,
    pub resource_bearing_body_count: UnsignedTriangle,
    pub quantity_per_body: UnsignedTriangle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceGenerationTuning {
    pub origin: OriginResourceGenerationTuning,
    pub frontier: FrontierResourceGenerationTuning,
}

/// Complete normalized revisioned generator configuration. No source defaults remain here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratorTuning {
    pub coordinate_quanta_per_map_unit: u64,
    pub target_system_count: u64,
    pub x_bounds: SignedBounds,
    pub y_bounds: SignedBounds,
    pub generated_z: i64,
    pub cell_width_quanta: u64,
    pub cell_height_quanta: u64,
    pub noise_octaves: u32,
    pub base_wavelength_quanta: u64,
    pub lacunarity: u64,
    pub persistence: UnsignedRatio,
    pub full_cell_jitter: bool,
    pub origin_system: SystemGenerationTuning,
    pub frontier_system: SystemGenerationTuning,
    pub resources: BTreeMap<game_core::ContentId, ResourceGenerationTuning>,
}

/// Validated, format-independent complete gameplay and generator configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedProfile {
    resources: Vec<ResourceDefinition>,
    gameplay: WorldTuning,
    generator: GeneratorTuning,
}

impl NormalizedProfile {
    pub fn new(
        mut resources: Vec<ResourceDefinition>,
        gameplay: WorldTuning,
        generator: GeneratorTuning,
    ) -> Result<Self, ProfileValidationError> {
        resources.sort_by(|left, right| left.id.cmp(&right.id));
        if resources.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(ProfileValidationError::DuplicateResource);
        }
        validate_world_tuning(&gameplay, &resources)
            .map_err(|error| ProfileValidationError::InvalidGameplay(error.to_string()))?;
        validate_triangle(generator.origin_system.strength_hundredths)?;
        validate_triangle(generator.origin_system.body_count)?;
        validate_triangle(generator.origin_system.eccentricity_hundredths)?;
        validate_triangle(generator.origin_system.slots_per_body)?;
        validate_triangle(generator.frontier_system.strength_hundredths)?;
        validate_triangle(generator.frontier_system.body_count)?;
        validate_triangle(generator.frontier_system.eccentricity_hundredths)?;
        validate_triangle(generator.frontier_system.slots_per_body)?;
        let width = u64::try_from(
            i128::from(generator.x_bounds.maximum_exclusive)
                - i128::from(generator.x_bounds.minimum),
        )
        .map_err(|_| ProfileValidationError::InvalidGeneratorBounds)?;
        let height = u64::try_from(
            i128::from(generator.y_bounds.maximum_exclusive)
                - i128::from(generator.y_bounds.minimum),
        )
        .map_err(|_| ProfileValidationError::InvalidGeneratorBounds)?;
        let mut wavelengths_are_exact = generator.lacunarity != 0;
        let mut amplitudes_are_positive = generator.persistence.denominator != 0;
        let mut wavelength_divisor = 1_u64;
        let mut amplitude = 1_u64 << 32;
        if wavelengths_are_exact {
            for octave in 0..generator.noise_octaves {
                if !generator
                    .base_wavelength_quanta
                    .is_multiple_of(wavelength_divisor)
                    || generator.base_wavelength_quanta / wavelength_divisor == 0
                {
                    wavelengths_are_exact = false;
                    break;
                }
                if amplitude == 0 {
                    amplitudes_are_positive = false;
                    break;
                }
                if generator.persistence.denominator != 0 {
                    amplitude = u64::try_from(
                        u128::from(amplitude) * u128::from(generator.persistence.numerator)
                            / u128::from(generator.persistence.denominator),
                    )
                    .unwrap_or(0);
                }
                if octave + 1 < generator.noise_octaves {
                    wavelength_divisor = match wavelength_divisor.checked_mul(generator.lacunarity)
                    {
                        Some(value) => value,
                        None => {
                            wavelengths_are_exact = false;
                            break;
                        }
                    };
                }
            }
        }
        let cell_count = (width / generator.cell_width_quanta.max(1))
            .checked_mul(height / generator.cell_height_quanta.max(1))
            .ok_or(ProfileValidationError::InvalidGeneratorBounds)?;
        if generator.origin_system.strength_hundredths
            != (UnsignedTriangle {
                minimum: 100,
                mode: 100,
                maximum: 100,
            })
            || generator.origin_system.body_count
                != (UnsignedTriangle {
                    minimum: 4,
                    mode: 4,
                    maximum: 12,
                })
            || generator.origin_system.eccentricity_hundredths
                != (UnsignedTriangle {
                    minimum: 100,
                    mode: 100,
                    maximum: 100,
                })
            || generator.origin_system.slots_per_body
                != (UnsignedTriangle {
                    minimum: 3,
                    mode: 3,
                    maximum: 8,
                })
            || generator.frontier_system.strength_hundredths
                != (UnsignedTriangle {
                    minimum: 10,
                    mode: 100,
                    maximum: 300,
                })
            || generator.frontier_system.body_count
                != (UnsignedTriangle {
                    minimum: 1,
                    mode: 4,
                    maximum: 12,
                })
            || generator.frontier_system.eccentricity_hundredths
                != (UnsignedTriangle {
                    minimum: 0,
                    mode: 100,
                    maximum: 150,
                })
            || generator.frontier_system.slots_per_body
                != (UnsignedTriangle {
                    minimum: 1,
                    mode: 3,
                    maximum: 8,
                })
            || generator.coordinate_quanta_per_map_unit == 0
            || generator.coordinate_quanta_per_map_unit != gameplay.coordinate_quanta_per_map_unit
            || generator.target_system_count == 0
            || generator.x_bounds.minimum >= generator.x_bounds.maximum_exclusive
            || generator.y_bounds.minimum >= generator.y_bounds.maximum_exclusive
            || i128::from(generator.x_bounds.minimum)
                + i128::from(generator.x_bounds.maximum_exclusive)
                != 0
            || i128::from(generator.y_bounds.minimum)
                + i128::from(generator.y_bounds.maximum_exclusive)
                != 0
            || generator.generated_z != 0
            || generator.cell_width_quanta == 0
            || generator.cell_height_quanta == 0
            || !width.is_multiple_of(generator.cell_width_quanta.max(1))
            || !height.is_multiple_of(generator.cell_height_quanta.max(1))
            // One cell is reserved for the origin, so eligible cells + origin equals cell_count.
            || generator.target_system_count > cell_count
            || cell_count == 0
            || cell_count - 1 > 1_000_000
            || generator.origin_system.body_count.maximum > 1_000
            || generator.origin_system.slots_per_body.maximum > 1_000
            || generator.frontier_system.body_count.maximum > 1_000
            || generator.frontier_system.slots_per_body.maximum > 1_000
            || generator.origin_system.strength_hundredths.maximum > u64::from(u16::MAX)
            || generator.origin_system.eccentricity_hundredths.maximum > u64::from(u16::MAX)
            || generator.frontier_system.strength_hundredths.maximum > u64::from(u16::MAX)
            || generator.frontier_system.eccentricity_hundredths.maximum > u64::from(u16::MAX)
            || generator.noise_octaves == 0
            || generator.base_wavelength_quanta == 0
            || generator.lacunarity == 0
            || !wavelengths_are_exact
            || !amplitudes_are_positive
            || generator.persistence.numerator == 0
            || generator.persistence.denominator == 0
            || greatest_common_divisor(
                generator.persistence.numerator,
                generator.persistence.denominator,
            ) != 1
            || !generator.full_cell_jitter
        {
            return Err(ProfileValidationError::InvalidGeneratorBounds);
        }
        let known = resources
            .iter()
            .map(|resource| resource.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        for (resource, tuning) in &generator.resources {
            if !known.contains(resource) {
                return Err(ProfileValidationError::UnknownGeneratedResource);
            }
            validate_triangle(tuning.origin.resource_bearing_body_count)?;
            validate_triangle(tuning.origin.quantity_per_body)?;
            validate_triangle(tuning.frontier.resource_bearing_body_count)?;
            validate_triangle(tuning.frontier.quantity_per_body)?;
            if tuning.frontier.presence_basis_points > 10_000
                || tuning.origin.resource_bearing_body_count.minimum == 0
                || tuning.frontier.resource_bearing_body_count.minimum == 0
                || tuning.origin.resource_bearing_body_count.minimum
                    > generator.origin_system.body_count.minimum
                || tuning.frontier.resource_bearing_body_count.minimum
                    > generator.frontier_system.body_count.minimum
                || tuning.origin.quantity_per_body.minimum == 0
                || tuning.frontier.quantity_per_body.minimum == 0
            {
                return Err(ProfileValidationError::InvalidResourceGeneration);
            }
        }
        for resource in resources
            .iter()
            .filter(|resource| resource.naturally_deposit_bearing)
        {
            if !generator.resources.contains_key(&resource.id) {
                return Err(ProfileValidationError::MissingGeneratedResource);
            }
        }
        if generator.resources.keys().any(|id| {
            resources
                .iter()
                .find(|resource| &resource.id == id)
                .is_some_and(|resource| !resource.naturally_deposit_bearing)
        }) {
            return Err(ProfileValidationError::IneligibleGeneratedResource);
        }
        Ok(Self {
            resources,
            gameplay,
            generator,
        })
    }

    #[must_use]
    pub fn resources(&self) -> &[ResourceDefinition] {
        &self.resources
    }

    #[must_use]
    pub fn gameplay(&self) -> &WorldTuning {
        &self.gameplay
    }

    #[must_use]
    pub fn generator(&self) -> &GeneratorTuning {
        &self.generator
    }
}

fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn validate_triangle(value: UnsignedTriangle) -> Result<(), ProfileValidationError> {
    if value.minimum <= value.mode && value.mode <= value.maximum {
        Ok(())
    } else {
        Err(ProfileValidationError::InvalidTriangle)
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProfileValidationError {
    #[error("duplicate resource in normalized profile")]
    DuplicateResource,
    #[error("invalid gameplay tuning: {0}")]
    InvalidGameplay(String),
    #[error("invalid triangular distribution")]
    InvalidTriangle,
    #[error("invalid generator bounds, resolution, noise, or jitter configuration")]
    InvalidGeneratorBounds,
    #[error("generator references an unknown resource")]
    UnknownGeneratedResource,
    #[error("deposit-bearing resource is missing generation tuning")]
    MissingGeneratedResource,
    #[error("non-deposit-bearing resource has generation tuning")]
    IneligibleGeneratedResource,
    #[error("invalid resource generation parameters")]
    InvalidResourceGeneration,
}
