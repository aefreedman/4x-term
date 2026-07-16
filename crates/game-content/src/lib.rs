//! RON loading, validation, and compilation into format-independent core definitions.

mod energy_logistics;

use energy_logistics::{EnergyLogisticsOverrideSource, EnergyLogisticsSource};
use game_core::{
    BrownoutConfig, ContentId, ENERGY_ID, EconomyConfig, Energy, FleetArchetype, FleetDynamics,
    FleetMode, GameDefinition, GoodAmount, GoodCategory, GoodDefinition, Governance,
    InvestmentKind, InvestmentPolicy, InvestmentShape, LiquidationTraderCapability,
    MAX_POPULATION_SUFFICIENCY_WINDOW_TICKS, MarketAuthority, MarketPolicy, PopulationConfig,
    PopulationState, Position3, PricingMode, RecipeDefinition, RecipeLayer, RecipeOutput,
    RefuelPolicy, SeasonalGenerationState, SourceDefinition, SystemDefinition, SystemGraph,
    TradeNetworkAccess, TraderDefinition, compute_protected_liquidation_budget,
    route_travel_energy, scaled_source_output, ticks_for_distance, validate_investment_shapes,
    validate_market_investment_bounds, validate_population_config,
};
#[cfg(test)]
use game_core::{liquidation_target_energy, liquidation_unit_price};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const EFFICIENCY_SCALE: i64 = 1_000;

#[derive(Error, Debug)]
pub enum ContentError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: Box<ron::error::SpannedError>,
    },
    #[error("content validation failed:\n{}", .0.join("\n"))]
    Validation(Vec<String>),
}

/// Non-fatal, source-aware diagnostics produced while compiling authored content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentWarning {
    BootstrapRunwayAcknowledged {
        source: &'static str,
        system: ContentId,
        starting_energy: u64,
        required_burn_per_tick: i64,
        runway_ticks: u64,
        required_ticks: u32,
        exporter: ContentId,
        trader: ContentId,
    },
    BootstrapDeliveryAcknowledged {
        source: &'static str,
        system: ContentId,
        detail: String,
    },
}

#[derive(Clone, Debug)]
pub struct LoadedContent {
    pub definition: GameDefinition,
    pub encyclopedia: Vec<EncyclopediaSection>,
    pub warnings: Vec<ContentWarning>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct EncyclopediaSection {
    pub title: String,
    pub articles: Vec<EncyclopediaArticle>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct EncyclopediaArticle {
    pub title: String,
    pub paragraphs: Vec<String>,
}

#[derive(Deserialize)]
struct SystemSource {
    id: String,
    name: String,
    position: PositionSource,
}
#[derive(Deserialize)]
struct PositionSource {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Deserialize)]
struct GoodSource {
    id: String,
    name: String,
    category: CategorySource,
    bootstrap_cost: i64,
}
#[derive(Clone, Copy, Deserialize)]
enum CategorySource {
    Energy,
    Raw,
    Primary,
    Secondary,
}

#[derive(Clone, Deserialize)]
struct RecipeSource {
    id: String,
    name: String,
    layer: LayerSource,
    inputs: Vec<AmountSource>,
    outputs: Vec<OutputSource>,
    operating_energy: i64,
    #[serde(default)]
    margin_percent: Option<u32>,
}
#[derive(Clone, Copy, Deserialize)]
enum LayerSource {
    Primary,
    Secondary,
    Tertiary,
}
#[derive(Clone, Deserialize)]
struct AmountSource {
    good: String,
    quantity: u32,
}
#[derive(Clone, Deserialize)]
struct OutputSource {
    good: String,
    quantity: u32,
    cost_weight: u32,
}

#[derive(Deserialize)]
struct EconomySource {
    markets: Vec<MarketSource>,
}
#[derive(Deserialize)]
struct SourceSource {
    good: String,
    quantity_per_tick: u32,
    extraction_energy: i64,
}

#[derive(Clone, Copy, Deserialize)]
enum PricingModeSource {
    Scarcity,
    CostAware,
}

#[derive(Deserialize)]
struct EconomyConfigSource {
    pricing_mode: PricingModeSource,
    producer_margin_percent: u32,
    operating_reserve_ticks: u32,
    #[serde(default)]
    import_priorities: Vec<PrioritySource>,
    liquidation_threshold_percent: u32,
    liquidation_discount_percent: u32,
    default_target: u32,
    reservation_ttl: u32,
    life_support_burn_per_capita: i64,
    source_output_percent: u32,
    idle_trader_repositioning: bool,
    brownouts: BrownoutConfigSource,
    population: PopulationConfigSource,
    investments: Vec<InvestmentShapeSource>,
    default_investment_allocation: Vec<InvestmentAllocationSource>,
    energy_logistics: EnergyLogisticsSource,
}

#[derive(Deserialize)]
struct BrownoutConfigSource {
    throttled_entry_ticks: u32,
    emergency_entry_ticks: u32,
    starvation_entry_ticks: u32,
    throttled_recovery_ticks: u32,
    emergency_recovery_ticks: u32,
    starvation_recovery_ticks: u32,
    minimum_stage_ticks: u32,
    throttled_throughput_percent: u32,
    emergency_throughput_percent: u32,
    starvation_throughput_percent: u32,
    survival_goods: Vec<String>,
}

#[derive(Deserialize)]
struct PopulationConfigSource {
    static_population: bool,
    sufficiency_window: u32,
    growth_sufficiency_percent: u32,
    essential_goods: Vec<String>,
    tertiary_demand: Vec<PopulationDemandSource>,
    decline_per_thousand: u32,
    growth_per_thousand: u32,
    logistic_scale: u32,
    minimum_cap: u64,
    maximum_cap: u64,
    tier_thresholds: Vec<u64>,
}

#[derive(Deserialize)]
struct PopulationDemandSource {
    good: String,
    units_per_thousand: u32,
}

#[derive(Clone, Copy, Deserialize)]
enum InvestmentKindSource {
    Collector,
    Storage,
    PopulationSupport,
    RouteSubsidy,
}

impl From<InvestmentKindSource> for InvestmentKind {
    fn from(value: InvestmentKindSource) -> Self {
        match value {
            InvestmentKindSource::Collector => Self::Collector,
            InvestmentKindSource::Storage => Self::Storage,
            InvestmentKindSource::PopulationSupport => Self::PopulationSupport,
            InvestmentKindSource::RouteSubsidy => Self::RouteSubsidy,
        }
    }
}

#[derive(Deserialize)]
struct InvestmentShapeSource {
    kind: InvestmentKindSource,
    enabled: bool,
    base_cost: i64,
    cost_growth_percent: u32,
    maximum_level: u32,
    cooldown_ticks: u32,
    effect_per_level: u32,
}

#[derive(Clone, Deserialize)]
struct InvestmentAllocationSource {
    kind: InvestmentKindSource,
    percent: u32,
}
#[derive(Clone, Deserialize)]
struct PrioritySource {
    good: String,
    percent: u32,
}

#[derive(Default, Deserialize)]
struct MarketPolicyOverrideSource {
    pricing_mode: Option<PricingModeSource>,
    producer_margin_percent: Option<u32>,
    operating_reserve_ticks: Option<u32>,
    import_priorities: Option<Vec<PrioritySource>>,
    liquidation_threshold_percent: Option<u32>,
    liquidation_discount_percent: Option<u32>,
    default_target: Option<u32>,
}

#[derive(Deserialize)]
struct MarketSource {
    system: String,
    starting_energy: u64,
    inventory: Vec<AmountSource>,
    targets: Vec<AmountSource>,
    recipes: Vec<String>,
    sources: Vec<SourceSource>,
    star_luminosity: i64,
    collector_efficiency: u32,
    energy_storage_cap: i64,
    population: u64,
    #[serde(default)]
    seasonal: SeasonalSource,
    #[serde(default)]
    population_reference: Option<u64>,
    #[serde(default)]
    population_cap: Option<u64>,
    #[serde(default)]
    investment_allocation: Option<Vec<InvestmentAllocationSource>>,
    #[serde(default)]
    governor: Option<String>,
    #[serde(default)]
    policy: MarketPolicyOverrideSource,
    #[serde(default)]
    energy_logistics: EnergyLogisticsOverrideSource,
    #[serde(default)]
    acknowledge_bootstrap_risk: bool,
}

#[derive(Deserialize)]
struct SeasonalSource {
    amplitude_percent: u32,
    #[serde(default = "default_seasonal_period")]
    period_ticks: u32,
    phase_ticks: u32,
}

impl Default for SeasonalSource {
    fn default() -> Self {
        Self {
            amplitude_percent: 0,
            period_ticks: default_seasonal_period(),
            phase_ticks: 0,
        }
    }
}

fn default_seasonal_period() -> u32 {
    100
}

#[derive(Deserialize)]
struct TraderConfigSource {
    player: PlayerTraderSource,
    npcs: NpcTraderSource,
}
#[derive(Deserialize)]
struct PlayerTraderSource {
    id: String,
    name: String,
    starting_system: String,
    energy_tank: i64,
    energy_tank_capacity: i64,
    bulk_energy_capacity: i64,
    cargo_capacity: u32,
    speed: f64,
    travel_burn_per_distance: i64,
    refuel_policy: RefuelPolicySource,
    #[serde(default)]
    trade_network_access: TradeNetworkAccessSource,
}
#[derive(Deserialize)]
struct NpcTraderSource {
    mode: NpcFleetModeSource,
    archetypes: Vec<NpcArchetypeSource>,
    dynamic: DynamicFleetSource,
}

#[derive(Deserialize)]
struct NpcArchetypeSource {
    id: String,
    id_prefix: String,
    name_prefix: String,
    initial_count: usize,
    maximum_count: usize,
    energy_tank: i64,
    energy_tank_capacity: i64,
    bulk_energy_capacity: i64,
    cargo_capacity: u32,
    speed: f64,
    travel_burn_per_distance: i64,
    refuel_policy: RefuelPolicySource,
    initial_distribution: Vec<String>,
}

#[derive(Clone, Copy, Deserialize)]
enum NpcFleetModeSource {
    Fixed,
    Dynamic,
}

#[derive(Deserialize)]
struct DynamicFleetSource {
    opportunity_threshold: u64,
    opportunity_window: u32,
    spawn_cooldown_ticks: u32,
    retirement_window: u32,
    retirement_threshold: i64,
    maximum_count: usize,
}
#[derive(Clone, Copy, Deserialize)]
enum RefuelPolicySource {
    DepositAndWithdraw,
    DepositOnly,
    Disabled,
}

#[derive(Clone, Copy, Default, Deserialize)]
enum TradeNetworkAccessSource {
    #[default]
    Offline,
    ReservationContracts,
}

impl From<TradeNetworkAccessSource> for TradeNetworkAccess {
    fn from(value: TradeNetworkAccessSource) -> Self {
        match value {
            TradeNetworkAccessSource::Offline => Self::Offline,
            TradeNetworkAccessSource::ReservationContracts => Self::ReservationContracts,
        }
    }
}

impl From<RefuelPolicySource> for RefuelPolicy {
    fn from(value: RefuelPolicySource) -> Self {
        match value {
            RefuelPolicySource::DepositAndWithdraw => Self::DepositAndWithdraw,
            RefuelPolicySource::DepositOnly => Self::DepositOnly,
            RefuelPolicySource::Disabled => Self::Disabled,
        }
    }
}
pub fn load_directory(root: impl AsRef<Path>) -> Result<GameDefinition, ContentError> {
    Ok(load_directory_with_warnings(root)?.definition)
}

pub fn load_directory_with_warnings(root: impl AsRef<Path>) -> Result<LoadedContent, ContentError> {
    let root = root.as_ref();
    let encyclopedia: Vec<EncyclopediaSection> = load(root.join("encyclopedia.ron"))?;
    let mut errors = Vec::new();
    validate_encyclopedia(&encyclopedia, &mut errors);
    if !errors.is_empty() {
        return Err(ContentError::Validation(errors));
    }
    let mut loaded = compile(
        load(root.join("systems.ron"))?,
        load(root.join("goods.ron"))?,
        load(root.join("recipes.ron"))?,
        load(root.join("economy.ron"))?,
        load(root.join("economy_config.ron"))?,
        load(root.join("traders.ron"))?,
    )?;
    loaded.encyclopedia = encyclopedia;
    Ok(loaded)
}

fn validate_encyclopedia(sections: &[EncyclopediaSection], errors: &mut Vec<String>) {
    if sections.is_empty() {
        errors.push("encyclopedia.ron: at least one section is required".into());
        return;
    }
    let mut section_titles = BTreeSet::new();
    for section in sections {
        let title = section.title.trim();
        if title.is_empty() {
            errors.push("encyclopedia.ron: section title must not be empty".into());
        } else if !section_titles.insert(title) {
            errors.push(format!(
                "encyclopedia.ron: duplicate section title {title:?}"
            ));
        }
        if section.articles.is_empty() {
            errors.push(format!(
                "encyclopedia.ron:{title}: at least one article is required"
            ));
        }
        let mut article_titles = BTreeSet::new();
        for article in &section.articles {
            let article_title = article.title.trim();
            if article_title.is_empty() {
                errors.push(format!(
                    "encyclopedia.ron:{title}: article title must not be empty"
                ));
            } else if !article_titles.insert(article_title) {
                errors.push(format!(
                    "encyclopedia.ron:{title}: duplicate article title {article_title:?}"
                ));
            }
            if article.paragraphs.is_empty()
                || article
                    .paragraphs
                    .iter()
                    .any(|paragraph| paragraph.trim().is_empty())
            {
                errors.push(format!(
                    "encyclopedia.ron:{title}:{article_title}: paragraphs must not be empty"
                ));
            }
        }
    }
}

fn load<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<T, ContentError> {
    let text = fs::read_to_string(&path).map_err(|source| ContentError::Read {
        path: path.clone(),
        source,
    })?;
    ron::from_str(&text).map_err(|source| ContentError::Parse {
        path,
        source: Box::new(source),
    })
}

fn parse_id(raw: &str, context: &str, errors: &mut Vec<String>) -> Option<ContentId> {
    match ContentId::new(raw) {
        Ok(id) => Some(id),
        Err(error) => {
            errors.push(format!("{context}: {error}"));
            None
        }
    }
}

fn compile(
    systems: Vec<SystemSource>,
    goods: Vec<GoodSource>,
    recipes: Vec<RecipeSource>,
    economy: EconomySource,
    config: EconomyConfigSource,
    traders: TraderConfigSource,
) -> Result<LoadedContent, ContentError> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    if systems.len() != 20 {
        errors.push(format!(
            "systems.ron: expected exactly 20 systems, found {}",
            systems.len()
        ));
    }

