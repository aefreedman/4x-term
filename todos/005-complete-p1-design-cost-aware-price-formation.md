---
status: complete
priority: p1
issue_id: 005
tags: [economy, pricing, markets, design]
dependencies: []
resolution: superseded
superseded_by: 006
---
# Design Cost-Aware Price Formation

## Problem Statement

The current scarcity formula produces visible price movement but does not derive prices from production cost, expected downstream revenue, market liquidity, or outstanding demand. Processors can purchase inputs at high scarcity bids and then sell outputs below their acquisition cost, causing structural insolvency even when recipes are producing useful goods.

Further equation-only mock testing will not resolve the underlying design question. Before changing production behavior, the project needs to select a price-formation model and define the economic invariants that model must preserve.

This todo is separate from `todos/004-pending-p1-design-trade-commitments-and-market-liquidity.md`: this item covers how bids and asks are formed, while todo 004 covers reservations, simultaneous trader contention, settlement guarantees, and market liquidity policy. The two designs must eventually integrate.

## Findings

- Current midpoint prices are anchored to manually authored base prices and local inventory versus target, with a fixed 90% market bid and 110% market ask.
- Quotes do not account for actual input acquisition cost, recipe yield, operating cost, available purchasing funds, reserved funds, or cargo already in transit.
- An untargeted output uses an implicit target of one, so increasing inventory from one to two units can push its ask to the maximum surplus discount.
- Under current scarce input bids, immediate-sale recipe margins are approximately:
  - Structural Alloy: -¤4
  - Ceramic Composite: -¤9
  - Biopolymer: -¤8
  - Industrial Machinery: +¤13
  - Habitat Modules: +¤22
  - Reactor Assemblies: +¤63
- With two units of untargeted output inventory, margins become approximately:
  - Structural Alloy: -¤17
  - Ceramic Composite: -¤26
  - Biopolymer: -¤23
  - Industrial Machinery: -¤33
  - Habitat Modules: -¤33
  - Reactor Assemblies: +¤2
- The current market can advertise demand that it cannot settle. At ¤13 per unit, a market with ¤313 can fund only 24 units, not a 30-unit cargo.
- A mock 15% acquisition-cost floor prevents accounting losses if every output sells at that floor, but it assumes buyer liquidity and inventory clearing. It does not demonstrate viable price discovery or equilibrium.
- More exhaustive tests of the same assumed equations would increase confidence in arithmetic without answering whether those equations represent the desired economy.

## Proposed Solution

Select a price-formation model before implementing additional pricing tests or changing ECS behavior.

**Approach A — Cost-aware automated market maker:**
- Track weighted acquisition cost or inventory lots per market and good.
- Transfer consumed input cost into recipe outputs.
- Set producer asks from cost basis, operating cost, and desired margin.
- Use scarcity as an adjustment above a sustainable floor rather than as the sole anchor.
- Bound input bids by expected downstream output revenue and market operating reserves.

**Approach B — Budget-backed bids and producer asks:**
- Producers post asks based on cost and margin.
- Consumers post finite quantities and maximum bids backed by available funds.
- Trade occurs only where bid and ask overlap.
- This aligns naturally with contracts/reservations but requires explicit matching and expiry rules.

**Approach C — Adaptive historical pricing:**
- Use weighted acquisition cost and realized sale prices as moving anchors.
- Adjust future bids and asks based on fill rate, inventory, and realized margin.
- This may produce more organic dynamics but is harder to reason about and tune deterministically.

**Why a design decision is needed:**
- Cost floors protect sellers but can create unsold inventory if buyers cannot support the price.
- Revenue-aware bids require an expected output value, which can become circular once tertiary support feeds extraction.
- An order-book model changes the role of the current aggregate market and overlaps with commitment design.
- Authored base prices may remain useful as bootstrap/reference values, but should not silently guarantee solvency.

**Trade-offs / risks:**
- Weighted-average costs are simpler but hide differences between cheap and expensive inventory lots.
- Lot accounting is more accurate but increases state and transaction complexity.
- Cost-plus pricing can suppress loss-making trades but does not guarantee demand.
- Adaptive pricing can oscillate or converge slowly.
- Circular production chains need bootstrap values or historical anchors to avoid recursive price calculation.

## Recommended Action

Keep this todo pending until the project chooses the intended market model. Do not invest further in broad equation-only mock coverage first.

Next design session should:

