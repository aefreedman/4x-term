use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorldSource {
    pub resources: Vec<ResourceSource>,
    pub locations: Vec<LocationSource>,
    pub origin: OriginSource,
    #[serde(default)]
    pub communities: Vec<CommunitySource>,
    pub systems: Vec<SystemSource>,
    #[serde(default)]
    pub sites: Vec<SiteSource>,
    pub tuning: TuningSource,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceSource {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub naturally_deposit_bearing: bool,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocationSource {
    pub id: String,
    pub name: String,
    pub position: PositionSource,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PositionSource {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OriginSource {
    pub system: String,
    pub community: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommunitySource {
    pub id: String,
    pub system: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SystemSource {
    pub location: String,
    pub stellar_strength_hundredths: u16,
    pub bodies: Vec<BodySource>,
    #[serde(default)]
    pub stocks: Vec<ResourceAmountSource>,
    #[serde(default)]
    pub player_founded: bool,
    #[serde(default)]
    pub command_unlock_received: bool,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BodySource {
    pub id: String,
    pub name: String,
    pub eccentricity_hundredths: u16,
    #[serde(default)]
    pub resources: Vec<ResourceAmountSource>,
    pub slots: Vec<SlotSource>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SlotSource {
    pub id: String,
    #[serde(default)]
    pub development: Option<DevelopmentSource>,
}
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) enum DevelopmentRoleSource {
    Collector,
    Battery,
    Extractor,
    Refinery,
    Habitat,
    Shipyard,
}
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) enum DevelopmentConditionSource {
    Functional,
    Damaged,
    Ruined,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DevelopmentSource {
    pub id: String,
    pub role: DevelopmentRoleSource,
    pub condition: DevelopmentConditionSource,
    #[serde(default)]
    pub extractor_resource: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceAmountSource {
    pub resource: String,
    pub quantity: u64,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SiteSource {
    pub id: String,
    pub location: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TuningSource {
    pub energy_resource: String,
    pub ore_resource: String,
    pub alloy_resource: String,
    pub seasonal_shape: Vec<u64>,
    pub seasonal_baseline_average: u64,
    pub life_support_per_population: u64,
    pub origin_construction_work: u64,
    pub intrinsic_energy_capacity: u64,
    pub battery_energy_capacity: u64,
    pub habitat_population_energy: u64,
    pub coordinate_quanta_per_map_unit: u64,
    pub collector_recipe: RecipeSource,
    pub battery_recipe: RecipeSource,
    pub extractor_recipe: RecipeSource,
    pub refinery_recipe: RecipeSource,
    pub habitat_recipe: RecipeSource,
    pub shipyard_recipe: RecipeSource,
    pub extractor: ExtractorSource,
    pub refinery: RefinerySource,
    pub probe_project: ProbeProjectSource,
    pub expedition_project: ExpeditionProjectSource,
    pub probe_travel: ShipTravelSource,
    pub expedition_travel: ShipTravelSource,
    pub probe_reveal_radius_quanta: u64,
    pub communication_delay_per_quantum: RateSource,
    pub resource_richness: Vec<ResourceRichnessSource>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecipeSource {
    pub costs: Vec<ResourceAmountSource>,
    pub required_work: u64,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExtractorSource {
    pub energy_upkeep: u64,
    pub cycle_duration: u64,
    pub output: u64,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RefinerySource {
    pub energy_upkeep: u64,
    pub cycle_duration: u64,
    pub input: u64,
    pub output: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProbeProjectSource {
    pub material_commitment: Vec<ResourceAmountSource>,
    pub duration_ticks: u64,
    pub energy_per_progress_tick: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExpeditionProjectSource {
    pub hull_material_commitment: Vec<ResourceAmountSource>,
    pub founding_stocks: Vec<ResourceAmountSource>,
    pub duration_ticks: u64,
    pub energy_per_progress_tick: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShipTravelSource {
    pub maximum_jump_quanta: u64,
    pub speed_quanta_per_tick: u64,
    pub energy_per_quantum: RateSource,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RateSource {
    pub numerator: u64,
    pub denominator: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceRichnessSource {
    pub resource: String,
    pub poor_minimum: u64,
    pub poor_maximum: u64,
    pub normal_minimum: u64,
    pub normal_maximum: u64,
    pub rich_minimum: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProfileSource {
    pub resources: Vec<ProfileResourceSource>,
    pub gameplay: TuningSource,
    pub generator: GeneratorSource,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProfileResourceSource {
    pub id: String,
    pub name: String,
    pub naturally_deposit_bearing: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GeneratorSource {
    pub coordinate_quanta_per_map_unit: u64,
    pub target_system_count: u64,
    pub x_bounds: SignedBoundsSource,
    pub y_bounds: SignedBoundsSource,
    pub generated_z: i64,
    pub cell_width_quanta: u64,
    pub cell_height_quanta: u64,
    pub noise_octaves: u32,
    pub base_wavelength_quanta: u64,
    pub lacunarity: u64,
    pub persistence: RatioSource,
    pub full_cell_jitter: bool,
    pub origin_system: SystemGenerationSource,
    pub frontier_system: SystemGenerationSource,
    pub resources: Vec<ResourceGenerationSource>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SignedBoundsSource {
    pub minimum: i64,
    pub maximum_exclusive: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RatioSource {
    pub numerator: u64,
    pub denominator: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TriangleSource {
    pub minimum: u64,
    pub mode: u64,
    pub maximum: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SystemGenerationSource {
    pub strength_hundredths: TriangleSource,
    pub body_count: TriangleSource,
    pub eccentricity_hundredths: TriangleSource,
    pub slots_per_body: TriangleSource,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceGenerationSource {
    pub resource: String,
    pub origin: OriginResourceGenerationSource,
    pub frontier: FrontierResourceGenerationSource,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OriginResourceGenerationSource {
    pub resource_bearing_body_count: TriangleSource,
    pub quantity_per_body: TriangleSource,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrontierResourceGenerationSource {
    pub presence_basis_points: u16,
    pub resource_bearing_body_count: TriangleSource,
    pub quantity_per_body: TriangleSource,
}