    let mut seen = BTreeSet::new();
    let mut categories = BTreeMap::new();
    let mut compiled_goods = Vec::new();
    for source in goods {
        let Some(id) = parse_id(&source.id, "goods.ron", &mut errors) else {
            continue;
        };
        if !seen.insert(id.clone()) {
            errors.push(format!("goods.ron: duplicate id {id}"));
            continue;
        }
        if source.bootstrap_cost <= 0 {
            errors.push(format!("goods.ron:{id}: bootstrap_cost must be positive"));
        }
        let category = match source.category {
            CategorySource::Energy => GoodCategory::Energy,
            CategorySource::Raw => GoodCategory::Raw,
            CategorySource::Primary => GoodCategory::Primary,
            CategorySource::Secondary => GoodCategory::Secondary,
        };
        categories.insert(id.clone(), category);
        compiled_goods.push(GoodDefinition {
            id,
            name: source.name,
            category,
            bootstrap_cost: Energy(source.bootstrap_cost),
        });
    }
    let energy_matches = compiled_goods
        .iter()
        .filter(|good| good.id.as_str() == ENERGY_ID)
        .collect::<Vec<_>>();
    if energy_matches.len() != 1
        || energy_matches[0].category != GoodCategory::Energy
        || energy_matches[0].bootstrap_cost != Energy(1)
    {
        errors.push("goods.ron: core:energy must appear exactly once with category Energy and bootstrap_cost 1".into());
    }
    if compiled_goods
        .iter()
        .any(|good| good.category == GoodCategory::Energy && good.id.as_str() != ENERGY_ID)
    {
        errors.push("goods.ron: core:energy is the only permitted Energy-category good".into());
    }
    let good_ids = compiled_goods
        .iter()
        .map(|good| good.id.clone())
        .collect::<BTreeSet<_>>();

    let mut recipe_seen = BTreeSet::new();
    let mut compiled_recipes = Vec::new();
    for source in recipes {
        let Some(id) = parse_id(&source.id, "recipes.ron", &mut errors) else {
            continue;
        };
        if !recipe_seen.insert(id.clone()) {
            errors.push(format!("recipes.ron: duplicate id {id}"));
            continue;
        }
        if source.operating_energy < 0 {
            errors.push(format!(
                "recipes.ron:{id}: operating_energy cannot be negative"
            ));
        }
        if source.margin_percent.is_some_and(|value| value > 10_000) {
            errors.push(format!("recipes.ron:{id}: margin_percent is out of range"));
        }
        let layer = match source.layer {
            LayerSource::Primary => RecipeLayer::Primary,
            LayerSource::Secondary => RecipeLayer::Secondary,
            LayerSource::Tertiary => RecipeLayer::Tertiary,
        };
        let inputs = source
            .inputs
            .into_iter()
            .filter_map(|value| {
                let good = parse_id(&value.good, &format!("recipes.ron:{id}:input"), &mut errors)?;
                if !good_ids.contains(&good) {
                    errors.push(format!("recipes.ron:{id}: unknown good {good}"));
                }
                if value.quantity == 0 {
                    errors.push(format!("recipes.ron:{id}: input quantity must be positive"));
                }
                Some(GoodAmount {
                    good,
                    quantity: value.quantity,
                })
            })
            .collect::<Vec<_>>();
        let outputs = source
            .outputs
            .into_iter()
            .filter_map(|value| {
                let good = parse_id(
                    &value.good,
                    &format!("recipes.ron:{id}:output"),
                    &mut errors,
                )?;
                if !good_ids.contains(&good) {
                    errors.push(format!("recipes.ron:{id}: unknown good {good}"));
                }
                if value.quantity == 0 || value.cost_weight == 0 {
                    errors.push(format!(
                        "recipes.ron:{id}: output quantity and cost_weight must be positive"
                    ));
                }
                Some(RecipeOutput {
                    good,
                    quantity: value.quantity,
                    cost_weight: value.cost_weight,
                })
            })
            .collect::<Vec<_>>();
        if inputs.is_empty() {
            errors.push(format!("recipes.ron:{id}: inputs cannot be empty"));
        }
        let mut input_goods = BTreeSet::new();
        for input in &inputs {
            if !input_goods.insert(input.good.clone()) {
                errors.push(format!(
                    "recipes.ron:{id}:input: duplicate good {}",
                    input.good
                ));
            }
        }
        let mut output_goods = BTreeSet::new();
        for output in &outputs {
            if !output_goods.insert(output.good.clone()) {
                errors.push(format!(
                    "recipes.ron:{id}:output: duplicate good {}",
                    output.good
                ));
            }
        }
        validate_recipe_layers(&id, layer, &inputs, &outputs, &categories, &mut errors);
        compiled_recipes.push(RecipeDefinition {
            id,
            name: source.name,
            layer,
            inputs,
            outputs,
            operating_energy: Energy(source.operating_energy),
            margin_percent: source.margin_percent,
        });
    }
    let recipe_ids = compiled_recipes
        .iter()
        .map(|recipe| recipe.id.clone())
        .collect::<BTreeSet<_>>();

    let default_policy = compile_policy_defaults(&config, &good_ids, &mut errors);
    let default_energy_logistics =
        energy_logistics::compile_global(&config.energy_logistics, &mut errors);
    let brownouts = compile_brownouts(&config.brownouts, &good_ids, &mut errors);
    let population_config = compile_population_config(&config.population, &good_ids, &mut errors);
    let investments =
        compile_investment_shapes(&config.investments, &population_config, &mut errors);
    let default_investment_policy = compile_investment_policy(
        &config.default_investment_allocation,
        "economy_config.ron:default_investment_allocation",
        &mut errors,
    );
    validate_config(&config, &default_policy, &mut errors);
    let compiled_config = EconomyConfig {
        reservation_ttl: config.reservation_ttl,
        life_support_burn_per_capita: Energy(config.life_support_burn_per_capita),
        source_output_percent: config.source_output_percent,
        idle_trader_repositioning: config.idle_trader_repositioning,
        brownouts,
        energy_logistics: default_energy_logistics,
        population: population_config,
        investments,
    };

