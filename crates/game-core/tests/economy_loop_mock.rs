//! Deliberately simplified stock-flow experiment for economy design.
//!
//! This does not exercise the ECS trader implementation. It isolates the monetary
//! hypothesis that recurring trader operating costs and tertiary-to-extraction
//! support goods are complementary loops. Keep it separate from production rules
//! until the economic model is selected.

const ACTOR_COUNT: usize = 4;
const SOURCE: usize = 0;
const PROCESSOR: usize = 1;
const TERTIARY: usize = 2;
const TRADER: usize = 3;
const STARTING_BALANCE: i64 = 10_000;
const TEST_CYCLES: u32 = 1_000;

#[derive(Clone, Copy, Debug)]
struct Scenario {
    trader_operating_cost: bool,
    tertiary_support_loop: bool,
}

#[derive(Debug)]
struct Outcome {
    completed_cycles: u32,
    balances: [i64; ACTOR_COUNT],
    tertiary_support_produced: u32,
    extraction_support_consumed: u32,
}

fn run(scenario: Scenario) -> Outcome {
    let mut balances = [STARTING_BALANCE; ACTOR_COUNT];
    let initial_total = balances.iter().sum::<i64>();
    let mut completed_cycles = 0;
    let mut tertiary_support_produced = 0;
    let mut extraction_support_consumed = 0;

    for _ in 0..TEST_CYCLES {
        let mut delta = [0_i64; ACTOR_COUNT];

        // Base chain: trader buys raw material, sells it to the processor, buys
        // the processed good, then sells it to a tertiary consumer.
        transfer(&mut delta, TRADER, SOURCE, 30);
        transfer(&mut delta, PROCESSOR, TRADER, 40);
        transfer(&mut delta, TRADER, PROCESSOR, 40);
        transfer(&mut delta, TERTIARY, TRADER, 60);

        if scenario.tertiary_support_loop {
            // The tertiary site sells one abstract support unit to the trader;
            // the source buys it as extraction upkeep for the next raw batch.
            transfer(&mut delta, TRADER, TERTIARY, 50);
            transfer(&mut delta, SOURCE, TRADER, 60);
        }

        if scenario.trader_operating_cost {
            // Abstract fuel/service spending. Distribution models refueling and
            // servicing at source and tertiary destinations without introducing
            // fuel inventory or route rules into the production simulation.
            transfer(&mut delta, TRADER, SOURCE, 30);
            transfer(&mut delta, TRADER, TERTIARY, 10);
        }

        if balances
            .iter()
            .zip(delta)
            .any(|(balance, change)| *balance + change < 0)
        {
            break;
        }
        for (balance, change) in balances.iter_mut().zip(delta) {
            *balance += change;
        }
        assert_eq!(balances.iter().sum::<i64>(), initial_total);
        if scenario.tertiary_support_loop {
            tertiary_support_produced += 1;
            extraction_support_consumed += 1;
        }
        completed_cycles += 1;
    }

    Outcome {
        completed_cycles,
        balances,
        tertiary_support_produced,
        extraction_support_consumed,
    }
}

fn transfer(delta: &mut [i64; ACTOR_COUNT], payer: usize, recipient: usize, amount: i64) {
    delta[payer] -= amount;
    delta[recipient] += amount;
}

#[test]
fn fuel_and_tertiary_support_mock_close_complementary_cash_loops() {
    let baseline = run(Scenario {
        trader_operating_cost: false,
        tertiary_support_loop: false,
    });
    let fuel_only = run(Scenario {
        trader_operating_cost: true,
        tertiary_support_loop: false,
    });
    let support_only = run(Scenario {
        trader_operating_cost: false,
        tertiary_support_loop: true,
    });
    let combined = run(Scenario {
        trader_operating_cost: true,
        tertiary_support_loop: true,
    });

    assert_eq!(baseline.completed_cycles, 166);
    assert_eq!(fuel_only.completed_cycles, 200);
    assert_eq!(support_only.completed_cycles, 333);
    assert_eq!(combined.completed_cycles, TEST_CYCLES);
    assert_eq!(combined.balances, [STARTING_BALANCE; ACTOR_COUNT]);
    assert_eq!(combined.tertiary_support_produced, TEST_CYCLES);
    assert_eq!(combined.extraction_support_consumed, TEST_CYCLES);
}
