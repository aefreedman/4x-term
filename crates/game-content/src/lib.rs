//! RON loading, validation, and compilation into format-independent core definitions.

use game_core::{
    ContentId, GameDefinition, GoodAmount, GoodCategory, GoodDefinition, Money, Position3,
    RecipeDefinition, RecipeLayer, SourceDefinition, SystemDefinition, TraderDefinition,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContentError {
    #[error("failed to read {path}: {source}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error("failed to parse {path}: {source}")]
    Parse { path: PathBuf, source: ron::error::SpannedError },
    #[error("content validation failed:\n{}", .0.join("\n"))]
    Validation(Vec<String>),
}

#[derive(Deserialize)]
struct SystemSource {
    id: String,
    name: String,
    position: PositionSource,
}

#[derive(Deserialize)]
struct PositionSource { x: f64, y: f64, z: f64 }

#[derive(Deserialize)]
struct GoodSource {
    id: String,
    name: String,
    category: CategorySource,
    base_price: i64,
}

#[derive(Clone, Copy, Deserialize)]
enum CategorySource { Raw, Primary, Secondary }

#[derive(Deserialize)]
struct RecipeSource {
    id: String,
    name: String,
    layer: LayerSource,
    inputs: Vec<AmountSource>,
    outputs: Vec<AmountSource>,
}

#[derive(Clone, Copy, Deserialize)]
enum LayerSource { Primary, Secondary, Tertiary }

#[derive(Deserialize)]
struct AmountSource { good: String, quantity: u32 }

#[derive(Deserialize)]
struct EconomySource {
    markets: Vec<MarketSource>,
    traders: Vec<TraderSource>,
}

#[derive(Deserialize)]
struct MarketSource {
    system: String,
    currency: i64,
    inventory: Vec<AmountSource>,
    targets: Vec<AmountSource>,
    recipes: Vec<String>,
    sources: Vec<AmountSource>,
}

#[derive(Deserialize)]
struct TraderSource {
    id: String,
    name: String,
    system: String,
    currency: i64,
    cargo_capacity: u32,
    speed: f64,
    player: bool,
}

pub fn load_directory(root: impl AsRef<Path>) -> Result<GameDefinition, ContentError> {
    let root = root.as_ref();
    let systems: Vec<SystemSource> = load(root.join("systems.ron"))?;
    let goods: Vec<GoodSource> = load(root.join("goods.ron"))?;
    let recipes: Vec<RecipeSource> = load(root.join("recipes.ron"))?;
    let economy: EconomySource = load(root.join("economy.ron"))?;
    compile(systems, goods, recipes, economy)
}

fn load<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<T, ContentError> {
    let text = fs::read_to_string(&path).map_err(|source| ContentError::Read { path: path.clone(), source })?;
    ron::from_str(&text).map_err(|source| ContentError::Parse { path, source })
}

fn parse_id(raw: &str, context: &str, errors: &mut Vec<String>) -> Option<ContentId> {
    match ContentId::new(raw) {
        Ok(id) => Some(id),
        Err(error) => { errors.push(format!("{context}: {error}")); None }
    }
}

fn compile(
    systems: Vec<SystemSource>, goods: Vec<GoodSource>, recipes: Vec<RecipeSource>, economy: EconomySource,
) -> Result<GameDefinition, ContentError> {
    let mut errors = Vec::new();
    if systems.len() != 20 { errors.push(format!("systems.ron: expected exactly 20 systems, found {}", systems.len())); }
    let mut seen = BTreeSet::new();
    let mut compiled_goods = Vec::new();
    let mut categories = BTreeMap::new();
    for source in goods {
        let Some(id) = parse_id(&source.id, "goods.ron", &mut errors) else { continue; };
        if !seen.insert(id.clone()) { errors.push(format!("goods.ron: duplicate id {id}")); continue; }
        if source.base_price <= 0 { errors.push(format!("goods.ron:{id}: base_price must be positive")); }
        let category = match source.category { CategorySource::Raw => GoodCategory::Raw, CategorySource::Primary => GoodCategory::Primary, CategorySource::Secondary => GoodCategory::Secondary };
        categories.insert(id.clone(), category);
        compiled_goods.push(GoodDefinition { id, name: source.name, category, base_price: Money(source.base_price) });
    }
    let good_ids: BTreeSet<_> = compiled_goods.iter().map(|good| good.id.clone()).collect();
    let mut recipe_seen = BTreeSet::new();
    let mut compiled_recipes = Vec::new();
    for source in recipes {
        let Some(id) = parse_id(&source.id, "recipes.ron", &mut errors) else { continue; };
        if !recipe_seen.insert(id.clone()) { errors.push(format!("recipes.ron: duplicate id {id}")); continue; }
        let layer = match source.layer { LayerSource::Primary => RecipeLayer::Primary, LayerSource::Secondary => RecipeLayer::Secondary, LayerSource::Tertiary => RecipeLayer::Tertiary };
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for (label, values, target) in [("input", source.inputs, &mut inputs), ("output", source.outputs, &mut outputs)] {
            for value in values {
                let Some(good) = parse_id(&value.good, &format!("recipes.ron:{id}:{label}"), &mut errors) else { continue; };
                if !good_ids.contains(&good) { errors.push(format!("recipes.ron:{id}: unknown good {good}")); }
                if value.quantity == 0 { errors.push(format!("recipes.ron:{id}: quantity must be positive")); }
                target.push(GoodAmount { good, quantity: value.quantity });
            }
        }
        if inputs.is_empty() { errors.push(format!("recipes.ron:{id}: inputs cannot be empty")); }
        match layer {
            RecipeLayer::Primary => {
                if !inputs.iter().any(|a| categories.get(&a.good) == Some(&GoodCategory::Raw)) { errors.push(format!("recipes.ron:{id}: primary recipe needs a raw input")); }
                if !outputs.iter().any(|a| categories.get(&a.good) == Some(&GoodCategory::Primary)) { errors.push(format!("recipes.ron:{id}: primary recipe needs a primary output")); }
            }
            RecipeLayer::Secondary => {
                if !inputs.iter().any(|a| categories.get(&a.good) == Some(&GoodCategory::Primary)) || !inputs.iter().any(|a| categories.get(&a.good) == Some(&GoodCategory::Raw)) { errors.push(format!("recipes.ron:{id}: secondary recipe needs primary and raw inputs")); }
                if !outputs.iter().any(|a| categories.get(&a.good) == Some(&GoodCategory::Secondary)) { errors.push(format!("recipes.ron:{id}: secondary recipe needs a secondary output")); }
            }
            RecipeLayer::Tertiary if !outputs.is_empty() => errors.push(format!("recipes.ron:{id}: tertiary recipe cannot produce goods")),
            RecipeLayer::Tertiary => {}
        }
        compiled_recipes.push(RecipeDefinition { id, name: source.name, layer, inputs, outputs });
    }
    let recipe_ids: BTreeSet<_> = compiled_recipes.iter().map(|recipe| recipe.id.clone()).collect();
    let mut markets = BTreeMap::new();
    for source in economy.markets {
        let Some(system) = parse_id(&source.system, "economy.ron:market", &mut errors) else { continue; };
        if markets.contains_key(&system) { errors.push(format!("economy.ron: duplicate market {system}")); continue; }
        let inventory = amounts_to_map(source.inventory, &good_ids, "inventory", &mut errors);
        let targets = amounts_to_map(source.targets, &good_ids, "targets", &mut errors);
        if targets.values().any(|value| *value == 0) { errors.push(format!("economy.ron:{system}: targets must be positive")); }
        let recipe_refs = source.recipes.into_iter().filter_map(|raw| {
            let parsed = parse_id(&raw, &format!("economy.ron:{system}:recipe"), &mut errors)?;
            if !recipe_ids.contains(&parsed) { errors.push(format!("economy.ron:{system}: unknown recipe {parsed}")); }
            Some(parsed)
        }).collect();
        let sources = source.sources.into_iter().filter_map(|amount| {
            let good = parse_id(&amount.good, &format!("economy.ron:{system}:source"), &mut errors)?;
            if categories.get(&good) != Some(&GoodCategory::Raw) { errors.push(format!("economy.ron:{system}: source {good} must be raw")); }
            if amount.quantity == 0 { errors.push(format!("economy.ron:{system}: source quantity must be positive")); }
            Some(SourceDefinition { good, quantity_per_tick: amount.quantity })
        }).collect();
        if source.currency < 0 { errors.push(format!("economy.ron:{system}: currency cannot be negative")); }
        markets.insert(system, (Money(source.currency), inventory, targets, recipe_refs, sources));
    }
    let mut system_seen = BTreeSet::new();
    let mut positions = BTreeSet::new();
    let mut compiled_systems = Vec::new();
    for source in systems {
        let Some(id) = parse_id(&source.id, "systems.ron", &mut errors) else { continue; };
        if !system_seen.insert(id.clone()) { errors.push(format!("systems.ron: duplicate id {id}")); continue; }
        let position = Position3 { x: source.position.x, y: source.position.y, z: source.position.z };
        if !position.is_finite() { errors.push(format!("systems.ron:{id}: position must be finite")); }
        let key = (position.x.to_bits(), position.y.to_bits(), position.z.to_bits());
        if !positions.insert(key) { errors.push(format!("systems.ron:{id}: duplicate position")); }
        let Some((currency, inventory, targets, recipes, sources)) = markets.remove(&id) else { errors.push(format!("economy.ron: missing market for {id}")); continue; };
        compiled_systems.push(SystemDefinition { id, name: source.name, position, inventory, targets, currency, recipes, sources });
    }
    for extra in markets.keys() { errors.push(format!("economy.ron: market references unknown system {extra}")); }
    let system_ids: BTreeSet<_> = compiled_systems.iter().map(|system| system.id.clone()).collect();
    let mut trader_seen = BTreeSet::new();
    let mut compiled_traders = Vec::new();
    for source in economy.traders {
        let Some(id) = parse_id(&source.id, "economy.ron:trader", &mut errors) else { continue; };
        let Some(system) = parse_id(&source.system, &format!("economy.ron:{id}"), &mut errors) else { continue; };
        if !trader_seen.insert(id.clone()) { errors.push(format!("economy.ron: duplicate trader {id}")); }
        if !system_ids.contains(&system) { errors.push(format!("economy.ron:{id}: unknown system {system}")); }
        if source.currency < 0 || source.cargo_capacity == 0 || !source.speed.is_finite() || source.speed <= 0.0 { errors.push(format!("economy.ron:{id}: invalid trader numeric value")); }
        compiled_traders.push(TraderDefinition { id, name: source.name, system, currency: Money(source.currency), cargo_capacity: source.cargo_capacity, speed: source.speed, player: source.player });
    }
    if compiled_traders.iter().filter(|trader| trader.player).count() != 1 { errors.push("economy.ron: expected exactly one player trader".into()); }
    if !errors.is_empty() { return Err(ContentError::Validation(errors)); }
    let definition = GameDefinition { goods: compiled_goods, recipes: compiled_recipes, systems: compiled_systems, traders: compiled_traders };
    game_core::SystemGraph::build(&definition.systems).map_err(|error| ContentError::Validation(vec![format!("system graph: {error}")]))?;
    Ok(definition)
}

fn amounts_to_map(values: Vec<AmountSource>, goods: &BTreeSet<ContentId>, label: &str, errors: &mut Vec<String>) -> BTreeMap<ContentId, u32> {
    let mut result = BTreeMap::new();
    for value in values {
        let Some(good) = parse_id(&value.good, &format!("economy.ron:{label}"), errors) else { continue; };
        if !goods.contains(&good) { errors.push(format!("economy.ron:{label}: unknown good {good}")); }
        if result.insert(good.clone(), value.quantity).is_some() { errors.push(format!("economy.ron:{label}: duplicate good {good}")); }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_content_loads() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let definition = load_directory(root).expect("repository content should validate");
        assert_eq!(definition.systems.len(), 20);
        assert_eq!(definition.goods.len(), 10);
        assert_eq!(definition.recipes.len(), 9);
        assert_eq!(definition.traders.iter().filter(|trader| trader.player).count(), 1);
    }
}