    let mut markets = BTreeMap::new();
    for source in economy.markets {
        let Some(system) = parse_id(&source.system, "economy.ron:market", &mut errors) else {
            continue;
        };
        if markets.contains_key(&system) {
            errors.push(format!("economy.ron: duplicate market {system}"));
            continue;
        }
        let mut inventory = amounts_to_map(source.inventory, &good_ids, "inventory", &mut errors);
        if inventory
            .insert(
                ContentId::new(ENERGY_ID).expect("constant id"),
                source.starting_energy,
            )
            .is_some()
        {
            errors.push(format!(
                "economy.ron:{system}: core:energy must use starting_energy, not inventory"
            ));
        }
        let targets_u64 = amounts_to_map(source.targets, &good_ids, "targets", &mut errors);
        let targets = targets_u64
            .into_iter()
            .filter_map(|(id, value)| match u32::try_from(value) {
                Ok(value) if value > 0 => Some((id, value)),
                _ => {
                    errors.push(format!(
                        "economy.ron:{system}: targets must be positive u32 values"
                    ));
                    None
                }
            })
            .collect();
        let mut market_recipe_ids = BTreeSet::new();
        let recipe_refs = source
            .recipes
            .into_iter()
            .filter_map(|raw| {
                let parsed = parse_id(&raw, &format!("economy.ron:{system}:recipe"), &mut errors)?;
                if !recipe_ids.contains(&parsed) {
                    errors.push(format!("economy.ron:{system}: unknown recipe {parsed}"));
                }
                if !market_recipe_ids.insert(parsed.clone()) {
                    errors.push(format!(
                        "economy.ron:{system}:recipes: duplicate recipe {parsed}"
                    ));
                }
                Some(parsed)
            })
            .collect();
        let mut market_source_goods = BTreeSet::new();
        let sources = source.sources.into_iter().filter_map(|value| {
            let good = parse_id(&value.good, &format!("economy.ron:{system}:source"), &mut errors)?;
            if categories.get(&good) != Some(&GoodCategory::Raw) { errors.push(format!("economy.ron:{system}: source {good} must be raw")); }
            if !market_source_goods.insert(good.clone()) { errors.push(format!("economy.ron:{system}:sources: duplicate source good {good}")); }
            if value.quantity_per_tick == 0 || value.extraction_energy < 0 { errors.push(format!("economy.ron:{system}: source quantity must be positive and extraction_energy non-negative")); }
            Some(SourceDefinition { good, quantity_per_tick: value.quantity_per_tick, extraction_energy: Energy(value.extraction_energy) })
        }).collect();
        let output = checked_generation(source.star_luminosity, source.collector_efficiency)
            .map_err(|message| format!("economy.ron:{system}:{message}"));
        let energy_output_per_tick = match output {
            Ok(value) => value,
            Err(message) => {
                errors.push(message);
                Energy(0)
            }
        };
        if source.energy_storage_cap <= 0
            || source.starting_energy > u64::try_from(source.energy_storage_cap).unwrap_or(0)
        {
            errors.push(format!(
                "economy.ron:{system}: starting_energy must fit positive energy_storage_cap"
            ));
        }
        let seasonal_shape_valid = source.seasonal.amplitude_percent <= 100
            && source.seasonal.period_ticks >= 2
            && source.seasonal.phase_ticks < source.seasonal.period_ticks;
        if !seasonal_shape_valid {
            errors.push(format!(
                "economy.ron:{system}:seasonal: amplitude must be 0..=100, period >= 2, and phase < period"
            ));
        }
        let seasonal_period_valid = source.seasonal.amplitude_percent == 0
            || source.seasonal.period_ticks.is_multiple_of(2);
        if !seasonal_period_valid {
            errors.push(format!(
                "economy.ron:{system}:seasonal: nonzero amplitude requires an even period"
            ));
        }
        let seasonal_generation = SeasonalGenerationState {
            base_output: energy_output_per_tick,
            amplitude_percent: source.seasonal.amplitude_percent,
            period_ticks: source.seasonal.period_ticks,
            phase_ticks: source.seasonal.phase_ticks,
            current_effective_output: energy_output_per_tick,
        };
        if seasonal_shape_valid && seasonal_period_valid && seasonal_generation.validate().is_err()
        {
            errors.push(format!(
                "economy.ron:{system}:seasonal: output bounds overflow"
            ));
        }
        if seasonal_shape_valid
            && seasonal_period_valid
            && validate_market_investment_bounds(
                &compiled_config.investments,
                &seasonal_generation,
                Energy(source.energy_storage_cap),
            )
            .is_err()
        {
            errors.push(format!(
                "economy.ron:{system}:investments: maximum collector/storage effect exceeds generation or storage bounds"
            ));
        }
        let reference = source.population_reference.unwrap_or(source.population);
        let cap = source.population_cap.unwrap_or(reference);
        if reference == 0
            || cap < source.population
            || cap < reference
            || cap < compiled_config.population.minimum_cap
            || cap > compiled_config.population.maximum_cap
        {
            errors.push(format!(
                "economy.ron:{system}:population: reference must be positive and cap must cover current/reference within configured bounds"
            ));
        }
        let population_state = PopulationState {
            current: source.population,
            reference,
            carrying_capacity: cap,
            support_capacity: cap,
            ..PopulationState::default()
        };
        let investment_policy = source.investment_allocation.map_or_else(
            || default_investment_policy.clone(),
            |values| {
                compile_investment_policy(
                    &values,
                    &format!("economy.ron:{system}:investment_allocation"),
                    &mut errors,
                )
            },
        );
        let governance = match source.governor {
            Some(raw) => parse_id(&raw, &format!("economy.ron:{system}:governor"), &mut errors)
                .map_or_else(Governance::default, |id| Governance {
                    authority: MarketAuthority::Player(id),
                }),
            None => Governance::default(),
        };
        let context = format!("economy.ron:{system}");
        let policy = merge_policy(
            &default_policy,
            source.policy,
            &good_ids,
            &context,
            &mut errors,
        );
        let energy_logistics = energy_logistics::merge_market(
            default_energy_logistics,
            source.energy_logistics,
            &context,
            &mut errors,
        );
        markets.insert(
            system,
            MarketCompiled {
                inventory,
                targets,
                recipes: recipe_refs,
                sources,
                energy_output_per_tick,
                seasonal_generation,
                energy_storage_cap: Energy(source.energy_storage_cap),
                population: source.population,
                population_state,
                investment_policy,
                governance,
                policy,
                energy_logistics,
                acknowledged: source.acknowledge_bootstrap_risk,
            },
        );
    }

    let mut system_seen = BTreeSet::new();
    let mut positions = BTreeSet::new();
    let mut compiled_systems = Vec::new();
    for source in systems {
        let Some(id) = parse_id(&source.id, "systems.ron", &mut errors) else {
            continue;
        };
        if !system_seen.insert(id.clone()) {
            errors.push(format!("systems.ron: duplicate id {id}"));
            continue;
        }
        let position = Position3 {
            x: source.position.x,
            y: source.position.y,
            z: source.position.z,
        };
        if !position.is_finite() {
            errors.push(format!("systems.ron:{id}: position must be finite"));
        }
        if !positions.insert((
            position.x.to_bits(),
            position.y.to_bits(),
            position.z.to_bits(),
        )) {
            errors.push(format!("systems.ron:{id}: duplicate position"));
        }
        let Some(market) = markets.remove(&id) else {
            errors.push(format!("economy.ron: missing market for {id}"));
            continue;
        };
        compiled_systems.push(SystemDefinition {
            id,
            name: source.name,
            position,
            inventory: market.inventory,
            targets: market.targets,
            recipes: market.recipes,
            sources: market.sources,
            energy_output_per_tick: market.energy_output_per_tick,
            seasonal_generation: market.seasonal_generation,
            energy_storage_cap: market.energy_storage_cap,
            population: market.population,
            population_state: market.population_state,
            investment_policy: market.investment_policy,
            governance: market.governance,
            policy: market.policy,
            energy_logistics: market.energy_logistics,
            protected_liquidation_budget: Energy(0),
            bootstrap_risk_acknowledged: market.acknowledged,
        });
    }
    for id in markets.keys() {
        errors.push(format!(
            "economy.ron: market references unknown system {id}"
        ));
    }
    if compiled_systems.len() > 2 {
        let distances = compiled_systems
            .iter()
            .enumerate()
            .flat_map(|(i, a)| {
                compiled_systems[i + 1..]
                    .iter()
                    .map(move |b| a.position.distance(b.position).to_bits())
            })
            .collect::<BTreeSet<_>>();
        if distances.len() < 2 {
            errors.push("systems.ron: system distances must not be uniform".into());
        }
    }

    let (compiled_traders, player_trade_network_access, fleet) =
        compile_traders(traders, &compiled_systems, &mut errors);
    let player_ids = compiled_traders
        .iter()
        .filter(|trader| trader.player)
        .map(|trader| trader.id.clone())
        .collect::<BTreeSet<_>>();
    for system in &compiled_systems {
        if let MarketAuthority::Player(player) = &system.governance.authority
            && !player_ids.contains(player)
        {
            errors.push(format!(
                "economy.ron:{}:governor: unknown player {player}",
                system.id
            ));
        }
    }
    validate_roles_and_anticorrelation(
        &compiled_systems,
        &compiled_recipes,
        &compiled_config,
        &mut errors,
    );

    let graph = if compiled_systems.len() == 20
        && compiled_systems
            .iter()
            .all(|system| system.position.is_finite())
    {
        match SystemGraph::build(&compiled_systems) {
            Ok(graph) => Some(graph),
            Err(error) => {
                errors.push(format!("system graph: {error}"));
                None
            }
        }
    } else {
        None
    };
    if errors.is_empty()
        && let Some(graph) = &graph
    {
        validate_archetype_route_capacity(&fleet, &compiled_systems, graph, &mut errors);
        compute_protected_budgets(
            &mut compiled_systems,
            &compiled_goods,
            &compiled_traders,
            &fleet,
            graph,
            &mut errors,
        );
        validate_bootstrap(
            &compiled_systems,
            &compiled_recipes,
            &compiled_traders,
            &compiled_config,
            graph,
            &mut warnings,
            &mut errors,
        );
    }
    if !errors.is_empty() {
        return Err(ContentError::Validation(errors));
    }
    Ok(LoadedContent {
        definition: GameDefinition {
            goods: compiled_goods,
            recipes: compiled_recipes,
            systems: compiled_systems,
            traders: compiled_traders,
            player_trade_network_access,
            fleet,
            economy: compiled_config,
        },
        encyclopedia: Vec::new(),
        warnings,
    })
}

struct MarketCompiled {
    inventory: BTreeMap<ContentId, u64>,
    targets: BTreeMap<ContentId, u32>,
    recipes: Vec<ContentId>,
    sources: Vec<SourceDefinition>,
    energy_output_per_tick: Energy,
    seasonal_generation: SeasonalGenerationState,
    energy_storage_cap: Energy,
    population: u64,
    population_state: PopulationState,
    investment_policy: InvestmentPolicy,
    governance: Governance,
    policy: MarketPolicy,
    energy_logistics: game_core::EnergyLogisticsPolicy,
    acknowledged: bool,
}

fn checked_generation(luminosity: i64, efficiency: u32) -> Result<Energy, &'static str> {
    if luminosity < 0 || efficiency > EFFICIENCY_SCALE as u32 {
        return Err(
            "star_luminosity must be non-negative and collector_efficiency must be 0..=1000",
        );
    }
    luminosity
        .checked_mul(i64::from(efficiency))
        .map(|value| Energy(value / EFFICIENCY_SCALE))
        .ok_or("generation multiplication overflow")
}

fn pricing_mode(value: PricingModeSource) -> PricingMode {
    match value {
        PricingModeSource::Scarcity => PricingMode::Scarcity,
        PricingModeSource::CostAware => PricingMode::CostAware,
    }
}

fn compile_priorities(
    values: Vec<PrioritySource>,
    goods: &BTreeSet<ContentId>,
    context: &str,
    errors: &mut Vec<String>,
) -> BTreeMap<ContentId, u32> {
    let mut result = BTreeMap::new();
    for value in values {
        let Some(good) = parse_id(&value.good, context, errors) else {
            continue;
        };
        if !goods.contains(&good) {
            errors.push(format!("{context}: unknown priority good {good}"));
        }
        if good.as_str() == ENERGY_ID {
            errors.push(format!(
                "{context}: core:energy import priority is obsolete because Energy is not ordinarily tradable"
            ));
            continue;
        }
        if value.percent == 0 || value.percent > 10_000 {
            errors.push(format!("{context}: priority percent must be 1..=10000"));
        }
        if result.insert(good.clone(), value.percent).is_some() {
            errors.push(format!("{context}: duplicate priority good {good}"));
        }
    }
    result
}

fn compile_policy_defaults(
    config: &EconomyConfigSource,
    goods: &BTreeSet<ContentId>,
    errors: &mut Vec<String>,
) -> MarketPolicy {
    MarketPolicy {
        pricing_mode: pricing_mode(config.pricing_mode),
        producer_margin_percent: config.producer_margin_percent,
        operating_reserve_ticks: config.operating_reserve_ticks,
        import_priorities: compile_priorities(
            config.import_priorities.clone(),
            goods,
            "economy_config.ron:import_priorities",
            errors,
        ),
        liquidation_threshold_percent: config.liquidation_threshold_percent,
        liquidation_discount_percent: config.liquidation_discount_percent,
        default_target: config.default_target,
    }
}

fn merge_policy(
    default: &MarketPolicy,
    source: MarketPolicyOverrideSource,
    goods: &BTreeSet<ContentId>,
    context: &str,
    errors: &mut Vec<String>,
) -> MarketPolicy {
    let policy = MarketPolicy {
        pricing_mode: source
            .pricing_mode
            .map(pricing_mode)
            .unwrap_or(default.pricing_mode),
        producer_margin_percent: source
            .producer_margin_percent
            .unwrap_or(default.producer_margin_percent),
        operating_reserve_ticks: source
            .operating_reserve_ticks
            .unwrap_or(default.operating_reserve_ticks),
        import_priorities: source
            .import_priorities
            .map(|value| {
                compile_priorities(
                    value,
                    goods,
                    &format!("{context}:policy:import_priorities"),
                    errors,
                )
            })
            .unwrap_or_else(|| default.import_priorities.clone()),
        liquidation_threshold_percent: source
            .liquidation_threshold_percent
            .unwrap_or(default.liquidation_threshold_percent),
        liquidation_discount_percent: source
            .liquidation_discount_percent
            .unwrap_or(default.liquidation_discount_percent),
        default_target: source.default_target.unwrap_or(default.default_target),
    };
    if policy.validate().is_err() {
        errors.push(format!("{context}: invalid market policy"));
    }
    policy
}

