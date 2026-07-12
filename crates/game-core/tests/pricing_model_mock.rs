//! Production-independent pricing experiments.
//!
//! These tests model candidate equations only. They do not change ECS quotes,
//! market state, trader behavior, or authored content.

#[derive(Clone, Copy, Debug)]
struct RecipeCase {
    name: &'static str,
    scarce_input_cost: i64,
    output_base_price: i64,
}

const RECIPES: [RecipeCase; 6] = [
    RecipeCase {
        name: "Structural Alloy",
        scarce_input_cost: 30,
        output_base_price: 24,
    },
    RecipeCase {
        name: "Ceramic Composite",
        scarce_input_cost: 42,
        output_base_price: 30,
    },
    RecipeCase {
        name: "Biopolymer",
        scarce_input_cost: 38,
        output_base_price: 28,
    },
    RecipeCase {
        name: "Industrial Machinery",
        scarce_input_cost: 80,
        output_base_price: 85,
    },
    RecipeCase {
        name: "Habitat Modules",
        scarce_input_cost: 88,
        output_base_price: 100,
    },
    RecipeCase {
        name: "Reactor Assemblies",
        scarce_input_cost: 58,
        output_base_price: 110,
    },
];

fn current_midpoint(base_price: i64, inventory: i64, target: i64) -> i64 {
    let scarcity = (target - inventory).clamp(-target, target);
    (base_price + base_price * scarcity / (2 * target)).max(1)
}

fn current_market_ask(base_price: i64, inventory: i64, target: i64) -> i64 {
    (current_midpoint(base_price, inventory, target) * 110 / 100).max(1)
}

fn cost_floor(input_cost: i64, margin_percent: i64) -> i64 {
    (input_cost * (100 + margin_percent) + 99) / 100
}

fn cost_aware_ask(
    base_price: i64,
    inventory: i64,
    target: i64,
    input_cost: i64,
    margin_percent: i64,
) -> i64 {
    current_market_ask(base_price, inventory, target).max(cost_floor(input_cost, margin_percent))
}

fn funded_quantity(shortage: u32, available_cash: i64, unit_bid: i64) -> u32 {
    if available_cash <= 0 || unit_bid <= 0 {
        return 0;
    }
    shortage.min(u32::try_from(available_cash / unit_bid).unwrap_or(u32::MAX))
}

#[test]
fn current_scarcity_quotes_do_not_protect_recipe_solvency() {
    let immediate_margins = RECIPES.map(|recipe| {
        current_market_ask(recipe.output_base_price, 1, 1) - recipe.scarce_input_cost
    });
    let backlog_margins = RECIPES.map(|recipe| {
        current_market_ask(recipe.output_base_price, 2, 1) - recipe.scarce_input_cost
    });

    assert_eq!(immediate_margins, [-4, -9, -8, 13, 22, 63]);
    assert_eq!(backlog_margins, [-17, -26, -23, -33, -33, 2]);
    assert_eq!(
        RECIPES
            .iter()
            .zip(immediate_margins)
            .filter(|(_, margin)| *margin < 0)
            .count(),
        3,
        "all primary processors lose money even with immediate output sales"
    );
    assert_eq!(
        RECIPES
            .iter()
            .zip(backlog_margins)
            .filter(|(_, margin)| *margin < 0)
            .count(),
        5,
        "a two-unit output backlog makes five of six recipes loss-making"
    );
}

#[test]
fn acquisition_cost_floor_preserves_a_configured_margin() {
    const MARGIN_PERCENT: i64 = 15;
    assert_eq!(
        RECIPES.map(|recipe| cost_floor(recipe.scarce_input_cost, MARGIN_PERCENT)),
        [35, 49, 44, 92, 102, 67]
    );
    for recipe in RECIPES {
        for inventory in [1, 2, 10] {
            let ask = cost_aware_ask(
                recipe.output_base_price,
                inventory,
                1,
                recipe.scarce_input_cost,
                MARGIN_PERCENT,
            );
            assert!(
                ask >= cost_floor(recipe.scarce_input_cost, MARGIN_PERCENT),
                "{} ask {ask} fell below its cost floor",
                recipe.name
            );
        }
    }
}

#[test]
fn cost_floor_keeps_a_simple_processor_solvent_for_the_experiment_horizon() {
    const MARGIN_PERCENT: i64 = 15;
    const CYCLES: usize = 1_000;
    for recipe in RECIPES {
        let mut cash = 10_000_i64;
        for _ in 0..CYCLES {
            assert!(
                cash >= recipe.scarce_input_cost,
                "{} became unable to fund inputs",
                recipe.name
            );
            cash -= recipe.scarce_input_cost;
            cash += cost_aware_ask(
                recipe.output_base_price,
                2,
                1,
                recipe.scarce_input_cost,
                MARGIN_PERCENT,
            );
        }
        assert!(cash >= 10_000, "{} lost cash", recipe.name);
    }
}

#[test]
fn funded_demand_does_not_advertise_an_unpayable_quantity() {
    let first_reservation = funded_quantity(30, 313, 13);
    assert_eq!(first_reservation, 24);
    let reserved_cash = i64::from(first_reservation) * 13;
    assert!(reserved_cash <= 313);

    let second_reservation = funded_quantity(30 - first_reservation, 313 - reserved_cash, 13);
    assert_eq!(second_reservation, 0);
    assert!(reserved_cash + i64::from(second_reservation) * 13 <= 313);
}
