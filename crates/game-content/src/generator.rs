use crate::{
    CanonicalEncodingError, NormalizedProfile, ResourceGenerationTuning, SystemGenerationTuning,
    UnsignedRatio, UnsignedTriangle, canonical_profile_bytes, sha256_fingerprint,
};
use game_core::{
    BodyDefinition, CommunityDefinition, ContentId, DevelopmentCondition, DevelopmentDefinition,
    DevelopmentRole, DevelopmentSlotDefinition, LocationDefinition, Position3, ResourceStore,
    SystemDefinition, WorldDefinition, WorldState,
};
use sha2::{Digest, Sha256};
use std::num::NonZeroU32;
use thiserror::Error;

const STREAM_PREFIX: &[u8] = b"4x-term.frontier-stream\0";
const Q32_ONE: u64 = 1_u64 << 32;
const MAX_GENERATED_SYSTEM_ORDINAL: u64 = 999_999;
const MAX_BODY_OR_SLOT_ORDINAL: u64 = 999;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceProvenance {
    /// Logical content identity only; machine-local filesystem paths are excluded.
    pub source_identity: String,
    pub source_content_sha256: [u8; 32],
}

impl SourceProvenance {
    #[must_use]
    pub fn from_source(source_identity: impl Into<String>, source_content: &[u8]) -> Self {
        Self {
            source_identity: source_identity.into(),
            source_content_sha256: sha256_fingerprint(source_content),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledProfile {
    normalized: NormalizedProfile,
    canonical_bytes: Vec<u8>,
    fingerprint: [u8; 32],
    provenance: SourceProvenance,
}

impl CompiledProfile {
    /// Canonically encodes validated configuration and binds source provenance to it.
    pub fn new(
        normalized: NormalizedProfile,
        provenance: SourceProvenance,
    ) -> Result<Self, CanonicalEncodingError> {
        let canonical_bytes = canonical_profile_bytes(&normalized)?;
        let fingerprint = sha256_fingerprint(&canonical_bytes);
        Ok(Self {
            normalized,
            canonical_bytes,
            fingerprint,
            provenance,
        })
    }

    #[must_use]
    pub fn normalized(&self) -> &NormalizedProfile {
        &self.normalized
    }

    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }

    #[must_use]
    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratorVersion {
    pub family: ContentId,
    pub revision: NonZeroU32,
}

impl GeneratorVersion {
    #[must_use]
    pub fn new(family: ContentId, revision: NonZeroU32) -> Self {
        Self { family, revision }
    }

    #[must_use]
    pub fn frontier_revision_1() -> Self {
        Self {
            family: ContentId::new("core:frontier_world").expect("canonical family is valid"),
            revision: NonZeroU32::MIN,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationRequest {
    pub version: GeneratorVersion,
    pub seed: u64,
    pub configuration: CompiledProfile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationIdentity {
    pub version: GeneratorVersion,
    pub seed: u64,
    pub configuration_fingerprint: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedWorldArtifact {
    identity: GenerationIdentity,
    provenance: SourceProvenance,
    definition: WorldDefinition,
}

impl GeneratedWorldArtifact {
    #[must_use]
    pub fn identity(&self) -> &GenerationIdentity {
        &self.identity
    }

    #[must_use]
    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    #[must_use]
    pub fn definition(&self) -> &WorldDefinition {
        &self.definition
    }

    fn from_generated_definition(request: &GenerationRequest, definition: WorldDefinition) -> Self {
        Self {
            identity: GenerationIdentity {
                version: request.version.clone(),
                seed: request.seed,
                configuration_fingerprint: request.configuration.fingerprint(),
            },
            provenance: request.configuration.provenance().clone(),
            definition,
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum GenerationError {
    #[error("unsupported generator version {family}@{revision}")]
    UnsupportedVersion { family: ContentId, revision: u32 },
    #[error("checked arithmetic overflow during generation")]
    Overflow,
    #[error("invalid generation range or empty weighted choice")]
    InvalidRange,
    #[error("generated identity exceeds its revision-1 ordinal width")]
    GeneratedIdExhausted,
    #[error("generated content id is invalid: {0}")]
    InvalidGeneratedId(String),
    #[error("generated world failed core validation: {0}")]
    InvalidGeneratedWorld(String),
}

/// Generates a complete artifact or returns an error without exposing a partial definition.
pub fn generate_world(
    request: &GenerationRequest,
) -> Result<GeneratedWorldArtifact, GenerationError> {
    if request.version != GeneratorVersion::frontier_revision_1() {
        return Err(GenerationError::UnsupportedVersion {
            family: request.version.family.clone(),
            revision: request.version.revision.get(),
        });
    }
    let mut definition = RevisionOne::new(request).generate()?;
    // Core normalization is also the final reference/arithmetic validation gate.
    WorldState::new(definition.clone())
        .map_err(|error| GenerationError::InvalidGeneratedWorld(error.to_string()))?;
    definition
        .resources
        .sort_by(|left, right| left.id.cmp(&right.id));
    Ok(GeneratedWorldArtifact::from_generated_definition(
        request, definition,
    ))
}

struct RevisionOne<'a> {
    request: &'a GenerationRequest,
    profile: &'a NormalizedProfile,
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    ordinal: u64,
    minimum_x: i64,
    minimum_y: i64,
    weight: u64,
}

impl<'a> RevisionOne<'a> {
    fn new(request: &'a GenerationRequest) -> Self {
        Self {
            request,
            profile: request.configuration.normalized(),
        }
    }

    fn generate(&self) -> Result<WorldDefinition, GenerationError> {
        let tuning = self.profile.generator();
        let width = bounds_width(tuning.x_bounds.minimum, tuning.x_bounds.maximum_exclusive)?;
        let height = bounds_width(tuning.y_bounds.minimum, tuning.y_bounds.maximum_exclusive)?;
        let columns = width / tuning.cell_width_quanta;
        let rows = height / tuning.cell_height_quanta;
        let mut cells = Vec::new();
        let mut weight_sum = 0_u64;
        for row in 0..rows {
            for column in 0..columns {
                let ordinal = row
                    .checked_mul(columns)
                    .and_then(|value| value.checked_add(column))
                    .ok_or(GenerationError::Overflow)?;
                let minimum_x = checked_cell_minimum(
                    tuning.x_bounds.minimum,
                    column,
                    tuning.cell_width_quanta,
                )?;
                let minimum_y =
                    checked_cell_minimum(tuning.y_bounds.minimum, row, tuning.cell_height_quanta)?;
                let maximum_x = i128::from(minimum_x) + i128::from(tuning.cell_width_quanta);
                let maximum_y = i128::from(minimum_y) + i128::from(tuning.cell_height_quanta);
                if i128::from(minimum_x) <= 0
                    && 0 < maximum_x
                    && i128::from(minimum_y) <= 0
                    && 0 < maximum_y
                {
                    continue;
                }
                let noise = self.cell_noise(minimum_x, minimum_y)?;
                let weight = noise.checked_add(1).ok_or(GenerationError::Overflow)?;
                weight_sum = weight_sum
                    .checked_add(weight)
                    .ok_or(GenerationError::Overflow)?;
                cells.push(Cell {
                    ordinal,
                    minimum_x,
                    minimum_y,
                    weight,
                });
            }
        }
        if cells.is_empty() || weight_sum == 0 {
            return Err(GenerationError::InvalidRange);
        }
        let target_non_origin = tuning.target_system_count - 1;
        let mut placed = Vec::new();
        for cell in cells {
            let numerator = target_non_origin
                .checked_mul(cell.weight)
                .ok_or(GenerationError::Overflow)?;
            let present = if numerator >= weight_sum {
                true
            } else {
                self.stream("cell_presence", &cell.ordinal.to_le_bytes())
                    .bounded(weight_sum)?
                    < numerator
            };
            if present {
                placed.push(cell);
            }
        }
        if u64::try_from(placed.len()).map_err(|_| GenerationError::Overflow)?
            > MAX_GENERATED_SYSTEM_ORDINAL + 1
        {
            return Err(GenerationError::GeneratedIdExhausted);
        }

        let origin_id = content_id("core:origin")?;
        let community_id = content_id("core:origin_community")?;
        let mut locations = vec![LocationDefinition {
            id: origin_id.clone(),
            name: "Origin".into(),
            position: Position3::from_quanta(0, 0, 0),
        }];
        let mut systems = vec![self.origin_system(&origin_id)?];
        for (index, cell) in placed.into_iter().enumerate() {
            let ordinal = u64::try_from(index).map_err(|_| GenerationError::Overflow)?;
            let id = generated_system_id(ordinal)?;
            let offset_x = self
                .stream("cell_jitter_x", &cell.ordinal.to_le_bytes())
                .bounded(tuning.cell_width_quanta)?;
            let offset_y = self
                .stream("cell_jitter_y", &cell.ordinal.to_le_bytes())
                .bounded(tuning.cell_height_quanta)?;
            let x = add_offset(cell.minimum_x, offset_x)?;
            let y = add_offset(cell.minimum_y, offset_y)?;
            locations.push(LocationDefinition {
                id: id.clone(),
                name: format!("Frontier System {ordinal:06}"),
                position: Position3::from_quanta(x, y, 0),
            });
            systems.push(self.frontier_system(&id)?);
        }
        locations.sort_by(|left, right| left.id.cmp(&right.id));
        systems.sort_by(|left, right| left.location.cmp(&right.location));

        Ok(WorldDefinition {
            resources: self.profile.resources().to_vec(),
            locations,
            origin_system: origin_id.clone(),
            origin_community: community_id.clone(),
            communities: vec![CommunityDefinition {
                id: community_id,
                system: origin_id,
            }],
            population_tokens: Vec::new(),
            systems,
            sites: Vec::new(),
            tuning: self.profile.gameplay().clone(),
        })
    }

    fn origin_system(&self, system_id: &ContentId) -> Result<SystemDefinition, GenerationError> {
        let tuning = self.profile.generator().origin_system;
        let body_count = self.triangle(
            "origin_body_count",
            entity_id_key(system_id),
            tuning.body_count,
            None,
        )?;
        let mut bodies = self.empty_bodies(system_id, body_count, tuning, true)?;
        for (resource, generation) in &self.profile.generator().resources {
            self.place_resource(
                system_id,
                &mut bodies,
                resource,
                *generation,
                ResourceScope::Origin,
            )?;
        }
        let first = bodies.first_mut().ok_or(GenerationError::InvalidRange)?;
        let first_slot = first
            .slots
            .first_mut()
            .ok_or(GenerationError::InvalidRange)?;
        first_slot.development = Some(DevelopmentDefinition {
            id: content_id(&format!("{}_collector", first_slot.id.as_str()))?,
            role: DevelopmentRole::Collector,
            condition: DevelopmentCondition::Functional,
            extractor_target: None,
        });
        let gameplay = self.profile.gameplay();
        let stocks = [
            (gameplay.energy_resource.clone(), 10),
            (gameplay.ore_resource.clone(), 10),
            (gameplay.alloy_resource.clone(), 0),
        ]
        .into_iter()
        .collect();
        Ok(SystemDefinition {
            location: system_id.clone(),
            stellar_strength_hundredths: 100,
            bodies,
            stocks,
            player_founded: true,
            command_unlock_received: false,
        })
    }

    fn frontier_system(&self, system_id: &ContentId) -> Result<SystemDefinition, GenerationError> {
        let tuning = self.profile.generator().frontier_system;
        let strength = self.triangle(
            "system_strength",
            entity_id_key(system_id),
            tuning.strength_hundredths,
            None,
        )?;
        let strength = u16::try_from(strength).map_err(|_| GenerationError::InvalidRange)?;
        let body_count = self.triangle(
            "system_body_count",
            entity_id_key(system_id),
            tuning.body_count,
            None,
        )?;
        let mut bodies = self.empty_bodies(system_id, body_count, tuning, false)?;
        for (resource, generation) in &self.profile.generator().resources {
            self.place_resource(
                system_id,
                &mut bodies,
                resource,
                *generation,
                ResourceScope::Frontier,
            )?;
        }
        Ok(SystemDefinition {
            location: system_id.clone(),
            stellar_strength_hundredths: strength,
            bodies,
            stocks: ResourceStore::new(),
            player_founded: false,
            command_unlock_received: false,
        })
    }

    fn empty_bodies(
        &self,
        system_id: &ContentId,
        body_count: u64,
        tuning: SystemGenerationTuning,
        origin: bool,
    ) -> Result<Vec<BodyDefinition>, GenerationError> {
        if body_count == 0 || body_count - 1 > MAX_BODY_OR_SLOT_ORDINAL {
            return Err(GenerationError::GeneratedIdExhausted);
        }
        let mut bodies = Vec::new();
        for body_ordinal in 0..body_count {
            let body_id = generated_body_id(system_id, body_ordinal)?;
            let eccentricity = if origin {
                100
            } else {
                u16::try_from(self.triangle(
                    "body_eccentricity",
                    entity_id_key(&body_id),
                    tuning.eccentricity_hundredths,
                    None,
                )?)
                .map_err(|_| GenerationError::InvalidRange)?
            };
            let slot_tag = if origin {
                "origin_body_slot_count"
            } else {
                "body_slot_count"
            };
            let slot_count = self.triangle(
                slot_tag,
                entity_id_key(&body_id),
                tuning.slots_per_body,
                None,
            )?;
            if slot_count == 0 || slot_count - 1 > MAX_BODY_OR_SLOT_ORDINAL {
                return Err(GenerationError::GeneratedIdExhausted);
            }
            let slots = (0..slot_count)
                .map(|slot_ordinal| {
                    Ok(DevelopmentSlotDefinition {
                        id: generated_slot_id(&body_id, slot_ordinal)?,
                        development: None,
                    })
                })
                .collect::<Result<Vec<_>, GenerationError>>()?;
            bodies.push(BodyDefinition {
                id: body_id,
                name: format!("Body {body_ordinal:03}"),
                eccentricity_hundredths: eccentricity,
                initial_resources: ResourceStore::new(),
                slots,
            });
        }
        Ok(bodies)
    }

    fn place_resource(
        &self,
        system_id: &ContentId,
        bodies: &mut [BodyDefinition],
        resource: &ContentId,
        tuning: ResourceGenerationTuning,
        scope: ResourceScope,
    ) -> Result<(), GenerationError> {
        let (body_triangle, quantity_triangle, prefix) = match scope {
            ResourceScope::Origin => (
                tuning.origin.resource_bearing_body_count,
                tuning.origin.quantity_per_body,
                "origin_resource",
            ),
            ResourceScope::Frontier => {
                let presence_tag = format!("resource_presence/{}", resource.as_str());
                let present = self
                    .stream(&presence_tag, &entity_id_key(system_id))
                    .bounded(10_000)?
                    < u64::from(tuning.frontier.presence_basis_points);
                if !present {
                    return Ok(());
                }
                (
                    tuning.frontier.resource_bearing_body_count,
                    tuning.frontier.quantity_per_body,
                    "resource",
                )
            }
        };
        let count_tag = format!("{prefix}_body_count/{}", resource.as_str());
        let body_count = self.triangle(
            &count_tag,
            entity_id_key(system_id),
            body_triangle,
            Some(u64::try_from(bodies.len()).map_err(|_| GenerationError::Overflow)?),
        )?;
        let mut candidates = (0..bodies.len()).collect::<Vec<_>>();
        let mut selected = Vec::new();
        for pick in 0..body_count {
            let pick_tag = format!("{prefix}_body_pick/{}/{}", resource.as_str(), pick);
            let index = self
                .stream(&pick_tag, &entity_id_key(system_id))
                .bounded(u64::try_from(candidates.len()).map_err(|_| GenerationError::Overflow)?)?;
            let index = usize::try_from(index).map_err(|_| GenerationError::Overflow)?;
            selected.push(candidates.remove(index));
        }
        for body_index in selected {
            let body = &mut bodies[body_index];
            let quantity_tag = format!("{prefix}_quantity/{}", resource.as_str());
            let quantity = self.triangle(
                &quantity_tag,
                entity_id_key(&body.id),
                quantity_triangle,
                None,
            )?;
            body.initial_resources.set(resource.clone(), quantity);
        }
        Ok(())
    }

    fn triangle(
        &self,
        tag: &str,
        key: Vec<u8>,
        triangle: UnsignedTriangle,
        maximum_candidate: Option<u64>,
    ) -> Result<u64, GenerationError> {
        let maximum =
            maximum_candidate.map_or(triangle.maximum, |value| triangle.maximum.min(value));
        if maximum < triangle.minimum {
            return Err(GenerationError::InvalidRange);
        }
        let mut candidates = Vec::new();
        let mut total = 0_u128;
        for candidate in triangle.minimum..=maximum {
            let weight = triangle_weight(triangle, candidate)?;
            total = total.checked_add(weight).ok_or(GenerationError::Overflow)?;
            candidates.push((candidate, total));
        }
        let total = u64::try_from(total).map_err(|_| GenerationError::Overflow)?;
        let draw = self.stream(tag, &key).bounded(total)?;
        let draw = u128::from(draw);
        candidates
            .into_iter()
            .find_map(|(candidate, cumulative)| (draw < cumulative).then_some(candidate))
            .ok_or(GenerationError::InvalidRange)
    }

    fn cell_noise(&self, minimum_x: i64, minimum_y: i64) -> Result<u64, GenerationError> {
        let tuning = self.profile.generator();
        let center_x = cell_center_q32(minimum_x, tuning.cell_width_quanta)?;
        let center_y = cell_center_q32(minimum_y, tuning.cell_height_quanta)?;
        let mut amplitude = Q32_ONE;
        let mut amplitude_sum = 0_u128;
        let mut weighted_sum = 0_u128;
        let mut divisor = 1_u64;
        for octave in 0..tuning.noise_octaves {
            let wavelength = tuning
                .base_wavelength_quanta
                .checked_div(divisor)
                .ok_or(GenerationError::InvalidRange)?;
            if wavelength == 0 {
                return Err(GenerationError::InvalidRange);
            }
            let value = self.value_noise_octave(octave, center_x, center_y, wavelength)?;
            weighted_sum = weighted_sum
                .checked_add(
                    u128::from(value)
                        .checked_mul(u128::from(amplitude))
                        .ok_or(GenerationError::Overflow)?,
                )
                .ok_or(GenerationError::Overflow)?;
            amplitude_sum = amplitude_sum
                .checked_add(u128::from(amplitude))
                .ok_or(GenerationError::Overflow)?;
            amplitude = ratio_multiply_q32(amplitude, tuning.persistence)?;
            if octave + 1 < tuning.noise_octaves {
                divisor = divisor
                    .checked_mul(tuning.lacunarity)
                    .ok_or(GenerationError::Overflow)?;
            }
        }
        if amplitude_sum == 0 {
            return Err(GenerationError::InvalidRange);
        }
        u64::try_from(weighted_sum / amplitude_sum).map_err(|_| GenerationError::Overflow)
    }

    fn value_noise_octave(
        &self,
        octave: u32,
        x_q32: i128,
        y_q32: i128,
        wavelength: u64,
    ) -> Result<u64, GenerationError> {
        let wavelength_q32 = i128::from(wavelength)
            .checked_mul(i128::from(Q32_ONE))
            .ok_or(GenerationError::Overflow)?;
        let lattice_x = x_q32.div_euclid(wavelength_q32);
        let lattice_y = y_q32.div_euclid(wavelength_q32);
        let fraction_x = u64::try_from(x_q32.rem_euclid(wavelength_q32) / i128::from(wavelength))
            .map_err(|_| GenerationError::Overflow)?;
        let fraction_y = u64::try_from(y_q32.rem_euclid(wavelength_q32) / i128::from(wavelength))
            .map_err(|_| GenerationError::Overflow)?;
        let fade_x = quintic_fade(fraction_x)?;
        let fade_y = quintic_fade(fraction_y)?;
        let lattice_x = i64::try_from(lattice_x).map_err(|_| GenerationError::Overflow)?;
        let lattice_y = i64::try_from(lattice_y).map_err(|_| GenerationError::Overflow)?;
        let x1 = lattice_x.checked_add(1).ok_or(GenerationError::Overflow)?;
        let y1 = lattice_y.checked_add(1).ok_or(GenerationError::Overflow)?;
        let v00 = self.lattice_value(octave, lattice_x, lattice_y)?;
        let v10 = self.lattice_value(octave, x1, lattice_y)?;
        let v01 = self.lattice_value(octave, lattice_x, y1)?;
        let v11 = self.lattice_value(octave, x1, y1)?;
        let lower = lerp_signed(v00, v10, fade_x)?;
        let upper = lerp_signed(v01, v11, fade_x)?;
        let value = lerp_signed(lower, upper, fade_y)?;
        u64::try_from(value).map_err(|_| GenerationError::Overflow)
    }

    fn lattice_value(&self, octave: u32, x: i64, y: i64) -> Result<i128, GenerationError> {
        let mut key = Vec::with_capacity(16);
        key.extend_from_slice(&x.to_le_bytes());
        key.extend_from_slice(&y.to_le_bytes());
        let tag = format!("noise_lattice/{octave}");
        Ok(i128::from(self.stream(&tag, &key).next() & 0xffff))
    }

    fn stream(&self, tag: &str, entity_key: &[u8]) -> SplitMix64 {
        derive_stream(self.request.seed, &self.request.version, tag, entity_key)
    }
}

#[derive(Clone, Copy)]
enum ResourceScope {
    Origin,
    Frontier,
}

fn bounds_width(minimum: i64, maximum: i64) -> Result<u64, GenerationError> {
    u64::try_from(i128::from(maximum) - i128::from(minimum)).map_err(|_| GenerationError::Overflow)
}

fn checked_cell_minimum(
    bounds_minimum: i64,
    ordinal: u64,
    size: u64,
) -> Result<i64, GenerationError> {
    let offset = i128::from(ordinal)
        .checked_mul(i128::from(size))
        .ok_or(GenerationError::Overflow)?;
    i64::try_from(
        i128::from(bounds_minimum)
            .checked_add(offset)
            .ok_or(GenerationError::Overflow)?,
    )
    .map_err(|_| GenerationError::Overflow)
}

fn add_offset(minimum: i64, offset: u64) -> Result<i64, GenerationError> {
    i64::try_from(i128::from(minimum) + i128::from(offset)).map_err(|_| GenerationError::Overflow)
}

fn cell_center_q32(minimum: i64, size: u64) -> Result<i128, GenerationError> {
    i128::from(minimum)
        .checked_mul(i128::from(Q32_ONE))
        .and_then(|value| value.checked_add(i128::from(size) << 31))
        .ok_or(GenerationError::Overflow)
}

fn ratio_multiply_q32(value: u64, ratio: UnsignedRatio) -> Result<u64, GenerationError> {
    let product = u128::from(value)
        .checked_mul(u128::from(ratio.numerator))
        .ok_or(GenerationError::Overflow)?;
    u64::try_from(product / u128::from(ratio.denominator)).map_err(|_| GenerationError::Overflow)
}

fn multiply_q32(left: u64, right: u64) -> Result<u64, GenerationError> {
    u64::try_from(
        u128::from(left)
            .checked_mul(u128::from(right))
            .ok_or(GenerationError::Overflow)?
            >> 32,
    )
    .map_err(|_| GenerationError::Overflow)
}

fn quintic_fade(value: u64) -> Result<u64, GenerationError> {
    let t2 = multiply_q32(value, value)?;
    let t3 = multiply_q32(t2, value)?;
    let t4 = multiply_q32(t3, value)?;
    let t5 = multiply_q32(t4, value)?;
    let positive = u128::from(t5)
        .checked_mul(6)
        .and_then(|value| value.checked_add(u128::from(t3) * 10))
        .ok_or(GenerationError::Overflow)?;
    let negative = u128::from(t4) * 15;
    u64::try_from(
        positive
            .checked_sub(negative)
            .ok_or(GenerationError::Overflow)?,
    )
    .map_err(|_| GenerationError::Overflow)
}

fn lerp_signed(left: i128, right: i128, t: u64) -> Result<i128, GenerationError> {
    let difference = right.checked_sub(left).ok_or(GenerationError::Overflow)?;
    let scaled = difference
        .checked_mul(i128::from(t))
        .ok_or(GenerationError::Overflow)?
        .div_euclid(i128::from(Q32_ONE));
    left.checked_add(scaled).ok_or(GenerationError::Overflow)
}

fn triangle_weight(triangle: UnsignedTriangle, candidate: u64) -> Result<u128, GenerationError> {
    if candidate < triangle.minimum || candidate > triangle.maximum {
        return Err(GenerationError::InvalidRange);
    }
    if triangle.minimum == triangle.maximum {
        return Ok(1);
    }
    if triangle.mode == triangle.minimum {
        return Ok(u128::from(triangle.maximum - candidate + 1));
    }
    if triangle.mode == triangle.maximum {
        return Ok(u128::from(candidate - triangle.minimum + 1));
    }
    if candidate <= triangle.mode {
        u128::from(candidate - triangle.minimum)
            .checked_mul(u128::from(triangle.maximum - triangle.mode))
            .and_then(|value| value.checked_add(1))
            .ok_or(GenerationError::Overflow)
    } else {
        u128::from(triangle.maximum - candidate)
            .checked_mul(u128::from(triangle.mode - triangle.minimum))
            .and_then(|value| value.checked_add(1))
            .ok_or(GenerationError::Overflow)
    }
}

fn derive_stream(
    seed: u64,
    version: &GeneratorVersion,
    tag: &str,
    entity_key: &[u8],
) -> SplitMix64 {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(STREAM_PREFIX);
    append_length_prefixed(&mut bytes, version.family.as_str().as_bytes());
    bytes.extend_from_slice(&version.revision.get().to_le_bytes());
    bytes.extend_from_slice(&seed.to_le_bytes());
    append_length_prefixed(&mut bytes, tag.as_bytes());
    append_length_prefixed(&mut bytes, entity_key);
    let digest = Sha256::digest(bytes);
    let mut state = [0_u8; 8];
    state.copy_from_slice(&digest[..8]);
    SplitMix64::new(u64::from_le_bytes(state))
}

fn append_length_prefixed(output: &mut Vec<u8>, value: &[u8]) {
    let length = u32::try_from(value.len()).expect("revision-1 stage and ID lengths fit u32");
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(value);
}

fn entity_id_key(id: &ContentId) -> Vec<u8> {
    let mut key = Vec::new();
    append_length_prefixed(&mut key, id.as_str().as_bytes());
    key
}

#[derive(Clone, Copy, Debug)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(state: u64) -> Self {
        Self { state }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    /// Unbiased value in the half-open interval `0..bound`.
    fn bounded(&mut self, bound: u64) -> Result<u64, GenerationError> {
        if bound == 0 {
            return Err(GenerationError::InvalidRange);
        }
        let threshold = 0_u64.wrapping_sub(bound) % bound;
        loop {
            let value = self.next();
            if value >= threshold {
                return Ok(value % bound);
            }
        }
    }
}

fn generated_system_id(ordinal: u64) -> Result<ContentId, GenerationError> {
    if ordinal > MAX_GENERATED_SYSTEM_ORDINAL {
        return Err(GenerationError::GeneratedIdExhausted);
    }
    content_id(&format!("generated:system_{ordinal:06}"))
}

fn generated_body_id(system: &ContentId, ordinal: u64) -> Result<ContentId, GenerationError> {
    if ordinal > MAX_BODY_OR_SLOT_ORDINAL {
        return Err(GenerationError::GeneratedIdExhausted);
    }
    content_id(&format!("{}_body_{ordinal:03}", system.as_str()))
}

fn generated_slot_id(body: &ContentId, ordinal: u64) -> Result<ContentId, GenerationError> {
    if ordinal > MAX_BODY_OR_SLOT_ORDINAL {
        return Err(GenerationError::GeneratedIdExhausted);
    }
    content_id(&format!("{}_slot_{ordinal:03}", body.as_str()))
}

fn content_id(value: &str) -> Result<ContentId, GenerationError> {
    ContentId::new(value).map_err(|_| GenerationError::InvalidGeneratedId(value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_zero_state_vectors_are_exact() {
        let mut stream = SplitMix64::new(0);
        assert_eq!(stream.next(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(stream.next(), 0x6e78_9e6a_a1b9_65f4);
        assert_eq!(stream.next(), 0x06c4_5d18_8009_454f);
    }

    #[test]
    fn domain_separation_changes_only_the_selected_stream() {
        let version = GeneratorVersion::frontier_revision_1();
        let key = 7_u64.to_le_bytes();
        let first = derive_stream(9, &version, "cell_presence", &key).next();
        assert_eq!(
            first,
            derive_stream(9, &version, "cell_presence", &key).next()
        );
        assert_ne!(
            first,
            derive_stream(9, &version, "cell_jitter_x", &key).next()
        );
        assert_ne!(
            first,
            derive_stream(9, &version, "cell_presence", &8_u64.to_le_bytes()).next()
        );
    }

    #[test]
    fn triangular_weights_cover_endpoints_modes_asymmetry_and_truncation() {
        let left_mode = UnsignedTriangle {
            minimum: 1,
            mode: 1,
            maximum: 4,
        };
        assert_eq!(
            (1..=4)
                .map(|x| triangle_weight(left_mode, x).unwrap())
                .collect::<Vec<_>>(),
            vec![4, 3, 2, 1]
        );
        let right_mode = UnsignedTriangle {
            minimum: 1,
            mode: 4,
            maximum: 4,
        };
        assert_eq!(
            (1..=4)
                .map(|x| triangle_weight(right_mode, x).unwrap())
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        let asymmetric = UnsignedTriangle {
            minimum: 1,
            mode: 2,
            maximum: 5,
        };
        assert_eq!(
            (1..=5)
                .map(|x| triangle_weight(asymmetric, x).unwrap())
                .collect::<Vec<_>>(),
            vec![1, 4, 3, 2, 1]
        );
        assert_eq!(
            (1..=3)
                .map(|x| triangle_weight(asymmetric, x).unwrap())
                .collect::<Vec<_>>(),
            vec![1, 4, 3]
        );
    }

    #[test]
    fn negative_cell_center_uses_euclidean_lattice_boundaries() {
        let wavelength_q32 = i128::from(4_u64) * i128::from(Q32_ONE);
        let center = cell_center_q32(-5, 2).expect("center");
        assert_eq!(center.div_euclid(wavelength_q32), -1);
        assert_eq!(center.rem_euclid(wavelength_q32), 0);
        let half = cell_center_q32(-5, 1).expect("half center");
        assert_eq!(half.div_euclid(wavelength_q32), -2);
        assert_eq!(half.rem_euclid(wavelength_q32), i128::from(Q32_ONE) * 7 / 2);
    }
}