fn compile_brownouts(
    source: &BrownoutConfigSource,
    goods: &BTreeSet<ContentId>,
    errors: &mut Vec<String>,
) -> BrownoutConfig {
    let survival_goods = source
        .survival_goods
        .iter()
        .filter_map(|raw| {
            let id = parse_id(raw, "economy_config.ron:brownouts:survival_goods", errors)?;
            if !goods.contains(&id) {
                errors.push(format!(
                    "economy_config.ron:brownouts:survival_goods: unknown good {id}"
                ));
            }
            Some(id)
        })
        .collect();
    let config = BrownoutConfig {
        throttled_entry_ticks: source.throttled_entry_ticks,
        emergency_entry_ticks: source.emergency_entry_ticks,
        starvation_entry_ticks: source.starvation_entry_ticks,
        throttled_recovery_ticks: source.throttled_recovery_ticks,
        emergency_recovery_ticks: source.emergency_recovery_ticks,
        starvation_recovery_ticks: source.starvation_recovery_ticks,
        minimum_stage_ticks: source.minimum_stage_ticks,
        throttled_throughput_percent: source.throttled_throughput_percent,
        emergency_throughput_percent: source.emergency_throughput_percent,
        starvation_throughput_percent: source.starvation_throughput_percent,
        survival_goods,
    };
    if config.validate().is_err() {
        errors.push("economy_config.ron:brownouts: invalid threshold ordering, throughput, or survival goods".into());
    }
    config
}

fn compile_population_config(
    source: &PopulationConfigSource,
    goods: &BTreeSet<ContentId>,
    errors: &mut Vec<String>,
) -> PopulationConfig {
    if source.sufficiency_window > MAX_POPULATION_SUFFICIENCY_WINDOW_TICKS {
        errors.push(format!(
            "economy_config.ron:population:sufficiency_window: must be at most {MAX_POPULATION_SUFFICIENCY_WINDOW_TICKS} ticks"
        ));
    }
    let essential_goods = source
        .essential_goods
        .iter()
        .filter_map(|raw| {
            let id = parse_id(raw, "economy_config.ron:population:essential_goods", errors)?;
            if !goods.contains(&id) {
                errors.push(format!(
                    "economy_config.ron:population:essential_goods: unknown good {id}"
                ));
            }
            Some(id)
        })
        .collect::<BTreeSet<_>>();
    let mut tertiary_demand_per_thousand = BTreeMap::new();
    for demand in &source.tertiary_demand {
        let Some(good) = parse_id(
            &demand.good,
            "economy_config.ron:population:tertiary_demand",
            errors,
        ) else {
            continue;
        };
        if !goods.contains(&good) {
            errors.push(format!(
                "economy_config.ron:population:tertiary_demand: unknown good {good}"
            ));
        }
        if demand.units_per_thousand == 0
            || tertiary_demand_per_thousand
                .insert(good.clone(), demand.units_per_thousand)
                .is_some()
        {
            errors.push(
                "economy_config.ron:population:tertiary_demand: goods must be unique with positive rates"
                    .into(),
            );
        }
    }
    let config = PopulationConfig {
        static_population: source.static_population,
        sufficiency_window: source.sufficiency_window,
        growth_sufficiency_percent: source.growth_sufficiency_percent,
        essential_goods,
        tertiary_demand_per_thousand,
        decline_per_thousand: source.decline_per_thousand,
        growth_per_thousand: source.growth_per_thousand,
        logistic_scale: source.logistic_scale,
        minimum_cap: source.minimum_cap,
        maximum_cap: source.maximum_cap,
        tier_thresholds: source.tier_thresholds.clone(),
    };
    if validate_population_config(&config).is_err() {
        errors.push("economy_config.ron:population: invalid goods, window, gate, rates, logistic scale, cap bounds, or tier thresholds".into());
    }
    config
}

fn compile_investment_shapes(
    sources: &[InvestmentShapeSource],
    population: &PopulationConfig,
    errors: &mut Vec<String>,
) -> BTreeMap<InvestmentKind, InvestmentShape> {
    let mut result = BTreeMap::new();
    for source in sources {
        let kind = InvestmentKind::from(source.kind);
        if result
            .insert(
                kind,
                InvestmentShape {
                    enabled: source.enabled,
                    base_cost: Energy(source.base_cost),
                    cost_growth_percent: source.cost_growth_percent,
                    maximum_level: source.maximum_level,
                    cooldown_ticks: source.cooldown_ticks,
                    effect_per_level: source.effect_per_level,
                },
            )
            .is_some()
        {
            errors.push(format!(
                "economy_config.ron:investments: duplicate {kind:?}"
            ));
        }
    }
    if validate_investment_shapes(&result, population).is_err() {
        errors.push("economy_config.ron:investments: all four shapes require valid costs, curves, levels, cooldowns, and kind-safe maximum effects".into());
    }
    result
}

fn compile_investment_policy(
    sources: &[InvestmentAllocationSource],
    context: &str,
    errors: &mut Vec<String>,
) -> InvestmentPolicy {
    let mut allocation_percent = BTreeMap::new();
    let mut total = 0_u32;
    for source in sources {
        let kind = InvestmentKind::from(source.kind);
        total = total.saturating_add(source.percent);
        if source.percent > 100 || allocation_percent.insert(kind, source.percent).is_some() {
            errors.push(format!(
                "{context}: allocations must be unique percentages in 0..=100"
            ));
        }
    }
    if total > 100 {
        errors.push(format!("{context}: allocation total cannot exceed 100"));
    }
    InvestmentPolicy { allocation_percent }
}

fn validate_config(config: &EconomyConfigSource, policy: &MarketPolicy, errors: &mut Vec<String>) {
    if policy.validate().is_err()
        || config.reservation_ttl == 0
        || config.life_support_burn_per_capita < 0
        || config.source_output_percent > 1_000
    {
        errors.push("economy_config.ron: invalid policy or physical configuration".into());
    }
}

fn validate_recipe_layers(
    id: &ContentId,
    layer: RecipeLayer,
    inputs: &[GoodAmount],
    outputs: &[RecipeOutput],
    categories: &BTreeMap<ContentId, GoodCategory>,
    errors: &mut Vec<String>,
) {
    match layer {
        RecipeLayer::Primary => {
            if !inputs
                .iter()
                .any(|a| categories.get(&a.good) == Some(&GoodCategory::Raw))
            {
                errors.push(format!(
                    "recipes.ron:{id}: primary recipe needs a raw input"
                ));
            }
            if !outputs
                .iter()
                .any(|a| categories.get(&a.good) == Some(&GoodCategory::Primary))
            {
                errors.push(format!(
                    "recipes.ron:{id}: primary recipe needs a primary output"
                ));
            }
        }
        RecipeLayer::Secondary => {
            if !inputs
                .iter()
                .any(|a| categories.get(&a.good) == Some(&GoodCategory::Primary))
                || !inputs
                    .iter()
                    .any(|a| categories.get(&a.good) == Some(&GoodCategory::Raw))
            {
                errors.push(format!(
                    "recipes.ron:{id}: secondary recipe needs primary and raw inputs"
                ));
            }
            if !outputs
                .iter()
                .any(|a| categories.get(&a.good) == Some(&GoodCategory::Secondary))
            {
                errors.push(format!(
                    "recipes.ron:{id}: secondary recipe needs a secondary output"
                ));
            }
        }
        RecipeLayer::Tertiary if !outputs.is_empty() => errors.push(format!(
            "recipes.ron:{id}: tertiary recipe cannot produce goods"
        )),
        RecipeLayer::Tertiary => {}
    }
}

fn amounts_to_map(
    values: Vec<AmountSource>,
    goods: &BTreeSet<ContentId>,
    label: &str,
    errors: &mut Vec<String>,
) -> BTreeMap<ContentId, u64> {
    let mut result = BTreeMap::new();
    for value in values {
        let Some(good) = parse_id(&value.good, &format!("economy.ron:{label}"), errors) else {
            continue;
        };
        if !goods.contains(&good) {
            errors.push(format!("economy.ron:{label}: unknown good {good}"));
        }
        if result
            .insert(good.clone(), u64::from(value.quantity))
            .is_some()
        {
            errors.push(format!("economy.ron:{label}: duplicate good {good}"));
        }
    }
    result
}

fn valid_trader_numbers(tank: i64, tank_capacity: i64, cargo: u32, speed: f64, burn: i64) -> bool {
    tank >= 0
        && tank <= tank_capacity
        && tank_capacity > 0
        && cargo > 0
        && speed.is_finite()
        && speed > 0.0
        && burn >= 0
}

