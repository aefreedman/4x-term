---
status: pending
priority: p1
issue_id: 004
tags: [economy, npc-traders, simulation, design]
dependencies: []
---
# Design Trade Commitments and Market Liquidity

## Problem Statement

The deterministic economy eventually deadlocks because NPC traders can commit to apparently profitable deliveries without reserving destination demand or purchasing power. Multiple traders can select the same opportunity before any of them arrives. If the destination market cannot afford a trader's complete cargo stack, the atomic sale fails; the trader retries the same sale forever and cannot reroute while laden.

This needs an explicit design for trade commitments, competing trader decisions, and market liquidity rather than a narrow partial-sale patch. The design must preserve deterministic simulation and keep the headless core independent from the frontend.

## Findings

- A 1,000-tick diagnostic run reproduced the stall deterministically. Trade activity stopped around tick 300 and remained at zero through tick 1,000.
- At tick 300 all nine NPC traders were stationary, all nine carried 30-unit cargo stacks, and none could complete a sale.
- System 16 had ¤313 while traders attempted sales including 30 Hydrocarbon at ¤13/unit (¤390 total) and 30 Ceramic Composite at ¤40/unit (¤1,200 total).
- System 17 had ¤396 while traders attempted to sell 30 Hydrocarbon at ¤16/unit (¤480 total).
- `crates/game-core/src/lib.rs` attempts to sell an NPC's entire first cargo stack, discards the `sell` error, and unconditionally continues. Laden traders therefore never reach idle repositioning logic.
- Destination selection ranks unit profit per travel tick but does not account for destination funds, other traders selecting the same opportunity, or cargo already in transit.
- Checking destination funds only when buying would reduce some failures but would not resolve races between traders choosing the same market before arrival.
- Allowing partial sales or market debt could prevent the immediate freeze, but each changes economic behavior and does not by itself define how simultaneous demand is allocated.
- Production stops after logistics lock up because processors no longer receive required inputs; this is a downstream symptom rather than the initiating failure.

## Proposed Solution

A design decision is required before implementation. Evaluate these approaches together rather than treating them as mutually exclusive quick fixes.

**Approach A — Sale contracts with reservations:**
- A trader accepts a contract before departure for a good, quantity, destination, and price or pricing rule.
- The destination reserves demand and either reserves currency, escrows payment, or guarantees settlement through an explicit credit policy.
- Contract state defines fulfillment, partial fulfillment, expiration, cancellation, rerouting, and what happens if production or inventory changes while cargo is in transit.
- Opportunity selection considers outstanding contracts so multiple traders cannot all claim the same demand.

**Approach B — Intent or capacity reservations without fixed-price contracts:**
- Reserve destination purchasing capacity and quantity while allowing the final price to float within defined rules.
- This is lighter than a full contract system but still needs expiration and failure semantics.

**Approach C — Market debt or credit limits:**
- Permit markets to complete committed purchases using bounded debt or a shared/system treasury.
- Define credit limits, repayment, insolvency, and whether debt affects quotes or production.
- Debt can support contracts but should not be used to hide unlimited or duplicated demand.

**Approach D — Partial sale and cargo recovery fallback:**
- Sell only what a market can currently afford, then reroute remaining cargo when no additional unit can be sold.
- This is useful defensive behavior even with contracts, but it does not solve simultaneous opportunity selection on its own.

**Why this needs design work:**
- The chosen model determines whether markets are hard-budget actors, credit-backed infrastructure, or merely pricing mechanisms.
- Reservation timing affects determinism, agent fairness, price discovery, and the meaning of market inventory targets.
- A robust solution must specify both normal settlement and failed/expired commitment behavior.

**Trade-offs / risks:**
- Full contracts provide clear guarantees but add lifecycle state and can reduce price responsiveness.
- Floating-price reservations preserve market movement but expose traders to price risk and require settlement bounds.
- Debt improves liquidity but can remove meaningful scarcity if limits and repayment are weak.
- Partial sales are simple but can fragment cargo and cause repeated rerouting or congestion.

## Recommended Action

Keep this todo pending until the economic actor and commitment model is selected. Before coding:

1. Define whether markets have hard budgets, bounded credit, or guaranteed treasury backing.
2. Define the commitment lifecycle from opportunity discovery through reservation, travel, settlement, cancellation, and expiry.
3. Define deterministic allocation when multiple traders seek the same demand in one simulation tick.
4. Decide whether prices are fixed at commitment, bounded, or calculated at settlement.
5. Define fallback behavior for unfulfilled or partially fulfilled cargo.
6. Prototype the selected model with a focused multi-trader contention test before integrating it into the repository economy.

## Technical Details

