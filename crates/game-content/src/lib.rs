//! RON loading, validation, and compilation into format-independent core definitions.

use game_core::{
    ContentId, ENERGY_ID, EconomyConfig, Energy, GameDefinition, GoodAmount, GoodCategory,
    GoodDefinition, LiquidationTraderCapability, MarketPolicy, Position3, PricingMode,
    RecipeDefinition, RecipeLayer, RecipeOutput, RefuelPolicy, SourceDefinition, SystemDefinition,
    SystemGraph, TraderDefinition, compute_protected_liquidation_budget, route_travel_energy,
    scaled_source_output, ticks_for_distance,
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
    pub warnings: Vec<ContentWarning>,
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
    policy: MarketPolicyOverrideSource,
    #[serde(default)]
    acknowledge_bootstrap_risk: bool,
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
    cargo_capacity: u32,
    speed: f64,
    travel_burn_per_distance: i64,
    refuel_policy: RefuelPolicySource,
}
#[derive(Deserialize)]
struct NpcTraderSource {
    count: usize,
    id_prefix: String,
    name_prefix: String,
    energy_tank: i64,
    energy_tank_capacity: i64,
    cargo_capacity: u32,
    speed: f64,
    travel_burn_per_distance: i64,
    refuel_policy: RefuelPolicySource,
    distribution: TraderDistributionSource,
}
#[derive(Clone, Copy, Deserialize)]
enum RefuelPolicySource {
    DepositAndWithdraw,
    DepositOnly,
    Disabled,
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
#[derive(Clone, Copy, Deserialize)]
enum TraderDistributionSource {
    EvenlySpaced,
}

pub fn load_directory(root: impl AsRef<Path>) -> Result<GameDefinition, ContentError> {
    Ok(load_directory_with_warnings(root)?.definition)
}

pub fn load_directory_with_warnings(root: impl AsRef<Path>) -> Result<LoadedContent, ContentError> {
    let root = root.as_ref();
    compile(
        load(root.join("systems.ron"))?,
        load(root.join("goods.ron"))?,
        load(root.join("recipes.ron"))?,
        load(root.join("economy.ron"))?,
        load(root.join("economy_config.ron"))?,
        load(root.join("traders.ron"))?,
    )
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
    validate_config(&config, &default_policy, &mut errors);
    let compiled_config = EconomyConfig {
        reservation_ttl: config.reservation_ttl,
        life_support_burn_per_capita: Energy(config.life_support_burn_per_capita),
        source_output_percent: config.source_output_percent,
        idle_trader_repositioning: config.idle_trader_repositioning,
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
        let recipe_refs = source
            .recipes
            .into_iter()
            .filter_map(|raw| {
                let parsed = parse_id(&raw, &format!("economy.ron:{system}:recipe"), &mut errors)?;
                if !recipe_ids.contains(&parsed) {
                    errors.push(format!("economy.ron:{system}: unknown recipe {parsed}"));
                }
                Some(parsed)
            })
            .collect();
        let sources = source.sources.into_iter().filter_map(|value| {
            let good = parse_id(&value.good, &format!("economy.ron:{system}:source"), &mut errors)?;
            if categories.get(&good) != Some(&GoodCategory::Raw) { errors.push(format!("economy.ron:{system}: source {good} must be raw")); }
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
        let policy = merge_policy(
            &default_policy,
            source.policy,
            &good_ids,
            &format!("economy.ron:{system}"),
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
                energy_storage_cap: Energy(source.energy_storage_cap),
                population: source.population,
                policy,
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
            energy_storage_cap: market.energy_storage_cap,
            population: market.population,
            policy: market.policy,
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

    let compiled_traders = compile_traders(traders, &compiled_systems, &mut errors);
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
        compute_protected_budgets(
            &mut compiled_systems,
            &compiled_goods,
            &compiled_traders,
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
            economy: compiled_config,
        },
        warnings,
    })
}

struct MarketCompiled {
    inventory: BTreeMap<ContentId, u64>,
    targets: BTreeMap<ContentId, u32>,
    recipes: Vec<ContentId>,
    sources: Vec<SourceDefinition>,
    energy_output_per_tick: Energy,
    energy_storage_cap: Energy,
    population: u64,
    policy: MarketPolicy,
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
) -> Vec<TraderDefinition> {
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
        ) {
            errors.push("traders.ron:player: invalid numeric value".into());
        }
        result.push(TraderDefinition {
            id,
            name: source.player.name,
            system,
            energy_tank: Energy(source.player.energy_tank),
            energy_tank_capacity: Energy(source.player.energy_tank_capacity),
            cargo_capacity: source.player.cargo_capacity,
            speed: source.player.speed,
            travel_burn_per_distance: Energy(source.player.travel_burn_per_distance),
            refuel_policy: source.player.refuel_policy.into(),
            player: true,
        });
    }
    if source.npcs.count > systems.len() {
        errors.push(format!(
            "traders.ron:npcs: count {} exceeds system count {}",
            source.npcs.count,
            systems.len()
        ));
    }
    if !valid_trader_numbers(
        source.npcs.energy_tank,
        source.npcs.energy_tank_capacity,
        source.npcs.cargo_capacity,
        source.npcs.speed,
        source.npcs.travel_burn_per_distance,
    ) {
        errors.push("traders.ron:npcs: invalid numeric value".into());
    }
    if source.npcs.name_prefix.trim().is_empty() {
        errors.push("traders.ron:npcs: name_prefix cannot be empty".into());
    }
    if !systems.is_empty() && source.npcs.count <= systems.len() {
        let TraderDistributionSource::EvenlySpaced = source.npcs.distribution;
        for index in 0..source.npcs.count {
            let system_index = ((2 * index + 1) * systems.len()) / (2 * source.npcs.count.max(1));
            let raw_id = format!("{}_{:02}", source.npcs.id_prefix, index + 1);
            let Some(id) = parse_id(&raw_id, "traders.ron:npcs:id_prefix", errors) else {
                continue;
            };
            result.push(TraderDefinition {
                id,
                name: format!("{} {:02}", source.npcs.name_prefix, index + 1),
                system: systems[system_index].id.clone(),
                energy_tank: Energy(source.npcs.energy_tank),
                energy_tank_capacity: Energy(source.npcs.energy_tank_capacity),
                cargo_capacity: source.npcs.cargo_capacity,
                speed: source.npcs.speed,
                travel_burn_per_distance: Energy(source.npcs.travel_burn_per_distance),
                refuel_policy: source.npcs.refuel_policy.into(),
                player: false,
            });
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
    result
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

fn compute_protected_budgets(
    systems: &mut [SystemDefinition],
    goods: &[GoodDefinition],
    traders: &[TraderDefinition],
    graph: &SystemGraph,
    errors: &mut Vec<String>,
) {
    let bootstrap_costs = goods
        .iter()
        .map(|good| good.bootstrap_cost)
        .collect::<Vec<_>>();
    let capabilities = traders
        .iter()
        .map(|trader| LiquidationTraderCapability {
            cargo_capacity: trader.cargo_capacity,
            energy_tank_capacity: trader.energy_tank_capacity,
            travel_burn_per_distance: trader.travel_burn_per_distance,
        })
        .collect::<Vec<_>>();
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

fn bootstrap_energy_ask(system: &SystemDefinition) -> Option<Energy> {
    let energy = ContentId::new(ENERGY_ID).ok()?;
    let target = u64::from(
        system
            .targets
            .get(&energy)
            .copied()
            .unwrap_or(system.policy.default_target),
    );
    if target == 0 {
        return None;
    }
    let stock = system.inventory.get(&energy).copied().unwrap_or(0);
    let shortage = target.saturating_sub(stock).min(target);
    let scarcity =
        1_000_u64.checked_add(500_u64.checked_mul(shortage)?.checked_add(target - 1)? / target)?;
    let sustainable = (100_u64
        .checked_add(u64::from(system.policy.producer_margin_percent))?
        .checked_add(99)?)
        / 100;
    let quote = sustainable.checked_mul(scarcity)?.checked_add(999)? / 1_000;
    Some(Energy(i64::try_from(quote.max(1)).ok()?))
}

fn bootstrap_energy_bid(system: &SystemDefinition) -> Option<Energy> {
    let energy = ContentId::new(ENERGY_ID).ok()?;
    let ask = bootstrap_energy_ask(system)?;
    let priority = u64::from(
        system
            .policy
            .import_priorities
            .get(&energy)
            .copied()
            .unwrap_or(100),
    );
    let quote = i128::from(ask.0)
        .checked_mul(i128::from(priority))?
        .checked_add(99)?
        / 100;
    Some(Energy(i64::try_from(quote.max(1)).ok()?))
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
                    let route_burn = route_travel_energy(
                        graph,
                        &approach_route,
                        trader.travel_burn_per_distance,
                    )
                    .ok()?
                    .checked_add(
                        route_travel_energy(
                            graph,
                            &delivery_route,
                            trader.travel_burn_per_distance,
                        )
                        .ok()?,
                    )
                    .ok()?;
                    let exporter_burn = *burn.get(&exporter.id)?;
                    let exporter_stock = exporter.inventory.get(energy_id).copied().unwrap_or(0);
                    let exporter_operating = exporter_burn
                        .checked_mul(i64::from(exporter.policy.operating_reserve_ticks))?;
                    let exporter_available = exporter_stock.saturating_sub(
                        u64::try_from(
                            exporter_operating
                                .checked_add(exporter.protected_liquidation_budget.0)?,
                        )
                        .ok()?,
                    );
                    let energy_ask = bootstrap_energy_ask(exporter)?;
                    let purchase_cost = energy_ask.checked_mul(delivery_quantity).ok()?;
                    let required_tank = purchase_cost.checked_add(route_burn).ok()?;
                    let arrival_tank = trader.energy_tank.checked_sub(required_tank).ok()?;
                    let importer_bid = bootstrap_energy_bid(importer)?;
                    let payout = importer_bid.checked_mul(delivery_quantity).ok()?;
                    let tank_headroom =
                        trader.energy_tank_capacity.checked_sub(arrival_tank).ok()?;
                    let exporter_final_stock = i64::try_from(exporter_stock)
                        .ok()?
                        .checked_sub(i64::try_from(delivery_quantity).ok()?)?
                        .checked_add(purchase_cost.0)?;
                    if exporter_available < delivery_quantity
                        || u64::from(trader.cargo_capacity) < delivery_quantity
                        || required_tank > trader.energy_tank
                        || payout > tank_headroom
                        || exporter_final_stock > exporter.energy_storage_cap.0
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
            let detail = "no exporter/trader pair has surplus stock, cargo capacity, purchase affordability, route burn, arrival tank headroom, exporter storage headroom, and one-tick delivery capacity".to_string();
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
        }));
        assert!(
            loaded
                .definition
                .traders
                .iter()
                .all(|trader| { trader.refuel_policy == RefuelPolicy::DepositAndWithdraw })
        );
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
    fn repository_energy_economy_remains_active_and_deterministic_for_1000_ticks() {
        #[derive(Debug, Eq, PartialEq)]
        struct Outcome {
            events: Vec<game_core::GameEvent>,
            snapshot: String,
            energy_loaded: i64,
            energy_delivered: i64,
            trades_after_300: u64,
            production_after_300: u64,
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
                            + i64::try_from(
                                trader
                                    .cargo
                                    .get(&ContentId::new(ENERGY_ID).unwrap())
                                    .copied()
                                    .unwrap_or(0),
                            )
                            .unwrap()
                    })
                    .sum::<i64>();
            let mut events = Vec::new();
            let mut trades_after_300 = 0_u64;
            let mut production_after_300 = 0_u64;
            for tick in 1..=1_000 {
                session.step().unwrap();
                let current = session.drain_events();
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
                .map(|market| market.energy_flow.market_to_energy_cargo.0)
                .sum();
            let energy_delivered = final_snapshot
                .markets
                .iter()
                .map(|market| market.energy_flow.energy_cargo_to_market.0)
                .sum();
            assert!(
                energy_loaded > 0,
                "no core:energy was loaded into a cargo bay"
            );
            assert!(
                energy_delivered > 0,
                "no core:energy cargo completed funded settlement"
            );
            assert!(trades_after_300 > 0, "trade stopped by tick 300");
            assert!(production_after_300 > 0, "production stopped by tick 300");
            assert!(
                final_snapshot
                    .markets
                    .iter()
                    .all(|market| market.energy_flow.life_support_unsupplied == Energy::ZERO),
                "repository importers accumulated unsupplied life support"
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
                            + i64::try_from(
                                trader
                                    .cargo
                                    .get(&ContentId::new(ENERGY_ID).unwrap())
                                    .copied()
                                    .unwrap_or(0),
                            )
                            .unwrap()
                    })
                    .sum::<i64>();
            assert_eq!(
                final_energy - initial_energy,
                i64::try_from(i128::from(
                    final_snapshot.energy_flow.net_external_delta().0,
                ))
                .unwrap()
            );
            Outcome {
                events,
                snapshot: format!("{final_snapshot:?}"),
                energy_loaded,
                energy_delivered,
                trades_after_300,
                production_after_300,
            }
        }

        let first = run(load_directory(root()).unwrap());
        let second = run(load_directory(root()).unwrap());
        assert_eq!(first, second);
        println!(
            "1000-tick acceptance: energy_loaded={} energy_delivered={} trades_after_300={} production_after_300={}",
            first.energy_loaded,
            first.energy_delivered,
            first.trades_after_300,
            first.production_after_300
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