fn compile_traders(
    source: TraderConfigSource,
    systems: &[SystemDefinition],
    errors: &mut Vec<String>,
) -> (Vec<TraderDefinition>, TradeNetworkAccess, FleetDynamics) {
    let player_trade_network_access = source.player.trade_network_access.into();
    let system_ids = systems
        .iter()
        .map(|system| system.id.clone())
        .collect::<BTreeSet<_>>();
    let mut result = Vec::new();
    let player_id = parse_id(&source.player.id, "traders.ron:player", errors);
    let player_system = parse_id(
        &source.player.starting_system,
        "traders.ron:player:starting_system",
        errors,
    );
    if let (Some(id), Some(system)) = (player_id, player_system) {
        if !system_ids.contains(&system) {
            errors.push(format!(
                "traders.ron:player: unknown starting system {system}"
            ));
        }
        if !valid_trader_numbers(
            source.player.energy_tank,
            source.player.energy_tank_capacity,
            source.player.cargo_capacity,
            source.player.speed,
            source.player.travel_burn_per_distance,
        ) || source.player.bulk_energy_capacity < 0
        {
            errors.push("traders.ron:player: invalid numeric value".into());
        }
        result.push(TraderDefinition {
            id,
            name: source.player.name,
            system,
            archetype: None,
            energy_tank: Energy(source.player.energy_tank),
            energy_tank_capacity: Energy(source.player.energy_tank_capacity),
            bulk_energy_capacity: Energy(source.player.bulk_energy_capacity),
            cargo_capacity: source.player.cargo_capacity,
            speed: source.player.speed,
            travel_burn_per_distance: Energy(source.player.travel_burn_per_distance),
            refuel_policy: source.player.refuel_policy.into(),
            player: true,
        });
    }

    let total_initial = source
        .npcs
        .archetypes
        .iter()
        .try_fold(0_usize, |sum, archetype| {
            sum.checked_add(archetype.initial_count)
        })
        .unwrap_or_else(|| {
            errors.push("traders.ron:npcs: initial count total overflows usize".into());
            usize::MAX
        });
    let total_archetype_maximum = source
        .npcs
        .archetypes
        .iter()
        .try_fold(0_usize, |sum, archetype| {
            sum.checked_add(archetype.maximum_count)
        })
        .unwrap_or_else(|| {
            errors.push("traders.ron:npcs: maximum count total overflows usize".into());
            usize::MAX
        });
    let fleet_mode = match source.npcs.mode {
        NpcFleetModeSource::Fixed => FleetMode::Fixed {
            count: total_initial,
        },
        NpcFleetModeSource::Dynamic => FleetMode::Dynamic {
            initial_count: total_initial,
            opportunity_threshold: source.npcs.dynamic.opportunity_threshold,
            opportunity_window: source.npcs.dynamic.opportunity_window,
            spawn_cooldown_ticks: source.npcs.dynamic.spawn_cooldown_ticks,
            retirement_window: source.npcs.dynamic.retirement_window,
            retirement_threshold: source.npcs.dynamic.retirement_threshold,
            maximum_count: source.npcs.dynamic.maximum_count,
        },
    };
    if source.npcs.dynamic.opportunity_threshold == 0
        || source.npcs.dynamic.opportunity_window == 0
        || source.npcs.dynamic.opportunity_window > 10_000
        || source.npcs.dynamic.spawn_cooldown_ticks == 0
        || source.npcs.dynamic.retirement_window == 0
        || source.npcs.dynamic.retirement_window > 10_000
        || source.npcs.dynamic.maximum_count == 0
        || source.npcs.dynamic.maximum_count < total_initial
        || source.npcs.dynamic.maximum_count > total_archetype_maximum
        || source.npcs.dynamic.maximum_count > systems.len()
    {
        errors.push("traders.ron:npcs:dynamic: thresholds/windows must be positive, windows must be at most 10000 ticks, and total maximum_count must cover initial count, respect archetype caps, and not exceed systems".into());
    }

    let mut archetypes = BTreeMap::new();
    let mut prefixes = Vec::<String>::new();
    for source_archetype in source.npcs.archetypes {
        let context = format!("traders.ron:npcs:archetype:{}", source_archetype.id);
        let Some(archetype_id) = parse_id(&source_archetype.id, &context, errors) else {
            continue;
        };
        if source_archetype.initial_count > source_archetype.maximum_count
            || source_archetype.maximum_count == 0
            || source_archetype.initial_distribution.len() != source_archetype.initial_count
        {
            errors.push(format!(
                "{context}: initial distribution/count must match and initial_count must not exceed a positive maximum_count"
            ));
        }
        if source_archetype.energy_tank <= 0
            || !valid_trader_numbers(
                source_archetype.energy_tank,
                source_archetype.energy_tank_capacity,
                source_archetype.cargo_capacity,
                source_archetype.speed,
                source_archetype.travel_burn_per_distance,
            )
            || source_archetype.travel_burn_per_distance == 0
            || source_archetype.bulk_energy_capacity < 0
        {
            errors.push(format!("{context}: invalid physical numeric value"));
        }
        if source_archetype.name_prefix.trim().is_empty()
            || ContentId::new(format!("{}_dynamic_00000001", source_archetype.id_prefix)).is_err()
        {
            errors.push(format!(
                "{context}: prefixes must form valid stable trader IDs and names"
            ));
        }
        if prefixes.iter().any(|prefix| {
            prefix == &source_archetype.id_prefix
                || prefix.starts_with(&format!("{}_", source_archetype.id_prefix))
                || source_archetype
                    .id_prefix
                    .starts_with(&format!("{prefix}_"))
        }) {
            errors.push(format!(
                "{context}: id_prefix collides with another archetype prefix"
            ));
        }
        prefixes.push(source_archetype.id_prefix.clone());

        for (index, raw_system) in source_archetype.initial_distribution.iter().enumerate() {
            let Some(system) = parse_id(
                raw_system,
                &format!("{context}:initial_distribution"),
                errors,
            ) else {
                continue;
            };
            if !system_ids.contains(&system) {
                errors.push(format!("{context}: unknown initial system {system}"));
            }
            let raw_id = format!("{}_{:02}", source_archetype.id_prefix, index + 1);
            let Some(id) = parse_id(&raw_id, &format!("{context}:id_prefix"), errors) else {
                continue;
            };
            result.push(TraderDefinition {
                id,
                name: format!("{} {:02}", source_archetype.name_prefix, index + 1),
                system,
                archetype: Some(archetype_id.clone()),
                energy_tank: Energy(source_archetype.energy_tank),
                energy_tank_capacity: Energy(source_archetype.energy_tank_capacity),
                bulk_energy_capacity: Energy(source_archetype.bulk_energy_capacity),
                cargo_capacity: source_archetype.cargo_capacity,
                speed: source_archetype.speed,
                travel_burn_per_distance: Energy(source_archetype.travel_burn_per_distance),
                refuel_policy: source_archetype.refuel_policy.into(),
                player: false,
            });
        }
        let archetype = FleetArchetype {
            id: archetype_id.clone(),
            id_prefix: source_archetype.id_prefix,
            name_prefix: source_archetype.name_prefix,
            initial_count: source_archetype.initial_count,
            maximum_count: source_archetype.maximum_count,
            starting_tank: Energy(source_archetype.energy_tank),
            energy_tank_capacity: Energy(source_archetype.energy_tank_capacity),
            bulk_energy_capacity: Energy(source_archetype.bulk_energy_capacity),
            cargo_capacity: source_archetype.cargo_capacity,
            speed: source_archetype.speed,
            travel_burn_per_distance: Energy(source_archetype.travel_burn_per_distance),
            refuel_policy: source_archetype.refuel_policy.into(),
        };
        if archetypes.insert(archetype_id.clone(), archetype).is_some() {
            errors.push(format!("{context}: duplicate archetype id {archetype_id}"));
        }
    }
    if archetypes.is_empty() {
        errors.push("traders.ron:npcs: at least one archetype is required".into());
    }
    if !archetypes
        .values()
        .any(|archetype| archetype.bulk_energy_capacity > Energy::ZERO)
    {
        errors.push(
            "traders.ron:npcs: at least one bulk Energy capable archetype is required".into(),
        );
    }
    if matches!(&fleet_mode, FleetMode::Dynamic { .. }) {
        for archetype in archetypes.values() {
            let generated_namespace = format!("{}_dynamic_", archetype.id_prefix);
            if let Some(collision) = result
                .iter()
                .find(|trader| trader.id.as_str().starts_with(&generated_namespace))
            {
                errors.push(format!(
                    "traders.ron:npcs:archetype:{}:id_prefix: generated trader namespace {generated_namespace} collides with existing trader {}",
                    archetype.id, collision.id
                ));
            }
        }
    }
    if result
        .iter()
        .map(|trader| &trader.id)
        .collect::<BTreeSet<_>>()
        .len()
        != result.len()
    {
        errors.push("traders.ron: trader IDs must be unique".into());
    }
    (
        result,
        player_trade_network_access,
        FleetDynamics {
            mode: Some(fleet_mode),
            archetypes,
            ..FleetDynamics::default()
        },
    )
}

fn system_burn(
    system: &SystemDefinition,
    recipes: &BTreeMap<ContentId, &RecipeDefinition>,
    config: &EconomyConfig,
) -> Option<i64> {
    let life = config
        .life_support_burn_per_capita
        .0
        .checked_mul(i64::try_from(system.population).ok()?)?;
    let source = system.sources.iter().try_fold(0_i64, |sum, value| {
        let output =
            scaled_source_output(value.quantity_per_tick, config.source_output_percent).ok()?;
        value
            .extraction_energy
            .0
            .checked_mul(i64::from(output))?
            .checked_add(sum)
    })?;
    let recipe = system.recipes.iter().try_fold(0_i64, |sum, id| {
        sum.checked_add(recipes.get(id)?.operating_energy.0)
    })?;
    life.checked_add(source)?.checked_add(recipe)
}

fn validate_roles_and_anticorrelation(
    systems: &[SystemDefinition],
    recipes: &[RecipeDefinition],
    config: &EconomyConfig,
    errors: &mut Vec<String>,
) {
    let recipes = recipes
        .iter()
        .map(|recipe| (recipe.id.clone(), recipe))
        .collect::<BTreeMap<_, _>>();
    let mut exporters = 0;
    let mut importers = 0;
    let mut knife = 0;
    let mut source_generation = Vec::new();
    let mut other_generation = Vec::new();
    for system in systems {
        let Some(burn) = system_burn(system, &recipes, config) else {
            errors.push(format!(
                "economy.ron:{}: burn arithmetic overflow",
                system.id
            ));
            continue;
        };
        let net = system.energy_output_per_tick.0 - burn;
        if net > 0 {
            exporters += 1;
        }
        if net < 0 {
            importers += 1;
        }
        if net.abs() <= (burn / 10).max(1) {
            knife += 1;
        }
        if system.sources.is_empty() {
            other_generation.push(system.energy_output_per_tick.0);
        } else {
            source_generation.push(system.energy_output_per_tick.0);
        }
    }
    if exporters == 0 || importers == 0 || knife == 0 {
        errors.push(format!("economy.ron: authored energy roles require exporter, importer, and knife-edge systems (found {exporters}/{importers}/{knife})"));
    }
    if source_generation.is_empty()
        || other_generation.is_empty()
        || source_generation.iter().sum::<i64>()
            * i64::try_from(other_generation.len()).unwrap_or(0)
            >= other_generation.iter().sum::<i64>()
                * i64::try_from(source_generation.len()).unwrap_or(0)
    {
        errors.push(
            "economy.ron: solar generation must be anti-correlated with raw-resource sources"
                .into(),
        );
    }
}

fn validate_archetype_route_capacity(
    fleet: &FleetDynamics,
    systems: &[SystemDefinition],
    graph: &SystemGraph,
    errors: &mut Vec<String>,
) {
    for archetype in fleet.archetypes.values() {
        let can_cover_adjacent_route = systems.iter().any(|system| {
            graph.neighbors(&system.id).iter().any(|(_, distance)| {
                game_core::travel_energy(*distance, archetype.travel_burn_per_distance)
                    .is_ok_and(|burn| burn <= archetype.energy_tank_capacity)
            })
        });
        if !can_cover_adjacent_route {
            errors.push(format!(
                "traders.ron:npcs:archetype:{}: tank capacity cannot cover any graph-adjacent route",
                archetype.id
            ));
        }
    }
}

fn compute_protected_budgets(
    systems: &mut [SystemDefinition],
    goods: &[GoodDefinition],
    traders: &[TraderDefinition],
    fleet: &FleetDynamics,
    graph: &SystemGraph,
    errors: &mut Vec<String>,
) {
    let bootstrap_costs = goods
        .iter()
        .map(|good| good.bootstrap_cost)
        .collect::<Vec<_>>();
    let mut capabilities = traders
        .iter()
        .filter(|trader| trader.player)
        .map(|trader| LiquidationTraderCapability {
            cargo_capacity: trader.cargo_capacity,
            energy_tank_capacity: trader.energy_tank_capacity,
            travel_burn_per_distance: trader.travel_burn_per_distance,
        })
        .collect::<Vec<_>>();
    capabilities.extend(
        fleet
            .archetypes
            .values()
            .map(FleetArchetype::liquidation_capability),
    );
    for system in systems.iter_mut() {
        match compute_protected_liquidation_budget(
            graph,
            &system.id,
            &system.policy,
            &bootstrap_costs,
            &capabilities,
        ) {
            Ok(budget) => system.protected_liquidation_budget = budget,
            Err(error) => errors.push(format!(
                "economy.ron:{}: protected liquidation budget is infeasible: {error}",
                system.id
            )),
        }
    }
}

