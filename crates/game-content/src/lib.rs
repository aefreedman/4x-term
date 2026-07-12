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
    base_price: i64,
}

#[derive(Clone, Copy, Deserialize)]
enum CategorySource {
    Raw,
    Primary,
    Secondary,
}

#[derive(Deserialize)]
struct RecipeSource {
    id: String,
    name: String,
    layer: LayerSource,
    inputs: Vec<AmountSource>,
    outputs: Vec<AmountSource>,
}

#[derive(Clone, Copy, Deserialize)]
enum LayerSource {
    Primary,
    Secondary,
    Tertiary,
}

#[derive(Deserialize)]
struct AmountSource {
    good: String,
    quantity: u32,
}

#[derive(Deserialize)]
struct EconomySource {
    markets: Vec<MarketSource>,
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
struct TraderConfigSource {
    player: PlayerTraderSource,
    npcs: NpcTraderSource,
}

#[derive(Deserialize)]
struct PlayerTraderSource {
    id: String,
    name: String,
    starting_system: String,
    currency: i64,
    cargo_capacity: u32,
    speed: f64,
}

#[derive(Deserialize)]
struct NpcTraderSource {
    count: usize,
    id_prefix: String,
    name_prefix: String,
    currency: i64,
    cargo_capacity: u32,
    speed: f64,
    distribution: TraderDistributionSource,
}

#[derive(Clone, Copy, Deserialize)]
enum TraderDistributionSource {
    EvenlySpaced,
}

pub fn load_directory(root: impl AsRef<Path>) -> Result<GameDefinition, ContentError> {
    let root = root.as_ref();
    let systems: Vec<SystemSource> = load(root.join("systems.ron"))?;
    let goods: Vec<GoodSource> = load(root.join("goods.ron"))?;
    let recipes: Vec<RecipeSource> = load(root.join("recipes.ron"))?;
    let economy: EconomySource = load(root.join("economy.ron"))?;
    let traders: TraderConfigSource = load(root.join("traders.ron"))?;
    compile(systems, goods, recipes, economy, traders)
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
    traders: TraderConfigSource,
) -> Result<GameDefinition, ContentError> {
    let mut errors = Vec::new();
    if systems.len() != 20 {
        errors.push(format!(
            "systems.ron: expected exactly 20 systems, found {}",
            systems.len()
        ));
    }
    let mut seen = BTreeSet::new();
    let mut compiled_goods = Vec::new();
    let mut categories = BTreeMap::new();
    for source in goods {
        let Some(id) = parse_id(&source.id, "goods.ron", &mut errors) else {
            continue;
        };
        if !seen.insert(id.clone()) {
            errors.push(format!("goods.ron: duplicate id {id}"));
            continue;
        }
        if source.base_price <= 0 {
            errors.push(format!("goods.ron:{id}: base_price must be positive"));
        }
        let category = match source.category {
            CategorySource::Raw => GoodCategory::Raw,
            CategorySource::Primary => GoodCategory::Primary,
            CategorySource::Secondary => GoodCategory::Secondary,
        };
        categories.insert(id.clone(), category);
        compiled_goods.push(GoodDefinition {
            id,
            name: source.name,
            category,
            base_price: Money(source.base_price),
        });
    }
    let good_ids: BTreeSet<_> = compiled_goods.iter().map(|good| good.id.clone()).collect();
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
        let layer = match source.layer {
            LayerSource::Primary => RecipeLayer::Primary,
            LayerSource::Secondary => RecipeLayer::Secondary,
            LayerSource::Tertiary => RecipeLayer::Tertiary,
        };
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for (label, values, target) in [
            ("input", source.inputs, &mut inputs),
            ("output", source.outputs, &mut outputs),
        ] {
            for value in values {
                let Some(good) = parse_id(
                    &value.good,
                    &format!("recipes.ron:{id}:{label}"),
                    &mut errors,
                ) else {
                    continue;
                };
                if !good_ids.contains(&good) {
                    errors.push(format!("recipes.ron:{id}: unknown good {good}"));
                }
                if value.quantity == 0 {
                    errors.push(format!("recipes.ron:{id}: quantity must be positive"));
                }
                target.push(GoodAmount {
                    good,
                    quantity: value.quantity,
                });
            }
        }
        if inputs.is_empty() {
            errors.push(format!("recipes.ron:{id}: inputs cannot be empty"));
        }
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
        compiled_recipes.push(RecipeDefinition {
            id,
            name: source.name,
            layer,
            inputs,
            outputs,
        });
    }
    let recipe_ids: BTreeSet<_> = compiled_recipes
        .iter()
        .map(|recipe| recipe.id.clone())
        .collect();
    let mut markets = BTreeMap::new();
    for source in economy.markets {
        let Some(system) = parse_id(&source.system, "economy.ron:market", &mut errors) else {
            continue;
        };
        if markets.contains_key(&system) {
            errors.push(format!("economy.ron: duplicate market {system}"));
            continue;
        }
        let inventory = amounts_to_map(source.inventory, &good_ids, "inventory", &mut errors);
        let targets = amounts_to_map(source.targets, &good_ids, "targets", &mut errors);
        if targets.values().any(|value| *value == 0) {
            errors.push(format!("economy.ron:{system}: targets must be positive"));
        }
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
        let sources = source
            .sources
            .into_iter()
            .filter_map(|amount| {
                let good = parse_id(
                    &amount.good,
                    &format!("economy.ron:{system}:source"),
                    &mut errors,
                )?;
                if categories.get(&good) != Some(&GoodCategory::Raw) {
                    errors.push(format!("economy.ron:{system}: source {good} must be raw"));
                }
                if amount.quantity == 0 {
                    errors.push(format!(
                        "economy.ron:{system}: source quantity must be positive"
                    ));
                }
                Some(SourceDefinition {
                    good,
                    quantity_per_tick: amount.quantity,
                })
            })
            .collect();
        if source.currency < 0 {
            errors.push(format!("economy.ron:{system}: currency cannot be negative"));
        }
        markets.insert(
            system,
            (
                Money(source.currency),
                inventory,
                targets,
                recipe_refs,
                sources,
            ),
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
        let key = (
            position.x.to_bits(),
            position.y.to_bits(),
            position.z.to_bits(),
        );
        if !positions.insert(key) {
            errors.push(format!("systems.ron:{id}: duplicate position"));
        }
        let Some((currency, inventory, targets, recipes, sources)) = markets.remove(&id) else {
            errors.push(format!("economy.ron: missing market for {id}"));
            continue;
        };
        compiled_systems.push(SystemDefinition {
            id,
            name: source.name,
            position,
            inventory,
            targets,
            currency,
            recipes,
            sources,
        });
    }
    for extra in markets.keys() {
        errors.push(format!(
            "economy.ron: market references unknown system {extra}"
        ));
    }
    let distinct_distances = compiled_systems
        .iter()
        .enumerate()
        .flat_map(|(index, system)| {
            compiled_systems[index + 1..]
                .iter()
                .map(move |other| system.position.distance(other.position).to_bits())
        })
        .collect::<BTreeSet<_>>();
    if compiled_systems.len() > 2 && distinct_distances.len() < 2 {
        errors.push("systems.ron: system distances must not be uniform".into());
    }
    let system_ids: BTreeSet<_> = compiled_systems
        .iter()
        .map(|system| system.id.clone())
        .collect();
    let mut compiled_traders = Vec::new();
    let player_id = parse_id(&traders.player.id, "traders.ron:player", &mut errors);
    let player_system = parse_id(
        &traders.player.starting_system,
        "traders.ron:player:starting_system",
        &mut errors,
    );
    if let (Some(id), Some(system)) = (player_id, player_system) {
        if !system_ids.contains(&system) {
            errors.push(format!(
                "traders.ron:player: unknown starting system {system}"
            ));
        }
        if !valid_trader_numbers(
            traders.player.currency,
            traders.player.cargo_capacity,
            traders.player.speed,
        ) {
            errors.push("traders.ron:player: invalid numeric value".into());
        }
        compiled_traders.push(TraderDefinition {
            id,
            name: traders.player.name,
            system,
            currency: Money(traders.player.currency),
            cargo_capacity: traders.player.cargo_capacity,
            speed: traders.player.speed,
            player: true,
        });
    }

    if traders.npcs.count > compiled_systems.len() {
        errors.push(format!(
            "traders.ron:npcs: count {} exceeds system count {}",
            traders.npcs.count,
            compiled_systems.len()
        ));
    }
    if !valid_trader_numbers(
        traders.npcs.currency,
        traders.npcs.cargo_capacity,
        traders.npcs.speed,
    ) {
        errors.push("traders.ron:npcs: invalid numeric value".into());
    }
    if traders.npcs.name_prefix.trim().is_empty() {
        errors.push("traders.ron:npcs: name_prefix cannot be empty".into());
    }
    if !compiled_systems.is_empty() && traders.npcs.count <= compiled_systems.len() {
        match traders.npcs.distribution {
            TraderDistributionSource::EvenlySpaced => {
                for index in 0..traders.npcs.count {
                    let system_index = ((2 * index + 1) * compiled_systems.len())
                        / (2 * traders.npcs.count.max(1));
                    let raw_id = format!("{}_{:02}", traders.npcs.id_prefix, index + 1);
                    let Some(id) = parse_id(&raw_id, "traders.ron:npcs:id_prefix", &mut errors)
                    else {
                        continue;
                    };
                    compiled_traders.push(TraderDefinition {
                        id,
                        name: format!("{} {:02}", traders.npcs.name_prefix, index + 1),
                        system: compiled_systems[system_index].id.clone(),
                        currency: Money(traders.npcs.currency),
                        cargo_capacity: traders.npcs.cargo_capacity,
                        speed: traders.npcs.speed,
                        player: false,
                    });
                }
            }
        }
    }
    let unique_trader_ids = compiled_traders
        .iter()
        .map(|trader| trader.id.clone())
        .collect::<BTreeSet<_>>();
    if unique_trader_ids.len() != compiled_traders.len() {
        errors.push("traders.ron: trader IDs must be unique".into());
    }
    if !errors.is_empty() {
        return Err(ContentError::Validation(errors));
    }
    let definition = GameDefinition {
        goods: compiled_goods,
        recipes: compiled_recipes,
        systems: compiled_systems,
        traders: compiled_traders,
    };
    game_core::SystemGraph::build(&definition.systems)
        .map_err(|error| ContentError::Validation(vec![format!("system graph: {error}")]))?;
    Ok(definition)
}

fn valid_trader_numbers(currency: i64, cargo_capacity: u32, speed: f64) -> bool {
    currency >= 0 && cargo_capacity > 0 && speed.is_finite() && speed > 0.0
}

fn amounts_to_map(
    values: Vec<AmountSource>,
    goods: &BTreeSet<ContentId>,
    label: &str,
    errors: &mut Vec<String>,
) -> BTreeMap<ContentId, u32> {
    let mut result = BTreeMap::new();
    for value in values {
        let Some(good) = parse_id(&value.good, &format!("economy.ron:{label}"), errors) else {
            continue;
        };
        if !goods.contains(&good) {
            errors.push(format!("economy.ron:{label}: unknown good {good}"));
        }
        if result.insert(good.clone(), value.quantity).is_some() {
            errors.push(format!("economy.ron:{label}: duplicate good {good}"));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_content_loads() {
        let definition = repository_definition();
        assert_eq!(definition.systems.len(), 20);
        assert_eq!(definition.goods.len(), 10);
        assert_eq!(definition.recipes.len(), 9);
        assert_eq!(
            definition
                .traders
                .iter()
                .filter(|trader| trader.player)
                .count(),
            1
        );
        let npcs = definition
            .traders
            .iter()
            .filter(|trader| !trader.player)
            .collect::<Vec<_>>();
        assert_eq!(npcs.len(), 9);
        assert!(npcs.iter().all(|trader| trader.speed == 8.0));
        assert_eq!(
            npcs.iter()
                .map(|trader| trader.system.as_str())
                .collect::<Vec<_>>(),
            vec![
                "frontier:system_02",
                "frontier:system_04",
                "frontier:system_06",
                "frontier:system_08",
                "frontier:system_11",
                "frontier:system_13",
                "frontier:system_15",
                "frontier:system_17",
                "frontier:system_19",
            ]
        );
    }

    #[test]
    fn semantic_validation_rejects_invalid_authored_content() {
        assert_invalid(|systems, _, _, _| systems[1].id = systems[0].id.clone());
        assert_invalid(|systems, _, _, _| systems[0].id = "Malformed ID".into());
        assert_invalid(|_, _, recipes, _| recipes[0].inputs[0].good = "frontier:missing".into());
        assert_invalid(|systems, _, _, _| systems[0].position.x = f64::INFINITY);
        assert_invalid(|systems, _, _, _| {
            let (x, y, z) = (
                systems[0].position.x,
                systems[0].position.y,
                systems[0].position.z,
            );
            systems[1].position = PositionSource { x, y, z };
        });
        assert_invalid(|_, goods, _, _| goods[0].base_price = 0);
        assert_invalid(|_, _, _, economy| economy.markets[0].targets[0].quantity = 0);
        assert_invalid(|_, _, recipes, _| {
            recipes[0].inputs[0].good = "frontier:structural_alloy".into()
        });
        assert_invalid_traders(|traders| traders.npcs.count = 21);
        assert_invalid_traders(|traders| traders.npcs.speed = 0.0);
        assert_invalid_traders(|traders| traders.npcs.id_prefix = "Invalid ID".into());
        assert_invalid_traders(|traders| traders.npcs.name_prefix.clear());
        assert_invalid_traders(|traders| {
            traders.player.starting_system = "frontier:missing".into()
        });
        assert_invalid(|systems, _, _, _| {
            for (index, system) in systems.iter_mut().enumerate() {
                let cluster = if index < 10 { 0.0 } else { 10_000.0 };
                system.position = PositionSource {
                    x: cluster + (index % 10) as f64,
                    y: (index % 3) as f64,
                    z: 0.0,
                };
            }
        });
    }

    #[test]
    fn repository_economy_is_deterministic_and_active() {
        let definition = repository_definition();
        let mut first = game_core::GameSession::new(definition.clone()).unwrap();
        let mut second = game_core::GameSession::new(definition).unwrap();
        for _ in 0..50 {
            first.step().unwrap();
            second.step().unwrap();
            assert_eq!(first.drain_events(), second.drain_events());
        }
        let first = first.snapshot();
        let second = second.snapshot();
        assert_eq!(format!("{first:?}"), format!("{second:?}"));
        assert!(
            first
                .traders
                .iter()
                .filter(|trader| !trader.player)
                .any(|trader| trader.ledger.completed_transactions > 0),
            "at least one automated trader should transact"
        );
        assert!(first.markets.iter().all(|market| market.currency.0 >= 0));
        assert!(first.traders.iter().all(|trader| trader.currency.0 >= 0));
    }

    fn repository_definition() -> GameDefinition {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        load_directory(root).expect("repository content should validate")
    }

    fn assert_invalid(
        mutate: impl FnOnce(
            &mut Vec<SystemSource>,
            &mut Vec<GoodSource>,
            &mut Vec<RecipeSource>,
            &mut EconomySource,
        ),
    ) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let mut systems = load(root.join("systems.ron")).unwrap();
        let mut goods = load(root.join("goods.ron")).unwrap();
        let mut recipes = load(root.join("recipes.ron")).unwrap();
        let mut economy = load(root.join("economy.ron")).unwrap();
        let traders = load(root.join("traders.ron")).unwrap();
        mutate(&mut systems, &mut goods, &mut recipes, &mut economy);
        assert!(matches!(
            compile(systems, goods, recipes, economy, traders),
            Err(ContentError::Validation(_))
        ));
    }

    fn assert_invalid_traders(mutate: impl FnOnce(&mut TraderConfigSource)) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let systems = load(root.join("systems.ron")).unwrap();
        let goods = load(root.join("goods.ron")).unwrap();
        let recipes = load(root.join("recipes.ron")).unwrap();
        let economy = load(root.join("economy.ron")).unwrap();
        let mut traders = load(root.join("traders.ron")).unwrap();
        mutate(&mut traders);
        assert!(matches!(
            compile(systems, goods, recipes, economy, traders),
            Err(ContentError::Validation(_))
        ));
    }
}