**Affected files/assets:**
- `crates/game-core/src/lib.rs` — NPC opportunity selection, cargo sale handling, market funds, events, and deterministic scheduling.
- `crates/game-content/src/lib.rs` — validation if contract, credit, or liquidity parameters become authored configuration.
- `content/economy_config.ron` — likely home for market credit, reservation, expiry, or settlement policy parameters.
- `content/economy.ron` — possible per-market liquidity or contract-capacity overrides.
- `docs/initial-prototype.md` — document the selected transaction and commitment model.

**Related systems:**
- Automated trader decision-making
- Market pricing and currency conservation
- Production input logistics
- Ordered deterministic simulation steps
- Event and diagnostic reporting

**Data/content impact:**
- Save data affected? Unknown; persistence is not implemented, but future contract state would need serialization.
- Serialized assets or prefabs affected? No.
- Migration or content reimport needed? Possibly for new economy configuration fields.

## Resources

- **Review/PR/changeset:** PR #3 — economy-flow balancing
- **Related issue/card:** None
- **Log/capture:** Run `cargo run -p game-cli -- --economy-diagnostics 1000` for permanent 50-tick activity and final cash-flow diagnostics.
- **Documentation:** `docs/initial-prototype.md`
- **Similar pattern:** `crates/game-core/src/lib.rs` atomic market transaction validation

## Acceptance Criteria

- [ ] The design states whether markets use hard budgets, bounded debt, escrow, or treasury backing.
- [ ] The design specifies deterministic resolution when multiple traders select overlapping demand in the same tick.
- [ ] Reservation or contract quantity cannot exceed the demand and purchasing capacity promised by the destination policy.
- [ ] Commitment price timing and price-risk rules are documented.
- [ ] Cancellation, expiration, partial fulfillment, and rerouting behavior are defined.
- [ ] NPC sale failures are handled explicitly rather than discarded.
- [ ] A contention test covers multiple traders pursuing the same opportunity on the same tick.
- [ ] A liquidity test covers a destination that cannot pay for all arriving cargo.
- [ ] A long-run deterministic test demonstrates continued trade activity beyond tick 300 and through at least tick 1,000.
- [ ] No NPC can remain permanently stationary solely because its full cargo stack cannot be sold atomically.
- [ ] Currency and goods conservation rules remain explicit and tested under the selected credit/contract model.
- [ ] Designer-facing liquidity and commitment parameters are validated and documented if introduced.

## Work Log

### 2026-07-10 - Reproduce and isolate economy stall

**By:** OpenAI

**Actions:**
- Ran a deterministic 1,000-tick diagnostic with 50-tick activity windows.
- Inspected NPC travel, cargo, transaction totals, destination market balances, and current buy quotes at the stall.
- Traced the failure to full-stack atomic sales whose errors are discarded by NPC update logic.
- Compared partial sales, reservations/contracts, and market debt as distinct design directions.

**Learnings:**
- The stall is deterministic and begins once all traders hold cargo that their destination markets cannot afford in full.
- Idle repositioning cannot help because laden traders unconditionally retry the failed sale path.
- Destination-fund checks alone are insufficient because multiple traders can select the same opportunity before settlement.

### 2026-07-10 - Add permanent economy diagnostics

**By:** OpenAI

**Actions:**
- Added cumulative per-market accounting for currency paid/received, traded units, source output, recipe throughput, and tertiary consumption.
- Added `--economy-diagnostics <ticks>` with 50-tick activity windows, currency conservation totals, final market cash flows, and NPC state summaries.
- Added tests proving successful and rejected transactions update diagnostic ledgers correctly and source/recipe counters follow deterministic execution.

**Learnings:**
- The retained 350-tick report confirms total currency remains ¤210,000 while it concentrates away from Systems 16 and 17.
- At the stall, Systems 16 and 17 have cumulative net trade cash flows of approximately ¤-9,687 and ¤-9,604 respectively, while all nine NPCs are stationary with cargo.

### 2026-07-10 - Compare closed-loop economy mocks

**By:** OpenAI

**Actions:**
- Added `crates/game-core/tests/economy_loop_mock.rs` as a production-independent stock-flow experiment.
- Compared baseline, abstract trader fuel/service spending, tertiary support goods consumed by extraction, and the combined loop over 1,000 cycles.
- Kept the mock outside ECS gameplay behavior so its assumptions can be revised without committing to a feature design.

**Learnings:**
- Under the mock's documented fixed-price assumptions, baseline liquidity lasts 166 cycles, fuel alone 200, tertiary support alone 333, and the combined loop all 1,000 cycles.
- Fuel and tertiary support address complementary cash-flow imbalances: trader operating costs return trader profits, while extraction upkeep gives source systems expenses and tertiary systems revenue.
- The result is a hypothesis check only; it does not address dynamic pricing, simultaneous trader contention, contract settlement, or bootstrap reserves.

## Notes

- Do not implement a partial-sale-only fix without first deciding the market commitment and liquidity model.
- Preserve the existing validate-before-mutate transaction guarantees.
- The unrelated untracked `.obsidian/` directory must remain untouched.