fn validate_bootstrap(
    systems: &[SystemDefinition],
    recipes: &[RecipeDefinition],
    traders: &[TraderDefinition],
    config: &EconomyConfig,
    graph: &SystemGraph,
    warnings: &mut Vec<ContentWarning>,
    errors: &mut Vec<String>,
) {
    let recipe_map = recipes
        .iter()
        .map(|recipe| (recipe.id.clone(), recipe))
        .collect::<BTreeMap<_, _>>();
    let burn = systems
        .iter()
        .filter_map(|system| {
            system_burn(system, &recipe_map, config).map(|value| (system.id.clone(), value))
        })
        .collect::<BTreeMap<_, _>>();
    let exporters = systems
        .iter()
        .filter(|system| {
            system.energy_output_per_tick.0 > burn.get(&system.id).copied().unwrap_or(i64::MAX)
        })
        .collect::<Vec<_>>();
    let energy_id = ContentId::new(ENERGY_ID).expect("constant id");
    for importer in systems.iter().filter(|system| {
        system.energy_output_per_tick.0 < burn.get(&system.id).copied().unwrap_or(0)
    }) {
        let required_burn = burn[&importer.id] - importer.energy_output_per_tick.0;
        let starting = importer.inventory.get(&energy_id).copied().unwrap_or(0);
        let runway = starting / u64::try_from(required_burn).unwrap_or(u64::MAX);
        let delivery_quantity = u64::try_from(required_burn).unwrap_or(u64::MAX);
        let best = exporters
            .iter()
            .flat_map(|exporter| {
                let burn = &burn;
                let energy_id = &energy_id;
                traders.iter().filter_map(move |trader| {
                    let (approach_route, approach) =
                        graph.shortest_path(&trader.system, &exporter.id)?;
                    let (delivery_route, delivery) =
                        graph.shortest_path(&exporter.id, &importer.id)?;
                    let deadhead_burn = route_travel_energy(
                        graph,
                        &approach_route,
                        trader.travel_burn_per_distance,
                    )
                    .ok()?;
                    let loaded_burn = route_travel_energy(
                        graph,
                        &delivery_route,
                        trader.travel_burn_per_distance,
                    )
                    .ok()?;
                    let (recovery_route, _) = graph.shortest_path(&importer.id, &exporter.id)?;
                    let recovery_burn = route_travel_energy(
                        graph,
                        &recovery_route,
                        trader.travel_burn_per_distance,
                    )
                    .ok()?;
                    let required_tank = deadhead_burn.checked_add(loaded_burn).ok()?;
                    let exporter_burn = *burn.get(&exporter.id)?;
                    let exporter_stock = exporter.inventory.get(energy_id).copied().unwrap_or(0);
                    let exporter_operating = exporter_burn
                        .checked_mul(i64::from(exporter.policy.operating_reserve_ticks))?;
                    let protected = exporter_operating
                        .checked_add(exporter.protected_liquidation_budget.0)?
                        .checked_add(exporter.energy_logistics.export_reserve.0)?;
                    let exporter_available =
                        exporter_stock.saturating_sub(u64::try_from(protected).ok()?);
                    if exporter_available < delivery_quantity
                        || trader.bulk_energy_capacity.0 < i64::try_from(delivery_quantity).ok()?
                        || required_tank > trader.energy_tank
                        || recovery_burn > trader.energy_tank_capacity
                    {
                        return None;
                    }
                    let ticks = ticks_for_distance(approach, trader.speed)
                        .checked_add(ticks_for_distance(delivery, trader.speed))?
                        .checked_add(1)?;
                    Some((ticks, exporter.id.clone(), trader.id.clone()))
                })
            })
            .min_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| a.1.cmp(&b.1))
                    .then_with(|| a.2.cmp(&b.2))
            });
        let Some((required_ticks, exporter, trader)) = best else {
            let detail = "no exporter/trader pair has protected surplus, bulk capacity, loaded/deadhead fuel, recovery capacity, and one-tick delivery capacity".to_string();
            if importer.bootstrap_risk_acknowledged {
                warnings.push(ContentWarning::BootstrapDeliveryAcknowledged {
                    source: "economy.ron",
                    system: importer.id.clone(),
                    detail,
                });
            } else {
                errors.push(format!(
                    "economy.ron:{}: {detail}; set acknowledge_bootstrap_risk: true to accept deliberately precarious content",
                    importer.id
                ));
            }
            continue;
        };
        if runway <= u64::from(required_ticks) {
            if importer.bootstrap_risk_acknowledged {
                warnings.push(ContentWarning::BootstrapRunwayAcknowledged {
                    source: "economy.ron",
                    system: importer.id.clone(),
                    starting_energy: starting,
                    required_burn_per_tick: required_burn,
                    runway_ticks: runway,
                    required_ticks,
                    exporter,
                    trader,
                });
            } else {
                errors.push(format!("economy.ron:{}: bootstrap runway {runway} ticks is not strictly greater than required {required_ticks} ticks via exporter {exporter} and trader {trader}; set acknowledge_bootstrap_risk: true to accept deliberately precarious content", importer.id));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content")
    }

    #[test]
    fn repository_content_loads_with_structural_roles() {
        let loaded =
            load_directory_with_warnings(root()).expect("repository content should validate");
        assert_eq!(loaded.definition.systems.len(), 20);
        assert_eq!(loaded.definition.goods.len(), 11);
        assert_eq!(loaded.definition.recipes.len(), 9);
        let energy = loaded
            .definition
            .goods
            .iter()
            .find(|good| good.id.as_str() == ENERGY_ID)
            .unwrap();
        assert_eq!(energy.category, GoodCategory::Energy);
        assert_eq!(energy.bootstrap_cost, Energy(1));
        assert!(loaded.definition.systems.iter().all(|system| {
            system.inventory.contains_key(&energy.id)
                && system.protected_liquidation_budget.0 > 0
                && system.policy.pricing_mode == PricingMode::CostAware
                && !system.policy.import_priorities.contains_key(&energy.id)
        }));
        assert!(
            loaded
                .definition
                .traders
                .iter()
                .all(|trader| { trader.refuel_policy == RefuelPolicy::DepositAndWithdraw })
        );
        assert_eq!(
            loaded.definition.player_trade_network_access,
            TradeNetworkAccess::Offline
        );
        assert!(matches!(
            loaded.definition.fleet.mode,
            Some(FleetMode::Dynamic {
                initial_count: 9,
                maximum_count: 20,
                ..
            })
        ));
        assert_eq!(loaded.definition.fleet.archetypes.len(), 2);
        let logistics = loaded.definition.economy.energy_logistics;
        assert_eq!(
            logistics.carrier_fee_bps,
            game_core::CarrierFeeSchedule {
                normal: 50,
                throttled: 100,
                emergency: 200,
                starvation: 300,
            }
        );
        assert_eq!(logistics.max_allocation_bps, 1_000);
        assert_eq!(logistics.curtailment_projection_window, 20);
        assert_eq!(logistics.export_reserve, Energy::ZERO);
        assert_eq!(logistics.authored_export_base, Energy::ZERO);
        assert_eq!(logistics.settlement_timeout_ticks, 20);
        let system_15 = loaded
            .definition
            .systems
            .iter()
            .find(|system| system.id.as_str() == "frontier:system_15")
            .unwrap();
        assert_eq!(system_15.inventory[&energy.id], 5_000);
        assert_eq!(
            system_15.energy_logistics.authored_export_base,
            Energy(3_200)
        );
        let system_14 = loaded
            .definition
            .systems
            .iter()
            .find(|system| system.id.as_str() == "frontier:system_14")
            .unwrap();
        assert_eq!(system_14.targets[&energy.id], 5_000);
        let player = loaded
            .definition
            .traders
            .iter()
            .find(|trader| trader.player)
            .unwrap();
        assert_eq!(player.bulk_energy_capacity, Energy(4_000));
        let general = &loaded.definition.fleet.archetypes
            [&ContentId::new("frontier:general_freighter").unwrap()];
        assert_eq!((general.initial_count, general.maximum_count), (5, 10));
        assert_eq!(
            (general.starting_tank, general.energy_tank_capacity),
            (Energy(1_000), Energy(1_500))
        );
        assert_eq!(general.bulk_energy_capacity, Energy::ZERO);
        assert_eq!(general.cargo_capacity, 300);
        assert_eq!(
            (general.speed, general.travel_burn_per_distance),
            (8.0, Energy(1))
        );
        let bulk = &loaded.definition.fleet.archetypes
            [&ContentId::new("frontier:bulk_energy_hauler").unwrap()];
        assert_eq!((bulk.initial_count, bulk.maximum_count), (4, 10));
        assert_eq!(
            (bulk.starting_tank, bulk.energy_tank_capacity),
            (Energy(1_000), Energy(1_500))
        );
        assert_eq!(bulk.bulk_energy_capacity, Energy(4_000));
        assert_eq!(bulk.cargo_capacity, 100);
        assert_eq!(
            (bulk.speed, bulk.travel_burn_per_distance),
            (8.0, Energy(1))
        );
        assert_eq!(
            loaded
                .definition
                .traders
                .iter()
                .filter(|trader| !trader.player)
                .count(),
            9
        );
        assert!(loaded.definition.traders.iter().any(|trader| {
            trader.archetype.as_ref() == Some(&bulk.id)
                && trader.system.as_str() == "frontier:system_15"
        }));
        assert!(!loaded.definition.economy.population.static_population);
        assert_eq!(loaded.definition.economy.investments.len(), 4);
        assert!(
            loaded
                .definition
                .economy
                .investments
                .values()
                .all(|shape| shape.enabled
                    && shape.maximum_level > 0
                    && shape.effect_per_level > 0)
        );
        assert!(loaded.definition.systems.iter().all(|system| {
            system
                .investment_policy
                .allocation_percent
                .values()
                .sum::<u32>()
                == 100
        }));
        assert_eq!(
            loaded
                .definition
                .systems
                .iter()
                .filter(|system| system.seasonal_generation.amplitude_percent > 0)
                .count(),
            3,
            "repository tuning keeps seasons focused on exactly three systems"
        );
        assert!(loaded.definition.systems.iter().all(|system| {
            system.seasonal_generation.base_output == system.energy_output_per_tick
                && system.population_state.current == system.population
        }));
        assert!(matches!(
            loaded.definition.systems[0].governance.authority,
            MarketAuthority::Player(ref player) if player.as_str() == "frontier:player"
        ));
        assert_eq!(loaded.encyclopedia.len(), 4);
        let encyclopedia_text = loaded
            .encyclopedia
            .iter()
            .flat_map(|section| &section.articles)
            .flat_map(|article| &article.paragraphs)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(encyclopedia_text.contains("A brownout describes"));
        assert!(encyclopedia_text.contains("A baseline cost is not a market price"));
        assert!(!encyclopedia_text.contains("bootstrap embodied-energy"));
        assert!(!encyclopedia_text.contains("The current frontier contains"));
        assert!(!encyclopedia_text.contains("CommitTrade"));
        assert!(loaded.warnings.iter().all(|warning| matches!(
            warning,
            ContentWarning::BootstrapRunwayAcknowledged {
                source: "economy.ron",
                ..
            } | ContentWarning::BootstrapDeliveryAcknowledged {
                source: "economy.ron",
                ..
            }
        )));
    }

    #[test]
    fn removed_energy_import_priorities_report_exact_source_contexts() {
        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let recipes = load(root().join("recipes.ron")).unwrap();
        let economy = load(root().join("economy.ron")).unwrap();
        let mut config: EconomyConfigSource = load(root().join("economy_config.ron")).unwrap();
        config.import_priorities.push(PrioritySource {
            good: ENERGY_ID.into(),
            percent: 200,
        });
        let traders = load(root().join("traders.ron")).unwrap();
        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(
                "economy_config.ron:import_priorities: core:energy import priority is obsolete"
            ),
            "{error}"
        );

        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let recipes = load(root().join("recipes.ron")).unwrap();
        let mut economy: EconomySource = load(root().join("economy.ron")).unwrap();
        economy.markets[0].policy.import_priorities = Some(vec![PrioritySource {
            good: ENERGY_ID.into(),
            percent: 130,
        }]);
        let config = load(root().join("economy_config.ron")).unwrap();
        let traders = load(root().join("traders.ron")).unwrap();
        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("economy.ron:frontier:system_01:policy:import_priorities: core:energy import priority is obsolete"),
            "{error}"
        );
    }

    #[test]
    fn malformed_world_dynamics_report_source_contexts() {
        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let recipes = load(root().join("recipes.ron")).unwrap();
        let mut economy: EconomySource = load(root().join("economy.ron")).unwrap();
        let mut config: EconomyConfigSource = load(root().join("economy_config.ron")).unwrap();
        let mut traders: TraderConfigSource = load(root().join("traders.ron")).unwrap();

        config.brownouts.emergency_entry_ticks = config.brownouts.throttled_entry_ticks;
        config.population.growth_per_thousand = config.population.decline_per_thousand;
        config.investments.pop();
        economy.markets[0].seasonal.period_ticks = 0;
        economy.markets[0].governor = Some("frontier:missing_player".into());
        traders.npcs.mode = NpcFleetModeSource::Dynamic;
        traders.npcs.dynamic.maximum_count = traders
            .npcs
            .archetypes
            .iter()
            .map(|archetype| archetype.initial_count)
            .sum::<usize>()
            - 1;

        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        for context in [
            "economy_config.ron:brownouts",
            "economy_config.ron:population",
            "economy_config.ron:investments",
            "economy.ron:frontier:system_01:seasonal",
            "economy.ron:frontier:system_01:governor",
            "traders.ron:npcs:dynamic",
        ] {
            assert!(error.contains(context), "missing {context} in {error}");
        }
    }

    #[test]
    fn malformed_energy_logistics_policy_reports_exact_source_contexts() {
        fn compile_with(mutator: fn(&mut EconomyConfigSource)) -> String {
            let systems = load(root().join("systems.ron")).unwrap();
            let goods = load(root().join("goods.ron")).unwrap();
            let recipes = load(root().join("recipes.ron")).unwrap();
            let economy = load(root().join("economy.ron")).unwrap();
            let mut config: EconomyConfigSource = load(root().join("economy_config.ron")).unwrap();
            mutator(&mut config);
            let traders = load(root().join("traders.ron")).unwrap();
            compile(systems, goods, recipes, economy, config, traders)
                .unwrap_err()
                .to_string()
        }

        type ConfigMutation = fn(&mut EconomyConfigSource);
        let cases: [(ConfigMutation, &str); 4] = [
            (
                |config| config.energy_logistics.carrier_fee_bps[1] = 50,
                "economy_config.ron:energy_logistics:carrier_fee_bps",
            ),
            (
                |config| config.energy_logistics.max_allocation_bps = 0,
                "economy_config.ron:energy_logistics:max_allocation_bps",
            ),
            (
                |config| config.energy_logistics.curtailment_projection_window = 0,
                "economy_config.ron:energy_logistics:curtailment_projection_window",
            ),
            (
                |config| config.energy_logistics.settlement_timeout_ticks = 0,
                "economy_config.ron:energy_logistics:settlement_timeout_ticks",
            ),
        ];
        for (mutator, context) in cases {
            let error = compile_with(mutator);
            assert!(error.contains(context), "missing {context} in {error}");
        }

        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let recipes = load(root().join("recipes.ron")).unwrap();
        let mut economy: EconomySource = load(root().join("economy.ron")).unwrap();
        economy.markets[0].energy_logistics.max_allocation_bps = Some(300);
        let config = load(root().join("economy_config.ron")).unwrap();
        let traders = load(root().join("traders.ron")).unwrap();
        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("economy.ron:frontier:system_01:energy_logistics:carrier_fee_bps"),
            "{error}"
        );
    }

    #[test]
    fn duplicate_archetype_id_reports_source_context() {
        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let recipes = load(root().join("recipes.ron")).unwrap();
        let economy = load(root().join("economy.ron")).unwrap();
        let config = load(root().join("economy_config.ron")).unwrap();
        let mut traders: TraderConfigSource = load(root().join("traders.ron")).unwrap();
        traders.npcs.archetypes[1].id = traders.npcs.archetypes[0].id.clone();
        let duplicate = traders.npcs.archetypes[1].id.clone();
        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!(
                "traders.ron:npcs:archetype:{duplicate}: duplicate archetype id"
            )),
            "{error}"
        );
    }

    #[test]
    fn investment_effect_bound_errors_retain_source_context() {
        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let recipes = load(root().join("recipes.ron")).unwrap();
        let economy = load(root().join("economy.ron")).unwrap();
        let mut config: EconomyConfigSource = load(root().join("economy_config.ron")).unwrap();
        let traders = load(root().join("traders.ron")).unwrap();
        let route = config
            .investments
            .iter_mut()
            .find(|shape| matches!(shape.kind, InvestmentKindSource::RouteSubsidy))
            .unwrap();
        route.maximum_level = 1;
        route.effect_per_level = u32::MAX - 99;
        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("economy_config.ron:investments"),
            "missing config source context in {error}"
        );

        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let recipes = load(root().join("recipes.ron")).unwrap();
        let mut economy: EconomySource = load(root().join("economy.ron")).unwrap();
        let mut config: EconomyConfigSource = load(root().join("economy_config.ron")).unwrap();
        let traders = load(root().join("traders.ron")).unwrap();
        let storage = config
            .investments
            .iter_mut()
            .find(|shape| matches!(shape.kind, InvestmentKindSource::Storage))
            .unwrap();
        storage.enabled = true;
        storage.maximum_level = 1;
        storage.effect_per_level = 1;
        economy.markets[0].energy_storage_cap = i64::MAX;
        let system = economy.markets[0].system.clone();
        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("economy.ron:{system}:investments")),
            "missing market source context in {error}"
        );
    }

    #[test]
    fn dynamic_trader_namespace_collision_reports_source_context() {
        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let recipes = load(root().join("recipes.ron")).unwrap();
        let economy = load(root().join("economy.ron")).unwrap();
        let config = load(root().join("economy_config.ron")).unwrap();
        let mut traders: TraderConfigSource = load(root().join("traders.ron")).unwrap();
        traders.npcs.mode = NpcFleetModeSource::Dynamic;
        traders.player.id = format!("{}_dynamic_00000001", traders.npcs.archetypes[0].id_prefix);

        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("traders.ron:npcs:archetype:frontier:general_freighter:id_prefix: generated trader namespace")
                && error.contains("_dynamic_00000001"),
            "{error}"
        );
    }

    #[test]
    fn nonzero_seasonal_amplitude_rejects_odd_period_with_source_context() {
        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let recipes = load(root().join("recipes.ron")).unwrap();
        let mut economy: EconomySource = load(root().join("economy.ron")).unwrap();
        let config = load(root().join("economy_config.ron")).unwrap();
        let traders = load(root().join("traders.ron")).unwrap();
        economy.markets[0].seasonal.amplitude_percent = 20;
        economy.markets[0].seasonal.period_ticks = 3;
        economy.markets[0].seasonal.phase_ticks = 0;

        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(
                "economy.ron:frontier:system_01:seasonal: nonzero amplitude requires an even period"
            ),
            "{error}"
        );
    }

    #[test]
    fn duplicate_market_schedules_report_source_contexts() {
        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let recipes = load(root().join("recipes.ron")).unwrap();
        let mut economy: EconomySource = load(root().join("economy.ron")).unwrap();
        let config = load(root().join("economy_config.ron")).unwrap();
        let traders = load(root().join("traders.ron")).unwrap();

        let source = &economy.markets[0].sources[0];
        let duplicate_source = SourceSource {
            good: source.good.clone(),
            quantity_per_tick: source.quantity_per_tick,
            extraction_energy: source.extraction_energy,
        };
        economy.markets[0].sources.push(duplicate_source);
        let recipe = economy.markets[4].recipes[0].clone();
        economy.markets[4].recipes.push(recipe);

        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        for context in [
            "economy.ron:frontier:system_01:sources: duplicate source good",
            "economy.ron:frontier:system_05:recipes: duplicate recipe",
        ] {
            assert!(error.contains(context), "missing {context} in {error}");
        }
    }

    #[test]
    fn fixed_point_generation_checks_ranges_rounding_and_overflow() {
        assert_eq!(checked_generation(101, 333).unwrap(), Energy(33));
        assert!(checked_generation(-1, 100).is_err());
        assert!(checked_generation(1, 1_001).is_err());
        assert!(checked_generation(i64::MAX, 1_000).is_err());
    }

    #[test]
    fn bootstrap_acknowledgement_is_a_structured_warning() {
        let systems: Vec<SystemSource> = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let recipes = load(root().join("recipes.ron")).unwrap();
        let mut economy: EconomySource = load(root().join("economy.ron")).unwrap();
        let config = load(root().join("economy_config.ron")).unwrap();
        let traders = load(root().join("traders.ron")).unwrap();
        let market = economy
            .markets
            .iter_mut()
            .find(|market| market.system == "frontier:system_19")
            .unwrap();
        market.starting_energy = 1;
        market.acknowledge_bootstrap_risk = false;
        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        assert!(error.contains("bootstrap runway"));

        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let recipes = load(root().join("recipes.ron")).unwrap();
        let mut economy: EconomySource = load(root().join("economy.ron")).unwrap();
        let config = load(root().join("economy_config.ron")).unwrap();
        let traders = load(root().join("traders.ron")).unwrap();
        let market = economy
            .markets
            .iter_mut()
            .find(|market| market.system == "frontier:system_19")
            .unwrap();
        market.starting_energy = 1;
        market.acknowledge_bootstrap_risk = true;
        let loaded = compile(systems, goods, recipes, economy, config, traders).unwrap();
        assert!(loaded.warnings.iter().any(|warning| matches!(warning, ContentWarning::BootstrapRunwayAcknowledged { system, .. } if system.as_str() == "frontier:system_19")));
    }

    #[test]
    fn population_window_content_accepts_maximum_and_rejects_first_value_above_it() {
        let compile_with_window = |window| {
            let systems = load(root().join("systems.ron")).unwrap();
            let goods = load(root().join("goods.ron")).unwrap();
            let recipes = load(root().join("recipes.ron")).unwrap();
            let economy = load(root().join("economy.ron")).unwrap();
            let mut config: EconomyConfigSource = load(root().join("economy_config.ron")).unwrap();
            config.population.sufficiency_window = window;
            let traders = load(root().join("traders.ron")).unwrap();
            compile(systems, goods, recipes, economy, config, traders)
        };
        assert!(compile_with_window(MAX_POPULATION_SUFFICIENCY_WINDOW_TICKS).is_ok());
        let error = compile_with_window(MAX_POPULATION_SUFFICIENCY_WINDOW_TICKS + 1)
            .unwrap_err()
            .to_string();
        assert!(error.contains("must be at most 10000 ticks"), "{error}");
    }

    #[test]
    #[ignore = "long repository acceptance; run explicitly with: cargo test -p game-content tests::repository_energy_economy_remains_active_and_deterministic_for_1000_ticks -- --ignored --exact"]
    fn repository_energy_economy_remains_active_and_deterministic_for_1000_ticks() {
        #[derive(Debug, Eq, PartialEq)]
        struct Outcome {
            events: Vec<game_core::GameEvent>,
            snapshot: String,
            energy_loaded: i64,
            energy_delivered: i64,
            contracts_accepted: u64,
            contracts_completed: u64,
            contracts_recovered: u64,
            starvation_attributions: u64,
            unsupplied_destination_ticks: u64,
            trades_after_300: u64,
            production_after_300: u64,
            stage_transitions: u64,
            population_changes: u64,
            reconciliation_difference: i64,
        }

        fn run(definition: GameDefinition) -> Outcome {
            let mut session = game_core::GameSession::new(definition).unwrap();
            let initial = session.snapshot();
            let initial_energy = initial
                .markets
                .iter()
                .map(|market| market.energy_stock.0)
                .sum::<i64>()
                + initial
                    .traders
                    .iter()
                    .map(|trader| {
                        trader.energy_tank.0
                            + trader.bulk_energy.owned.0
                            + trader.bulk_energy.locked.map_or(0, |lot| lot.amount.0)
                    })
                    .sum::<i64>();
            let mut events = Vec::new();
            let mut trades_after_300 = 0_u64;
            let mut production_after_300 = 0_u64;
            let mut unsupplied_destination_ticks = 0_u64;
            for tick in 1..=1_000 {
                session.step().unwrap();
                let current = session.drain_events();
                unsupplied_destination_ticks += current
                    .iter()
                    .filter(|event| {
                        matches!(
                            event,
                            game_core::GameEvent::LifeSupport {
                                unsupplied,
                                ..
                            } if *unsupplied > Energy::ZERO
                        )
                    })
                    .count() as u64;
                if tick > 300 {
                    trades_after_300 += current
                        .iter()
                        .filter(|event| {
                            matches!(
                                event,
                                game_core::GameEvent::Bought { .. }
                                    | game_core::GameEvent::Sold { .. }
                            )
                        })
                        .count() as u64;
                    production_after_300 += current
                        .iter()
                        .filter(|event| matches!(event, game_core::GameEvent::Produced { .. }))
                        .count() as u64;
                }
                events.extend(current);
                if tick % 50 == 0 {
                    assert!(session.snapshot().traders.iter().all(|trader| {
                        trader.player
                            || trader.travel.is_some()
                            || trader.cargo.values().all(|quantity| *quantity == 0)
                    }));
                }
            }
            let final_snapshot = session.snapshot();
            let processor_solvency = session.processor_solvency().unwrap();
            assert!(
                processor_solvency.iter().all(|row| row.solvent),
                "processor structural insolvency: {processor_solvency:?}"
            );
            let energy_loaded = final_snapshot
                .markets
                .iter()
                .map(|market| market.energy_flow.contract_source_loaded.0)
                .sum();
            let energy_delivered = final_snapshot
                .markets
                .iter()
                .map(|market| market.energy_flow.contract_destination_delivered.0)
                .sum();
            assert!(
                energy_loaded > 0,
                "no Energy was loaded into a contract-locked bulk lot"
            );
            assert!(
                energy_delivered > 0,
                "no Energy contract delivered physical Energy"
            );
            assert!(trades_after_300 > 0, "trade stopped by tick 300");
            assert!(production_after_300 > 0, "production stopped by tick 300");
            let initially_populated = initial
                .markets
                .iter()
                .map(|market| market.population)
                .sum::<u64>();
            let finally_populated = final_snapshot
                .markets
                .iter()
                .map(|market| market.population)
                .sum::<u64>();
            assert!(
                finally_populated >= initially_populated / 2,
                "repository population globally collapsed"
            );
            // This 1,000-tick gate owns deterministic activity, fleet health,
            // and exact reconciliation. Population has a 500-tick sufficiency
            // window and intentionally slower dynamics; its required settled
            // change is enforced by the separate 10,000-tick CLI gate.
            assert!(
                final_snapshot
                    .markets
                    .iter()
                    .filter(|market| market.energy_flow.life_support_unsupplied > Energy::ZERO)
                    .all(|market| market.system_id.as_str() == "frontier:system_19"),
                "life-support failure escaped the tuned demographic stress market"
            );
            assert!(
                final_snapshot
                    .markets
                    .iter()
                    .filter(|market| !market.policy.import_priorities.is_empty()
                        || !market.targets.is_empty())
                    .all(|market| market.energy_stock > Energy::ZERO)
            );
            for market in &final_snapshot.markets {
                let claims = final_snapshot
                    .reservations
                    .iter()
                    .filter(|reservation| {
                        reservation.status == game_core::ReservationStatus::Active
                            && reservation.destination == market.system_id
                    })
                    .map(|reservation| reservation.reserved_energy.0)
                    .sum::<i64>();
                assert_eq!(market.reserved_energy.0, claims);
            }
            let final_energy = final_snapshot
                .markets
                .iter()
                .map(|market| market.energy_stock.0)
                .sum::<i64>()
                + final_snapshot
                    .traders
                    .iter()
                    .map(|trader| {
                        trader.energy_tank.0
                            + trader.bulk_energy.owned.0
                            + trader.bulk_energy.locked.map_or(0, |lot| lot.amount.0)
                    })
                    .sum::<i64>();
            let expected_delta = i64::try_from(i128::from(
                final_snapshot.energy_flow.net_external_delta().0,
            ))
            .unwrap();
            let reconciliation_difference = final_energy - initial_energy - expected_delta;
            assert_eq!(reconciliation_difference, 0);
            let diagnostics = &final_snapshot.energy_logistics;
            let starvation_attributions = diagnostics
                .arrived_settlement_blocked
                .checked_add(diagnostics.accepted_delivery_pending)
                .and_then(|value| value.checked_add(diagnostics.no_reachable_surplus))
                .and_then(|value| value.checked_add(diagnostics.no_viable_candidate))
                .and_then(|value| value.checked_add(diagnostics.viable_but_unaccepted))
                .unwrap();
            assert_eq!(starvation_attributions, unsupplied_destination_ticks);
            assert!(
                diagnostics.accepted > 0,
                "no Energy contracts were accepted"
            );
            assert!(diagnostics.completed > 0, "no Energy contracts completed");
            Outcome {
                events,
                snapshot: format!("{final_snapshot:?}"),
                energy_loaded,
                energy_delivered,
                contracts_accepted: diagnostics.accepted,
                contracts_completed: diagnostics.completed,
                contracts_recovered: diagnostics.recovered_after_failure,
                starvation_attributions,
                unsupplied_destination_ticks,
                trades_after_300,
                production_after_300,
                stage_transitions: final_snapshot.dynamics_history.stage_transitions,
                population_changes: final_snapshot.dynamics_history.population_changes,
                reconciliation_difference,
            }
        }

        let first = run(load_directory(root()).unwrap());
        let second = run(load_directory(root()).unwrap());
        assert_eq!(first, second);
        println!(
            "1000-tick acceptance: reconciliation_difference={} stage_transitions={} population_changes={} energy_loaded={} energy_delivered={} contracts_accepted={} contracts_completed={} contracts_recovered={} starvation_attributions={} unsupplied_destination_ticks={} trades_after_300={} production_after_300={}",
            first.reconciliation_difference,
            first.stage_transitions,
            first.population_changes,
            first.energy_loaded,
            first.energy_delivered,
            first.contracts_accepted,
            first.contracts_completed,
            first.contracts_recovered,
            first.starvation_attributions,
            first.unsupplied_destination_ticks,
            first.trades_after_300,
            first.production_after_300
        );
    }

    #[test]
    fn repository_economy_short_smoke_is_deterministic_and_active() {
        fn run(definition: GameDefinition) -> (Vec<game_core::GameEvent>, String) {
            let mut session = game_core::GameSession::new(definition).unwrap();
            let mut events = Vec::new();
            for _ in 0..50 {
                session.step().unwrap();
                events.extend(session.drain_events());
            }
            (events, format!("{:?}", session.snapshot()))
        }

        let first = run(load_directory(root()).unwrap());
        let second = run(load_directory(root()).unwrap());
        assert_eq!(first, second);
        assert!(
            first.0.iter().any(|event| matches!(
                event,
                game_core::GameEvent::Bought { .. } | game_core::GameEvent::Sold { .. }
            )),
            "short repository smoke had no trade activity"
        );
        assert!(
            first
                .0
                .iter()
                .any(|event| matches!(event, game_core::GameEvent::Produced { .. })),
            "short repository smoke had no production activity"
        );
    }

    #[test]
    fn graph_errors_aggregate_with_independent_schema_errors() {
        let mut systems: Vec<SystemSource> = load(root().join("systems.ron")).unwrap();
        for (index, system) in systems.iter_mut().enumerate() {
            system.position.x = if index < 10 {
                index as f64
            } else {
                10_000.0 + index as f64
            };
            system.position.y = 0.0;
            system.position.z = 0.0;
        }
        let mut goods: Vec<GoodSource> = load(root().join("goods.ron")).unwrap();
        goods[0].bootstrap_cost = 0;
        let recipes = load(root().join("recipes.ron")).unwrap();
        let economy = load(root().join("economy.ron")).unwrap();
        let config = load(root().join("economy_config.ron")).unwrap();
        let traders = load(root().join("traders.ron")).unwrap();
        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        assert!(error.contains("bootstrap_cost must be positive"), "{error}");
        assert!(error.contains("system graph is disconnected"), "{error}");
    }

    #[test]
    fn rejects_duplicate_recipe_inputs_and_outputs_with_source_context() {
        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let mut recipes: Vec<RecipeSource> = load(root().join("recipes.ron")).unwrap();
        let duplicate_input = recipes[0].inputs[0].clone();
        let duplicate_output = recipes[0].outputs[0].clone();
        recipes[0].inputs.push(duplicate_input);
        recipes[0].outputs.push(duplicate_output);
        let economy = load(root().join("economy.ron")).unwrap();
        let config = load(root().join("economy_config.ron")).unwrap();
        let traders = load(root().join("traders.ron")).unwrap();
        let error = compile(systems, goods, recipes, economy, config, traders)
            .unwrap_err()
            .to_string();
        assert!(error.contains("recipes.ron:"));
        assert!(error.contains(":input: duplicate good"));
        assert!(error.contains(":output: duplicate good"));
    }

    #[test]
    fn protected_budget_uses_the_runtime_liquidation_contract_adversarially() {
        let loaded = load_directory(root()).unwrap();
        let graph = SystemGraph::build(&loaded.systems).unwrap();
        for system in &loaded.systems {
            let adjacent = graph
                .neighbors(&system.id)
                .iter()
                .map(|(_, distance)| *distance)
                .min_by(f64::total_cmp)
                .unwrap();
            for trader in &loaded.traders {
                let target = liquidation_target_energy(
                    game_core::travel_energy(adjacent, trader.travel_burn_per_distance).unwrap(),
                    system.policy.liquidation_threshold_percent,
                )
                .unwrap();
                for good in &loaded.goods {
                    let price = liquidation_unit_price(
                        good.bootstrap_cost,
                        system.policy.liquidation_discount_percent,
                    )
                    .unwrap();
                    let payout = ((target.0 + price.0 - 1) / price.0) * price.0;
                    assert!(
                        system.protected_liquidation_budget.0 >= payout,
                        "{} / {} / {} is under-protected",
                        system.id,
                        trader.id,
                        good.id
                    );
                }
            }
        }
    }

    #[test]
    fn non_default_source_scaling_matches_runtime_role_and_reserve_math() {
        assert_eq!(scaled_source_output(7, 50).unwrap(), 3);
        assert_eq!(scaled_source_output(7, 150).unwrap(), 10);
        let loaded = load_directory(root()).unwrap();
        let mut config = loaded.economy.clone();
        config.source_output_percent = 50;
        let recipes = loaded
            .recipes
            .iter()
            .map(|recipe| (recipe.id.clone(), recipe))
            .collect::<BTreeMap<_, _>>();
        let sourced = loaded
            .systems
            .iter()
            .find(|system| !system.sources.is_empty())
            .unwrap();
        let burn = system_burn(sourced, &recipes, &config).unwrap();
        let expected_source = sourced.sources.iter().fold(0_i64, |sum, source| {
            sum + source.extraction_energy.0
                * i64::from(scaled_source_output(source.quantity_per_tick, 50).unwrap())
        });
        let life = config.life_support_burn_per_capita.0 * sourced.population as i64;
        let recipe = sourced
            .recipes
            .iter()
            .map(|id| recipes[id].operating_energy.0)
            .sum::<i64>();
        assert_eq!(burn, life + expected_source + recipe);
    }

    #[test]
    fn rejects_wrong_energy_identity_weights_and_correlation() {
        let systems = load(root().join("systems.ron")).unwrap();
        let mut goods: Vec<GoodSource> = load(root().join("goods.ron")).unwrap();
        goods[0].bootstrap_cost = 2;
        let recipes = load(root().join("recipes.ron")).unwrap();
        let economy = load(root().join("economy.ron")).unwrap();
        let config = load(root().join("economy_config.ron")).unwrap();
        let traders = load(root().join("traders.ron")).unwrap();
        assert!(compile(systems, goods, recipes, economy, config, traders).is_err());

        let systems = load(root().join("systems.ron")).unwrap();
        let goods = load(root().join("goods.ron")).unwrap();
        let mut recipes: Vec<RecipeSource> = load(root().join("recipes.ron")).unwrap();
        recipes[0].outputs[0].cost_weight = 0;
        let economy = load(root().join("economy.ron")).unwrap();
        let config = load(root().join("economy_config.ron")).unwrap();
        let traders = load(root().join("traders.ron")).unwrap();
        assert!(compile(systems, goods, recipes, economy, config, traders).is_err());
    }
}