1. Decide whether markets remain automatic market makers or move toward finite bids/asks.
2. Define the source of an output's cost basis, including initial inventory, free sources, fuel, upkeep, and multi-output recipes.
3. Define whether selling below cost is forbidden, permitted under explicit liquidation rules, or controlled by market policy.
4. Define how expected output revenue constrains processor input bids without introducing unstable recursive pricing.
5. Define operating reserves and how available cash limits advertised demand.
6. Coordinate funded quantities and reserved cash with todo 004's commitment model.
7. Only then implement an experimental pricing mode and compare it against the existing model using the permanent economy diagnostics.

## Technical Details

**Affected files/assets:**
- `crates/game-core/src/lib.rs` — quote calculation, market state, transactions, recipes, and diagnostic snapshots.
- `crates/game-core/tests/pricing_model_mock.rs` — existing limited equation experiment and captured baseline margins.
- `crates/game-cli/src/main.rs` — economy diagnostic output for model comparisons.
- `crates/game-content/src/lib.rs` — validation for any new pricing policy configuration.
- `content/economy_config.ron` — likely home for selected pricing mode, margins, reserves, and adjustment rates.
- `content/goods.ron` — authored reference prices or bootstrap values.
- `archive/market-trading-prototype/docs/initial-prototype.md` — selected price-formation behavior and invariants.

**Related systems:**
- Market bids and asks
- Recipe cost propagation
- Trader opportunity scoring
- Market liquidity and reservations
- Fuel and tertiary-support closed loops
- Long-run economy diagnostics

**Data/content impact:**
- Save data affected? Unknown; future persisted markets may need inventory cost basis and order state.
- Serialized assets or prefabs affected? No.
- Migration or content reimport needed? Likely for new economy configuration fields.

## Resources

- **Review/PR/changeset:** Commit `c933afb` on `experiment/cost-aware-pricing`
- **Related issue/card:** `todos/004-pending-p1-design-trade-commitments-and-market-liquidity.md`
- **Log/capture:** `cargo run -p game-cli -- --economy-diagnostics 350`
- **Documentation:** `archive/market-trading-prototype/docs/initial-prototype.md`
- **Similar pattern:** `crates/game-core/tests/economy_loop_mock.rs`

## Acceptance Criteria

- [ ] The project selects and documents an intended price-formation model.
- [ ] Producer asks have an explicit relationship to input acquisition cost, recipe yield, operating cost, and margin policy.
- [ ] Consumer bids have an explicit relationship to expected downstream value and available funds.
- [ ] Advertised demand quantity cannot exceed available and unreserved purchasing capacity.
- [ ] Initial inventory and free/generated resource cost bases are defined.
- [ ] Cost propagation through single-output, multi-output, and consuming recipes is defined.
- [ ] Liquidation or below-cost sale behavior is explicit rather than an accidental result of inventory targets.
- [ ] Circular production chains have a non-recursive bootstrap or historical price anchor.
- [ ] Pricing arithmetic remains deterministic and overflow-safe.
- [ ] Experimental and existing pricing modes can be compared over identical deterministic content.
- [ ] Diagnostics report realized input cost, output revenue, margin, inventory accumulation, and funded demand.
- [ ] A 1,000-tick ECS experiment evaluates processor solvency, tertiary/source cash flow, trade activity, and stationary-laden NPCs.
- [ ] The final pricing model is integrated with the commitment and liquidity decisions from todo 004.

## Work Log

### 2026-07-10 - Capture pricing-model decision

**By:** OpenAI

**Actions:**
- Reproduced current quote margins from the authored goods and recipes.
- Added a limited isolated test for current scarcity margins, a cost-floor hypothesis, and funded quantity arithmetic.
- Stopped before adding cost-basis state or changing ECS quote behavior.
- Split price formation into this dedicated todo rather than expanding the contract/liquidity todo.

**Learnings:**
- The current model is useful for creating visible scarcity differences but cannot enforce production solvency.
- A cost floor is an invariant candidate, not a complete market model.
- More robust tests should follow the model decision rather than substitute for it.

### 2026-07-12 - Superseded by cost-aware energy pricing

**By:** OpenAI

**Actions:**
- Replaced the isolated pricing mock with production embodied-energy cost basis, sustainable asks, processor bid ceilings, funded demand, and scarcity A/B diagnostics under todo 006.
- Validated non-negative structural processor margins and exact physical-energy reconciliation over 1,000 ticks.

**Learnings:**
- Cost-aware pricing must be integrated with physical funding and settlement; a standalone cost floor cannot prove buyer liquidity or processor solvency.

## Notes

- Superseded by todo 006 and `archive/market-trading-prototype/docs/energy-economy.md`.
- Preserve the current scarcity model as a comparison baseline until an alternative is validated.
- Do not treat the 1,000-cycle cost-floor mock as evidence of buyer liquidity or market equilibrium.
- The unrelated untracked `.obsidian/` directory must remain untouched.
