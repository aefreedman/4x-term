---
title: Physical Energy Logistics Implementation Plan
type: feature
date: 2026-07-14
revised: 2026-07-15
status: ready
---
# Physical Energy Logistics Implementation Plan

## Executive Summary

Replace ordinary Energy-for-Energy market trading with source-backed **Energy delivery contracts**. A source consigns physical surplus Energy, a carrier transports it in a dedicated bulk hold, the shipment reimburses the loaded route burn and pays a small carrier fee in kind, and the destination receives the strictly positive remainder.

Energy remains all of the following:

- The economy's unit of account.
- A physical market inventory good.
- Trader working capital and drive fuel.
- A production and life-support input.
- A delayed, capacity-bounded inter-system shipment.

It is never bought with itself. The player-facing explanation is:

> Energy is the unit of account. It is never bought or sold — only generated, moved, and burned.

This plan is implementation-ready. Economic constants still require Phase 0 fixture tuning, but the runtime ownership, arithmetic, lifecycle, deterministic ordering, command boundary, failure recovery, and phase placement are fixed below. Changes to those contracts should update this plan rather than being improvised during implementation.

System-controlled service fleets are intentionally excluded. Commercial Energy logistics must pass its own deterministic and long-run gates with no service fleets present. The later resilience layer is tracked in `docs/plans/2026-07-14-feature-system-service-fleets-plan.md`.

This is a full replacement of ordinary `core:energy` bid/ask trading. No save migration is required because persistence does not exist. No new crate or dependency is needed.

## Goals

- Move physical Energy between systems without making a deficient destination spend more Energy than it receives.
- Make commercial Energy hauling low-margin and volume-driven.
- Give Energy dedicated bulk capacity without introducing generalized cargo mass or volume.
- Keep tank Energy, trader-owned bulk Energy, and contract-locked Energy physically and visibly distinct.
- Preserve checked arithmetic, validate-before-mutate transitions, exact Energy reconciliation, deterministic contention, reserves, claims, anti-strand guarantees, and headless-core ownership.
- Let NPCs compare Energy contracts and ordinary goods work through one canonical profit-per-tick utility.
- Let players inspect and accept the same commercially viable contracts through typed commands and immutable views.
- Attribute unserved Energy demand to actionable logistical causes.

## Non-Goals

- System service fleets or guaranteed rescue.
- Player-owned spot Energy arbitrage outside contracts.
- Tank-to-bulk or market-to-owned-bulk loading.
- Ship construction, shipyards, technology trees, maintenance, or crews.
- Generalized cargo mass or volume.
- Diplomacy, factions, tariffs, reputation, piracy, interception, combat, or insurance.
- Multi-stop, multi-source, or multi-destination contracts.
- Loading or unloading throughput.
- Runtime route-topology mutation.
- More than one active Energy contract per carrier.

A critical system may still fail because no source has surplus or no carrier finds a route commercially viable. That is acceptable when the reason is visible and correctly attributed.

## Terminology

Two percentages must never be conflated:

- **Carrier fee** — the authored, brownout-scaled profit percentage, `carrier_fee_bps`. This is the tuning input.
- **Freight rate** — the all-in carrier allocation (loaded-route reimbursement plus fee) as a fraction of gross payload. This is the displayed and viability-tested result.

Other terms:

- **Gross payload** — all contract-locked Energy loaded at the source.
- **Loaded route** — source-to-destination route. Its planned burn is reimbursed.
- **Deadhead route** — carrier-to-source pickup route. Its burn is the carrier's unreimbursed acquisition cost.
- **Net delivery** — gross payload minus loaded-route reimbursement and carrier fee.
- **Recovery reserve** — a contingent portion of the remaining locked cargo kept available to reimburse destination-to-source recovery burn while a delivery is incomplete.
- **Source claim** — non-physical protection of gross payload while a carrier deadheads to the source.
- **Inbound commitment** — expected undelivered net Energy from an accepted non-recovering contract. It suppresses duplicate requests but does not reserve storage.

## Core Invariants

1. **Physical reconciliation:** Generation, external inflow, life support, source/production/investment/travel burn, ordinary curtailment, recovery curtailment, and final physical stores reconcile exactly.
2. **Single spending store:** Only trader tank Energy pays for goods or powers travel.
3. **Locked ownership:** `ContractLocked(contract_id)` Energy cannot be spent, burned directly, redirected, manually transferred, liquidated, or cancelled into trader ownership.
4. **One physical lot:** Every loaded active contract has exactly one corresponding locked lot on its carrier. The lot owns remaining cargo quantity; the contract does not duplicate that mutable amount.
5. **One active contract:** A carrier has at most one active Energy contract.
6. **One claim owner:** Pre-load source claims live canonically in the active contract resource and are summed from it. Markets do not maintain a second mutable copy.
7. **No destination storage reservation:** Inbound commitments affect request sizing only. They never block generation, ordinary deposits, or another arrival.
8. **Recovery reserve:** Every incomplete delivery retains enough locked Energy to reimburse its accepted recovery route.
9. **Exactly-once accounting:** Reimbursement, fee conversion, source-claim release, lot removal, terminal events, and diagnostic counters apply exactly once.
10. **Stable ordering:** No ECS query iteration order selects contract acceptance, settlement, timeout, or recovery outcomes.
11. **Single settlement authority:** Energy contracts never call ordinary funded-sale settlement. One contract transition executor owns loading, delivery, allocation conversion, timeout, and recovery.
12. **Validate before mutate:** Each transition reads all affected state, prepares checked next values and ledger deltas, and only then applies them atomically and emits events.

## Resolved Design Decisions

### D1. Source offers are protected, curtailment-graded consignments

A market offers Energy only from physical stock remaining after existing protections. At the post-production matching phase:

```text
protected = ordinary_payment_claims
          + active_preload_export_claims
          + operating_reserve
          + protected_liquidation_budget
          + export_reserve

exportable = max(0, stock − protected)
```

Definitions:

- `ordinary_payment_claims` is the market's existing `reserved_energy` for funded ordinary-goods purchases.
- `active_preload_export_claims` is summed from active contracts whose source claim has not yet loaded or released.
- `operating_reserve` reuses the existing core helper and already protects life support plus scheduled source/recipe operation over the authored reserve horizon. Do not subtract a second life-support reserve.
- `protected_liquidation_budget` is the existing anti-strand protection.
- `export_reserve` is a new non-negative Energy logistics policy value.

Curtailment pressure is projected from the phase-10 post-investment snapshot. Clone the market's current source/recipe throughput carries, then simulate each tick `k` in `1..=W` in the same physical order as generation and operating burn:

```text
projected_stock = phase-10 stock
projected_glut  = 0

for k in 1..=W:
    generated = effective seasonal generation at tick t+k
                using the phase-10 post-investment generation base
    gross = projected_stock + generated
    tick_glut = max(0, gross − phase-10 energy_storage_cap)
    stored = min(gross, phase-10 energy_storage_cap)

    life = phase-10 population × life-support burn per capita
    operating = scheduled source/recipe burn from the cloned carries,
                advanced once under the phase-10 stage/labor profile
    projected_stock = max(0, stored − life − operating)
    projected_glut += tick_glut

offered_payload = min(exportable,
                      projected_glut + authored_export_base)
```

`W` is `curtailment_projection_window`. Population, brownout stage, labor profile, storage capacity, later investments, trades, and contracts are frozen at the phase-10 snapshot; the phase-14 population/labor update is not previewed. Scheduled operating burn is conservative: it assumes the carry-derived scheduled source/recipe throughput consumes its operating Energy even if a later input shortage would prevent execution. All accumulation uses checked wide signed intermediates before conversion to non-negative `Energy`. Speculative trade or contract outflows are not included because their claims are protected explicitly.

Consequences:

- A system far from curtailment with `authored_export_base = 0` offers nothing.
- A needy destination cannot manufacture supply by increasing urgency.
- Accepting a remote pickup creates a source claim that reduces later offers, ordinary purchase funding, and direct withdrawals.
- Mandatory life support and scheduled production are not blocked by a source claim. After those phases, a pre-load claim that is no longer safe is revoked deterministically before loading.

All subtraction is saturating at zero only after checked wide accumulation. Negative Energy values never enter runtime definitions.

### D2. Carrier fees follow the existing four-stage brownout ladder

`carrier_fee_bps` is authored for `Normal`, `Throttled`, `Emergency`, and `Starvation`. The schedule must be strictly increasing and each value must be below 10,000 basis points. `Normal` here means whatever variant `game-core` already uses for the healthy non-brownout state; reuse that exact enum rather than introducing a parallel stage type or name.

For gross payload `P` and planned loaded-route burn `B`:

```text
carrier_profit          = floor(P × carrier_fee_bps / 10,000)
carrier_allocation      = B + carrier_profit
net_destination_delivery = P − carrier_allocation
effective_freight_bps   = ceil(carrier_allocation × 10,000 / P)
```

The core stores integer terms. Presentation may format `effective_freight_bps` as a percentage, but the TUI must not reconstruct economic arithmetic from floats.

The fee is captured at acceptance and never changes when the destination's stage later changes.

### D3. Destination demand, inbound suppression, and gross payload sizing are exact

A market publishes commercial Energy demand only when it has an authored/current `core:energy` target. Markets without an Energy target do not publish a request in this slice.

At matching time:

```text
committed_inbound_net = sum(remaining undelivered net Energy for active
                            DeadheadingToSource, InTransit, and Arrived
                            contracts targeting this market)
remaining_requested_net = max(0,
                              energy_target − current_stock
                              − committed_inbound_net)
```

`Recovering` contracts are not inbound commitments. Revoked, cancelled, completed, and recovered contracts are absent from the active resource and therefore contribute nothing.

For each carrier/source/destination candidate, let `H` be remaining deadhead ticks plus loaded-route ticks; deadhead ticks are zero when the carrier is already at the source. Starting from the phase-10 destination stock, project ticks `t+1..t+H` using D1's frozen snapshot and cloned-carry algorithm: add that tick's seasonal generation, cap at the phase-10 storage capacity, then saturating-subtract frozen life support and carry-derived scheduled operating burn.

`prior_inbound_at_arrival` is the remaining undelivered net of existing non-recovering contracts whose stored route progress gives an expected destination arrival no later than `t+H`; an already `Arrived` contract always qualifies. Later-arriving commitments still suppress `remaining_requested_net` but do not consume this candidate's projected arrival headroom.

```text
projected_arrival_headroom = max(0,
    phase-10 energy_storage_cap
    − projected_physical_stock_at_t_plus_H
    − prior_inbound_at_arrival)

candidate_net_cap = min(remaining_requested_net, projected_arrival_headroom)
```

This is a checked signed-wide sizing projection only. It does not mutate forecast state or create a hard storage claim. The projection depends only on the destination and the horizon `H`, not on the carrier or source, so implementations may memoize it per `(destination, H)` within one matching phase rather than recomputing it per candidate triple; this is the one hot loop in matching and the memoization must not change any result.

The payload helper chooses the **largest gross integer payload** satisfying all of the following:

- `P <= offered_payload` after existing source claims.
- `P <= carrier free bulk headroom`.
- `net_destination_delivery(P) <= candidate_net_cap`.
- `net_destination_delivery(P) > planned_recovery_burn`.
- `effective_freight_bps(P) <= max_allocation_bps`.
- `carrier_profit(P) > deadhead_burn` for a commercially eligible carrier.
- The carrier tank at acceptance covers `deadhead_burn + planned_loaded_route_burn`.
- The carrier tank capacity covers `planned_recovery_burn`.
- Source, destination, and routes are distinct/valid as required.

The gross-to-net function is monotonic under floor fee rounding. Implement sizing as a checked monotonic helper with explicit boundary tests; do not use unbounded increment loops.

NPCs choose the maximum viable payload. A player command supplies an exact positive gross payload no greater than the advertised maximum; stale or oversized requests reject atomically and report the newly computed maximum/blocker.

`max_allocation_bps` is authored globally with an optional destination override, must be in `1..10_000`, and must exceed every fee schedule entry that can apply at that destination.

### D4. Routes are immutable snapshots and only the loaded route is reimbursed

Runtime route topology is immutable for this slice. Dynamic edge removal, partition recovery, and route invalidation are out of scope.

At candidate evaluation, store:

- Deadhead route, burn, and ticks from the carrier's current system to the source; zero when already at the source.
- Loaded route, burn, and ticks from source to destination.
- Recovery route, burn, and ticks from destination back to source.

`planned_loaded_route_burn` is reimbursed and fixed at acceptance. Realized movement follows the stored route. Any future rerouting feature would be at the carrier's expense unless this contract is revised.

The carrier's expected commercial return is:

```text
carrier_net_profit = carrier_profit − deadhead_burn
opportunity_ticks  = max(1, deadhead_ticks + loaded_route_ticks)
opportunity_score  = carrier_net_profit × 1,000,000 / opportunity_ticks
```

Loaded-route burn is not subtracted again because the contract reimburses it. Settlement waiting time is excluded from the initial canonical score; retry/timeout frequency is measured and may justify a later scoring revision.

A commercially eligible Energy contract requires strictly positive `carrier_net_profit`. Player and NPC carriers share this rule. Strategic loss-making delivery belongs to the later service-fleet feature.

### D5. Traders have tank Energy and a typed bulk hold

| Store | Purpose | Spendable? | Powers travel? | Capacity |
| --- | --- | --- | --- | --- |
| Tank | Working capital, proceeds, drive fuel | Yes | Yes | Existing `energy_tank_capacity` |
| Bulk hold: `Owned` | Carrier allocations received beyond tank headroom | No | No | New `bulk_energy_capacity` |
| Bulk hold: `ContractLocked(id)` | Remaining physical contract payload | No | No | Shared bulk capacity |
| General cargo | All non-Energy goods | No | No | Existing `cargo_capacity` |

Runtime representation should be equivalent to:

```text
BulkEnergyHold {
    owned: Energy,
    locked: Option<LockedEnergyLot { contract_id, amount }>,
}
```

An `Option` is intentional because this slice permits only one active contract per carrier. The physical used amount is `owned + locked.amount_or_zero`. `core:energy` no longer appears in the generic cargo map and consumes zero general cargo slots.

Allowed exact player transfers while docked:

- Tank → current market, subject to market storage headroom and refuel policy.
- Current market → tank, subject to tank headroom, refuel policy, and D1 exportable surplus.
- Owned bulk → tank, subject to tank headroom.
- Owned bulk → current market, subject to storage headroom.

Commands request an exact positive amount and reject without mutation when the amount cannot fit. Automatic partial behavior is reserved for ordinary sale settlement and Energy contract arrival settlement.

There is no tank → owned bulk or market → owned bulk command in this slice. That prevents a second player spot-hauling settlement path. Owned bulk originates only from carrier allocation/recovery reimbursement overflow.

Ordinary goods sale proceeds retain existing disclosed partial settlement and never spill silently into bulk storage.

### D6. Contract lifecycle includes remote pickup but no persistent loading state

Active lifecycle states:

```text
DeadheadingToSource {
    source_claim,
    accepted_tick,
}
InTransit {
    loaded_tick,
}
Arrived {
    arrived_tick,
    settlement_deadline,
}
Recovering {
    recovery_departure_tick,
}
```

Terminal outcomes are emitted as typed events and aggregate counters, then removed from the active contract map:

- `Completed` — all net delivery settled and allocation converted.
- `CancelledBeforeLoad` — player cancelled during deadhead.
- `RevokedBeforeLoad` — source protections became unsafe before loading.
- `RejectedBeforeLoad` — atomic loading/departure validation failed.
- `RecoveredAfterFailure` — timeout recovery returned/curtailed all remaining locked Energy at the source.

There is no persistent `Matched` or `Loading` state because loading throughput is not modeled.

Acceptance behavior:

1. Fully validate and score the intent against current state.
2. Allocate a monotonic `ContractId` only for a successful accepted transition.
3. If the carrier is already at the source, atomically subtract source stock, create the locked lot, burn loaded-route tank Energy, enter `InTransit`, and emit acceptance/loading/departure events.
4. Otherwise create the gross-payload source claim, burn deadhead Energy, begin the stored deadhead route, enter `DeadheadingToSource`, and emit acceptance/departure events.

During `DeadheadingToSource`:

- The source claim protects gross payload from offers, ordinary purchasing, and direct withdrawals.
- Mandatory sinks may still make claims collectively unsafe.
- At phase 6, compute each source's post-production claim capacity without subtracting the claims being allocated:

```text
claim_capacity = max(0,
    stock − ordinary_payment_claims
          − operating_reserve
          − protected_liquidation_budget
          − export_reserve)
```

- Visit that source's pre-load contracts by ascending `ContractId`, maintaining remaining claim capacity. A claim is safe only when its full gross payload fits; reserve that capacity for it, otherwise revoke it. This oldest-first allocation is the sole distress winner rule.
- If a safe carrier has reached the source, loading reduces source stock and releases the same claim in one prepared transition; the capacity already assigned to later claims does not change.
- A player cancellation releases the claim immediately and removes the contract. Commands resolve synchronously between steps, so a successful cancellation wins before the next travel/maintenance phases; after phase-6 loading it is rejected. Already-started deadhead travel continues to the source because travel itself is not cancellable. The same non-cancellable travel rule applies after `RevokedBeforeLoad`.
- On source arrival, loading and loaded-route departure execute as one prepared atomic transition. Success subtracts source stock, creates the locked lot, and releases the source claim in the same apply step.
- `RejectedBeforeLoad` is reserved for route/state/integrity validation failure after claim safety passes. Failure releases the claim and leaves the carrier docked with no locked cargo. Acceptance fuel validation guarantees the carrier retained at least the planned loaded-route burn after deadheading, preserving an escape budget.

From loading onward cancellation is rejected. Manual travel to a non-contract destination, locked transfer, liquidation, rerouting, and dynamic retirement are rejected while a carrier owns an active contract or locked lot.

### D7. Destination headroom is soft and partial settlement preserves recovery

Arrival settlement uses actual physical destination headroom. Multiple arrived contracts are processed in the stable order defined by D12.

For contract constants:

```text
P = gross payload
B = planned loaded-route reimbursement
F = carrier profit
N = net destination delivery
R = planned recovery burn
```

Mutable accounting stores only:

- `cumulative_settled`.
- The carrier's locked-lot quantity (canonical remaining cargo).

Reimbursement and fee conversion are derived, not duplicated mutable fields:

```text
reimbursement_entitled = B when cumulative_settled > 0 or state is Recovering,
                         otherwise 0
earned_fee_total        = floor(F × cumulative_settled / N)
```

Each transition calculates conversion deltas as `derived_after − derived_before`. This makes repeated retries idempotent without separate reimbursement/fee counters.

On each settlement attempt:

1. Compute `remaining_net = N − cumulative_settled`.
2. If actual headroom can absorb all `remaining_net`, settle it, convert any unconverted reimbursement and the full remaining fee, remove the empty locked lot, emit completion, and remove the contract.
3. Otherwise choose the largest positive `settle_now <= actual_headroom` for which the prepared post-transition locked lot remains at least `R` after delivery, first reimbursement conversion, and newly earned fee conversion. If no positive amount preserves `R`, make no physical change this tick.
4. Transfer `settle_now` to destination storage.
5. On the first positive settlement, convert all `B` from locked cargo to carrier ownership.
6. Compute total earned fee after the new cumulative delivery and its derived delta:

```text
fee_before = floor(F × cumulative_settled / N)
fee_after  = floor(F × new_cumulative_settled / N)
fee_now    = fee_after − fee_before
```

7. Convert `fee_now` from locked cargo to carrier ownership.
8. Converted allocation fills tank headroom first; any remainder becomes `Owned` bulk Energy.

The helper that selects `settle_now` must account for floor boundaries and prove the post-state before mutation. Retries use cumulative fields, so reimbursement and fee cannot be paid twice. The carrier never earns fee for undelivered Energy.

### D8. Timeout recovery uses the same contract and always terminates

For an arrival at tick `A` and positive `settlement_timeout_ticks = T`:

```text
settlement_deadline = A + T
```

Settlement is attempted on ticks `A .. A+T−1`. At tick `A+T`, timeout transitions to recovery **before** another settlement attempt. This gives exactly `T` settlement opportunities including the arrival tick.

The D7 invariant leaves at least `R` locked after any positive partial settlement. A zero-settlement contract still holds its full payload. At timeout:

1. If `cumulative_settled == 0`, convert the still-unconverted `B` from the locked lot to carrier ownership. If any positive settlement occurred, D7 already converted `B` and this delta is zero.
2. Convert exactly `R` from the locked lot to carrier ownership. Both conversions fill tank headroom first and place overflow in `Owned` bulk. No fee converts at timeout.
3. The carrier's tank capacity is at least `R`; its existing tank plus the converted tank portion is therefore sufficient to burn the stored recovery route.
4. Burn `R` from the tank, begin the stored destination-to-source route, and enter `Recovering` in one prepared transition.
5. Any remaining locked Energy stays on the same `ContractId`; recovery is not a newly spawned contract and earns no fee.

D3's `N > R` guarantees the zero-settlement lot can fund both `B` and `R`, because `P = B + F + N > B + R`. After a positive partial settlement, `B` is already converted and D7 guarantees at least `R` remains.

The conversion reimburses the carrier for recovery burn. If existing tank Energy contributes to the departure because the tank was already near full, the corresponding converted overflow remains owned bulk; total carrier-owned Energy is unchanged by the reimbursed recovery burn.

On recovery arrival at the source:

- Transfer as much remaining locked Energy as storage headroom allows.
- Record any excess as explicit `recovery_curtailed` Energy at that source.
- Remove the locked lot and active contract.
- Emit `RecoveredAfterFailure` and update terminal counters exactly once.

Recovery curtailment is the only capacity-shortfall exception to “cargo is never deleted.” It is a named, reconciled loss channel analogous to generation curtailment. Recovery never spawns another recovery, so every accepted contract terminates.

### D9. Players and NPCs use the same commercial rules

There is no trust or reputation system. Player carriers cannot steal consigned cargo because locked lots are inaccessible to ordinary commands. Players may cancel only during remote pickup as specified in D6; deadhead cost is sunk and movement continues.

Players cannot accept a contract with non-positive net commercial profit merely to perform strategic relief. That behavior belongs to service fleets.

### D10. Starvation attribution is mutually exclusive and exhaustive

For each destination tick with unsupplied life support, report one state in priority order:

1. `ArrivedSettlementBlocked` — an arrived contract has undelivered net Energy but cannot currently settle because of headroom/recovery-reserve constraints.
2. `AcceptedDeliveryPending` — a non-recovering inbound commitment exists but was not available for this tick's phase-3 life support. This includes in-transit contracts and an arrived, unblocked contract that can settle later in phase 7; blocked arrived contracts remain in the higher-priority category above.
3. `NoReachableSurplus` — no reachable source has positive offered payload.
4. `NoViableCandidate` — surplus exists, but no carrier/source/payload passes route, fuel, capacity, freight-rate, recovery, and positive-profit checks.
5. `ViableButUnaccepted` — a viable candidate exists but loses deterministic contention or the available carriers choose better work.

Active contracts take priority so a temporarily empty source network does not misclassify an already committed delivery as absent supply.

The headline split remains:

- **Supply starvation:** `NoReachableSurplus`.
- **Commercial/logistics starvation:** `NoViableCandidate + ViableButUnaccepted`.

Active-delivery delay and settlement blockage are reported separately rather than misclassified as absent supply or absent carrier interest.

### D11. Energy leaves every ordinary trade entry point

All ordinary quote, buy, sell, reservation, automated commitment, liquidation, and reroute entry points reject `core:energy` without mutation. This includes player commands, NPC collection, local trade limits, and laden fallback.

Allowed non-trade Energy paths remain:

- Generation and authored external delivery.
- Life support, production, investments, and travel burn.
- Exact tank/market and owned-bulk transfers from D5.
- Contract loading, allocation conversion, destination settlement, and recovery.
- Ledgered curtailment.

Contract settlement must not call `execute_funded_sale`, ordinary reservation settlement, or `RecordExternalDelivery`. A shared checked physical-transfer helper may prepare market/tank/bulk/curtailment deltas, but the contract transition executor is the sole lifecycle authority.

Remove configuration that exists only for Energy quotes:

- The `core:energy` entry in global/per-market `import_priorities`.
- `emergency_energy_bid_ceiling`.
- Any additional Energy-quote-only knob found during the Phase 2 sweep.

Ordinary goods remain Energy-denominated and retain current pricing, funded settlement, reservations, liquidation, and route subsidies.

### D12. Opportunity selection and every contested phase have a total order

#### Carrier utility selection

Each idle NPC computes canonical expected-profit-per-tick scores for both ordinary trade and Energy contracts. Energy uses D4's score. Ordinary trade retains its existing score. The NPC selects one highest-scoring intent; ties resolve by:

1. Opportunity kind (`EnergyContract` before `OrdinaryTrade` only when scores are exactly equal).
2. Source stable ID.
3. Destination stable ID.
4. Good stable ID where applicable.
5. Carrier stable ID.

Only positive-scoring opportunities are eligible. This utility choice is how the fee schedule attracts carriers without forcing them into inferior work.

A player acceptance command contributes one Energy intent and receives typed success/rejection feedback.

#### Energy intent resolution

Selected Energy intents are sorted by:

1. Destination brownout stage, most severe first.
2. Destination projected runway, ascending.
3. Gross payload, descending.
4. Destination stable ID.
5. Source stable ID.
6. Carrier stable ID.

All ordering keys are captured from the phase-10 snapshot and the list is sorted exactly once. `destination_projected_runway` uses D3's projected destination occupancy immediately before this candidate's expected arrival: it is unlimited when the frozen life-support obligation is zero, otherwise `floor(projected_occupancy / frozen_life_support_obligation)`.

Every selected intent carries an exact gross payload. After each acceptance, source protections, destination remaining request, carrier state, the current maximum payload, and every viability rule are recomputed. The original payload is accepted only if it remains valid; stale NPC/player intents are rejected rather than silently downsized or resorted. A failed Energy intent does not fall back to ordinary work in the same tick; it may choose again next tick. This keeps one deterministic resolution pass and makes contention visible.

Ordinary intents then resolve through their existing stable path using carriers not accepted for Energy work.

#### Maintenance and settlement order

- Pre-load revocation/loading: `ContractId` ascending.
- Destination settlement: destination stable ID, `arrived_tick`, then `ContractId`, ascending.
- Recovery arrival: source stable ID, recovery arrival tick, then `ContractId`, ascending.

Insertion-order permutation tests cover all three orders.

### D13. Tick phase placement is fixed

Extend `GameSession::step` in this order:

1. Advance all travel and mark contract carriers that reached source/destination/recovery source.
2. Refresh and expire ordinary reservations.
3. Generate Energy, cap storage, record ordinary curtailment, and assess life support.
4. Classify brownouts and derive operating profiles.
5. Execute sources and recipes.
6. Maintain pre-load Energy contracts in `ContractId` order: revoke unsafe claims or atomically load/depart carriers that reached the source.
7. Settle/retry/timeout Energy deliveries and settle recovery arrivals in D12 order.
8. Run existing ordinary idle-laden settlement/liquidation and NPC tank balancing.
9. Execute autonomous investments.
10. Derive Energy requests/offers and collect canonical NPC opportunity choices from the resulting state.
11. Resolve selected Energy intents in D12 order.
12. Resolve ordinary trade intents for still-idle carriers.
13. Evaluate dynamic-fleet spawn/retirement state.
14. Update populations and next-tick demand/labor effects.
15. Advance the clock and publish events/snapshots.

Reasons for this placement:

- Offers observe realized current-tick generation, mandatory burn, production, settlement, balancing, and investment.
- Arrived deliveries settle before ordinary liquidation and investment use the resulting stock.
- Newly accepted source claims protect Energy from subsequent ordinary commitments in the same tick.
- Population keeps its existing following-tick effect.

The implementation must update `docs/energy-economy.md` to make this the canonical phase order.

### D14. Runtime ownership stays in `game-core`

No new crate boundary is introduced.

`game-core` owns:

- `ContractId`: monotonic serialization-friendly integer newtype; no raw ECS `Entity` in public records or commands.
- `EnergyContracts` resource: next ID, active contracts, and aggregate logistics diagnostics. Source claims are derived from active pre-load entries.
- `PendingEnergyContractIntents` resource: transient player/NPC intents awaiting D12 resolution.
- `EnergyContract`: stable source/destination/carrier IDs, stored routes/ticks/burns, gross terms, captured fee, `cumulative_settled`, timestamps, and active state. Reimbursement and earned fee are derived from state and cumulative settlement.
- `BulkEnergyHold` on `Trader`: owned Energy and one optional locked physical lot.
- Effective per-market Energy logistics policy, compiled from content.
- Pure checked arithmetic and prepared transition helpers.
- Typed lifecycle, transfer, rejection, and recovery-curtailment events.
- Immutable snapshots for requests, offers, opportunities, active contracts, lots, blockers, and diagnostics.

Requests and offers are computed projections, never independently mutable records.

An active contract stores immutable gross terms and mutable accounting, but does not duplicate the locked lot's remaining physical quantity. Each transition asserts:

```text
active loaded contract ↔ exactly one carrier locked lot
terminal/pre-load contract ↔ no locked lot
sum source claims derived from active DeadheadingToSource records
```

Dynamic retirement/despawn is forbidden while a trader has an active Energy contract, any locked lot, owned/general cargo requiring cleanup, travel, or an ordinary reservation. Carrier profitability counts earned contract fee as revenue, deadhead burn as travel cost, loaded/recovery burn as reimbursed travel cost, and never counts reimbursement as profit.

### D15. Content ownership and validation are explicit

RON remains a source format only; `game-content` validates and compiles typed runtime definitions.

#### `economy_config.ron`

Add global Energy logistics defaults equivalent to:

```text
energy_logistics: (
  carrier_fee_bps: [normal, throttled, emergency, starvation],
  max_allocation_bps: ...,
  curtailment_projection_window: ...,
  export_reserve: ...,
  authored_export_base: ...,
  settlement_timeout_ticks: ...,
)
```

Phase 0 selected repository defaults of 50/100/200/300 fee bps, 1,000 maximum allocation bps, a 20-tick projection window, zero global export reserve/base, and a 20-tick settlement timeout. The authored fixture gives `frontier:system_15` a 3,200-Energy export-base override and full starting storage, raises `frontier:system_14`'s Energy target to 5,000, and uses the exact archetype values recorded in `docs/energy-logistics-validation.md`; these are validated content values rather than hidden code defaults.

#### `economy.ron`

Add optional per-market overrides for:

- `export_reserve` and `authored_export_base` as source policy.
- `carrier_fee_bps` and `max_allocation_bps` as destination policy.

The existing `core:energy` target marks a commercial importing market and remains the target used by D3. Remove per-market Energy import-priority overrides.

#### `traders.ron`

Add `bulk_energy_capacity` to the player profile. Replace the single homogeneous NPC physical profile with a stable-ID archetype list. Each archetype defines:

- Stable archetype ID, generated ID/name prefixes, initial count, and maximum count.
- Tank amount/capacity, bulk Energy capacity, general cargo capacity.
- Speed, burn, refuel policy, and initial distribution.

Global dynamic-fleet windows/thresholds remain shared. `FleetDynamics` compiles a registry rather than one archetype. Dynamic spawn evaluation constructs hypothetical candidates at each opportunity source using each eligible archetype's authored initial tank and physical capacities; that source must be able to fund the starting tank. It compares the canonical opportunity score, then the D12 opportunity keys, then archetype stable ID, and spawns the winner at that opportunity source. This lets an unserved bulk opportunity attract the first compatible hauler instead of requiring one to exist already. Total and per-archetype caps both apply.

At least one general freighter and one bulk-capable Energy-hauler archetype must exist in repository content after Phase 0. Exact values are tuning data.

#### Validation

Reject:

- Missing or non-strict four-stage fee schedules.
- Any bps value outside its valid range or fee not below `max_allocation_bps`.
- Zero projection window or settlement timeout.
- Negative reserves/base values or overflow in compiled Energy values.
- Zero tank/bulk capacity for an Energy-capable archetype.
- Tank capacity unable to cover any graph-adjacent route for an enabled archetype.
- Duplicate archetype IDs/prefix collisions or invalid count caps.
- Unknown per-market override references.
- Energy import-priority or quote-only policy left in repository content.
- Repository content in which no authored source can ever offer Energy or no importer has a positive Energy target; use a structured validation failure/warning only if a deliberate test fixture opts out.

### D16. Player command and immutable-view boundary is fixed

Add typed requests/commands equivalent to:

- `AcceptEnergyContract { source, destination, gross_payload }`.
- `CancelEnergyContract { contract_id }` — valid only for the player's `DeadheadingToSource` contract.
- `TransferOwnedBulkToTank { amount }`.
- `DepositOwnedBulkEnergy { amount }`.

The player carrier remains implicit as in existing buy/sell/travel commands. Commands contain stable domain IDs and quantities only, never UI or ECS IDs.

Add immutable app views for:

- Per-market request/offer: stock, cap, target, offered/requested amount, inbound commitment, runway, stage, and blocker/cause.
- Player contract opportunities: source/destination names and IDs, maximum gross payload, deadhead/loaded/recovery route facts, fee, allocation, net delivery, effective freight rate, expected net profit, score, and blocker.
- Active contract: ID, state, source/destination, route progress, locked amount, cumulative settlement, converted reimbursement/fee, deadline, recovery reserve, and latest blocker.
- Player storage: tank, owned bulk, locked bulk by contract, total bulk used/capacity, and general cargo separately.
- Aggregate D10 diagnostic counters.

`game-app` resolves names and formats factual labels; the TUI renders snapshots and submits typed requests only.

The Energy market row stops showing bid/ask and instead shows logistics state. The focused contract UI must display at least:

```text
Payload                  4,000 Energy
Deadhead burn               10 Energy
Loaded-route burn           20 Energy
Carrier fee                 40 Energy
Carrier net profit          30 Energy
Net delivery             3,940 Energy
Freight rate               1.5%
Recovery reserve             20 Energy
Destination runway       3 → 42 ticks
```

Help, encyclopedia, README, and current economy docs use the unit-of-account sentence from the Executive Summary. There must not be two live explanations of Energy trade.

## Worked Accounting Examples

### Full delivery

```text
Gross payload P             4,000
Loaded-route burn B            20
Carrier fee bps               100
Carrier profit F                40
Carrier allocation              60
Net delivery N               3,940
Deadhead burn                   10
Carrier net profit              30
Recovery burn R                 20 (contingent; unused on success)
```

Physical sequence:

1. Source stock decreases by 4,000; locked lot increases by 4,000.
2. Carrier tank and travel ledger decrease by 20 for the loaded route.
3. Destination stock increases by 3,940.
4. Carrier ownership increases by 60, restoring the 20 loaded-route burn and earning 40 fee.
5. Contract and locked lot terminate.

Ignoring the separately sunk 10-Energy deadhead leg, the 4,000 loaded units become 3,940 destination Energy + 20 travel burn + 40 carrier-owned fee.

### Partial delivery and recovery

Using the same terms, assume only 2,000 Energy can settle before timeout. D7 converts the 20 reimbursement and:

```text
earned fee = floor(40 × 2,000 / 3,940) = 20
```

The post-settlement locked lot remains at least the 20-Energy recovery reserve. At timeout, 20 converts to carrier ownership, 20 burns on the return route, and all other locked Energy returns to the source or is ledgered as recovery curtailment at source capacity. The unearned 20 fee remains in locked cargo and is not paid to the carrier.

## Main-Agent and Subagent Coordination Strategy

The main agent is the architect, test designer, integration owner, and final validator. Subagents are bounded implementers or read-only reviewers. Production implementation must not be delegated until the main agent has converted this plan's invariants into an explicit test contract for the relevant slice.

Only the root/main agent invokes subagents. Workers must not delegate further. The main agent owns interpretation of research, cross-slice decisions, plan updates, worktree creation, integration, conflict resolution, and final commits.

### Authority order

When instructions conflict, use this order and stop rather than guessing:

1. This accepted implementation plan and its D1–D16 decisions.
2. The main-agent-authored test contract and fixtures in `docs/energy-logistics-validation.md`.
3. Project architecture and current canonical economy documentation.
4. The bounded implementation handoff.
5. Worker-local implementation preferences.

A worker may propose a plan or fixture correction in its handoff, but it may not silently change expected arithmetic, weaken an assertion, reopen a resolved decision, or create an alternate settlement path.

### Main agent pre-delegation responsibilities

Before handing any production slice to an implementation agent, the main agent must:

1. **Establish a clean baseline.** Run the routine workspace tests and targeted existing economy/content tests. Record pre-existing failures separately; do not hand them to a feature worker as implicit scope.
2. **Create the validation contract.** Add `docs/energy-logistics-validation.md` containing stable test IDs, exact fixtures, expected arithmetic/state/ledger outcomes, required commands, and evidence slots.
3. **Freeze invariants for the slice.** Map every delegated task to the Core Invariants and acceptance criteria it must preserve. At minimum the validation contract must identify:
   - `EL-INV-PHYSICAL`: exact whole-world Energy reconciliation.
   - `EL-INV-LOCKED`: locked Energy is inaccessible outside the contract executor.
   - `EL-INV-LOT`: one active loaded contract corresponds to one optional locked lot.
   - `EL-INV-CLAIM`: source claims are canonical, oldest-first under distress, and release exactly once.
   - `EL-INV-RECOVERY`: every incomplete delivery preserves `R` and every recovery terminates.
   - `EL-INV-ALLOCATION`: reimbursement and fee deltas are derived and never double-paid.
   - `EL-INV-ORDER`: matching, maintenance, settlement, and recovery are insertion-order independent.
   - `EL-INV-LEGACY`: every ordinary Energy trade path rejects without mutation.
   - `EL-INV-ANTISTRAND`: every accepted carrier retains a valid loaded/recovery/escape budget.
   - `EL-INV-BOUNDARY`: frontends act only through typed commands and immutable views.
4. **Design exact test vectors before behavior.** For the slice being delegated, specify inputs and expected outputs for normal, boundary, overflow, stale, contention, and failure cases. The first validation artifact must include:
   - D1 projection and source-claim capacity tables.
   - D2/D3 floor/ceil and largest-gross boundary vectors.
   - D6 transition table with claim/lot/tank/market pre- and post-state.
   - D7 multi-retry sequences with derived reimbursement/fee deltas.
   - D8 zero-settlement and partial-settlement recovery ledgers.
   - D12 tie fixtures and insertion permutations.
   - The complete ordinary Energy entry-point rejection matrix.
   - App/TUI request, view, and typed-rejection expectations.
   - Required 1,000/10,000-tick metrics and pass/fail conditions.
5. **Create executable scaffolding where practical.** The main agent writes the shared fixture builders, assertion helpers, public type/signature skeletons, and failing or explicitly pending tests needed to make the handoff unambiguous. In Rust, tests must still compile; if an API does not exist yet, the main agent first adds the minimal signature/type scaffold or records the exact test body in the validation artifact and ports it to executable form before that behavior is accepted.
6. **Capture failure-first evidence.** For executable tests, record the expected pre-implementation failure. A test that already passes must be identified as characterization or rewritten so it actually proves the new behavior.
7. **Define file ownership and stop conditions.** Each handoff names allowed files, forbidden neighboring systems, required tests, commands, and the point at which the worker must stop and return to the main agent.

Read-only reconnaissance and review may be delegated before this gate. Production code may not.

### Test-first handoff protocol

Use this handshake for every production slice:

1. Main agent selects one bounded behavior and writes/updates its validation cases.
2. Main agent confirms the test contract is internally consistent and, when executable, observes the intended failure.
3. Main agent freezes the slice's public interfaces and allowed file set.
4. One implementation agent receives the handoff and changes only the authorized production/tests needed to satisfy it.
5. The worker may add stronger local edge tests but may not edit or delete main-owned expected values without returning a blocker.
6. The worker returns changed files, tests run, exact results, unresolved risks, and a concise invariant-by-invariant self-check.
7. Main agent reads the diff, reruns the targeted tests, verifies no assertion was weakened, and either integrates or returns a specific correction task.
8. A read-only specialist reviews high-risk accounting/lifecycle slices after the main agent's first integration pass, not instead of it.
9. Only after the slice gate is green does the main agent update plan/validation checkboxes and prepare the next handoff.

### Delegation brief template

Every implementation task should include:

```text
Goal:
One bounded behavior from the accepted Energy logistics plan.

Authoritative decisions/invariants:
Exact D-sections and EL-INV IDs.

Allowed files:
A short explicit list.

Forbidden scope:
Adjacent features and files the worker must not redesign.

Main-authored tests/fixtures:
Exact test names, expected vectors, and current failure evidence.

Required implementation result:
Observable behavior, not a broad request to “implement the phase.”

Validation commands:
Targeted fmt/test/clippy commands within the slice.

Stop conditions:
Return immediately on plan/test conflict, required public API change,
new dependency/crate need, unrelated baseline failure, or scope expansion.

Return handoff:
Changed files; tests and output; invariant self-check; blockers; no nested delegation.
```

Do not ask one worker to discover the subsystem, choose the design, implement it, and validate the whole feature. Discovery, design/test authoring, implementation, and review are separate responsibilities.

### Worktree and integration policy

- Start implementation on a dedicated feature branch, not `main`.
- The main agent creates and cleans isolated worktrees with the project `git-worktree` manager when parallel edits are genuinely independent. Subagents never create worktrees themselves.
- Give each implementation worker an explicit `cwd` and branch/worktree.
- `crates/game-core/src/lib.rs` remains a shared integration hotspot even after the bounded seam refactor below. Root integration, legacy removal, scheduling, and fleet edits must be delegated **serially**, with main-agent integration and green tests between them. Never run two agents that edit this file in parallel.
- New Energy logistics behavior belongs in `crates/game-core/src/energy_logistics/` rather than further expanding the root file. Its production module and main-owned test module are separate files.
- Parallelize only disjoint stable surfaces, such as `game-app` and `game-cli` after core snapshots freeze. `game-tui` follows `game-app`; it does not guess unfinished view contracts.
- The main agent is the sole conflict resolver and plan/validation-artifact editor during implementation.
- Workers make a conventional atomic commit in an isolated worktree only when the handoff explicitly authorizes it. The main agent reviews before integrating and owns final feature commits.
- Preserve unrelated working-tree changes. Never stage by broad path when the handoff owns only specific files.

### Bounded delegation-seam refactor

Do one narrow, behavior-preserving refactor before Phase 0. Its purpose is to separate main-owned tests from worker-owned production code and give the new feature a cohesive home—not to redesign the core architecture.

Required preparatory changes:

| Surface | Preparatory action | Ownership afterward |
| --- | --- | --- |
| `game-core/src/lib.rs` | Move the existing root `#[cfg(test)] mod tests` body to `game-core/src/tests.rs`; retain `#[cfg(test)] mod tests;` in the root. | Existing characterization/regression tests are main-agent-owned unless a handoff explicitly authorizes changes. |
| `game-core/src/energy_logistics/mod.rs` | Create the cohesive feature module and expose only the `pub(super)`/`pub(crate)` seams needed by root orchestration. | Serial core workers implement bounded Energy slices here. |
| `game-core/src/energy_logistics/tests.rs` | Create a separate compile-safe test module. | Main agent owns invariant fixtures and expected values; workers may add tests only when authorized. |
| `game-content/src/energy_logistics.rs` | Create when Phase 1 adds logistics source/compiled policy types. | Content worker owns only this module plus explicitly listed root wiring/content files. |
| `game-app/src/energy_logistics.rs` | Create when Phase 5 freezes core commands/snapshots. | App worker owns Energy requests/views/builders; actor ownership remains in the root. |
| `game-tui/src/screens/energy_logistics.rs` | Create after app views integrate. | TUI worker owns Energy logistics presentation; existing input/state boundaries remain. |

The preparatory refactor must not:

- Split all core definitions into `model.rs`, `session.rs`, or a new `systems/` hierarchy.
- Move existing ordinary trade, fleet, population, investment, or scheduling behavior merely to create parallel work.
- Add a crate, dependency, trait abstraction, Bevy schedule migration, or public API.
- Change tick order, arithmetic, events, snapshots, content behavior, or test expectations.
- Extract every app/TUI/content test module preemptively. Those files are smaller and can gain feature modules only when their contracts stabilize.

This narrow policy accepts that some root integration remains serial. Avoiding merge conflicts is not sufficient justification for a broad architecture rewrite.

### Delegation waves and dependency graph

| Wave | Owner | Slice | Dependency | Parallelism and gate |
| --- | --- | --- | --- | --- |
| -1 | Main agent | Clean baseline, extract existing core tests, create core Energy module/test seams, prove behavior unchanged | Accepted plan | No production behavior; separate mechanical commit; workspace tests and public API remain unchanged. |
| 0A | Main agent | Post-refactor baseline, repository seam verification, validation artifact, invariant IDs, exact arithmetic/transition fixtures | Integrated Wave -1 | No production delegation; read-only research/review may run in parallel. |
| 0B | Main agent | Minimal core API/type/test scaffold and failure-first evidence in the new Energy module/test files | 0A | Main-owned; freeze signatures before worker handoff. |
| 1A | One core worker | Checked arithmetic, contract/lot/config types, snapshots/events/errors | 0B | Serial `game-core`; targeted pure tests must pass. |
| 1B | One content worker | Global/per-market logistics schema and validation | Integrated 1A types | May use isolated worktree; main runs repository content tests before integration. |
| 2 | One core worker | Remove ordinary Energy paths and add exact transfer primitives | Integrated Wave 1 | Serial `game-core`; complete `EL-INV-LEGACY` matrix is the gate. |
| 3A | One core worker | Acceptance, source claim, deadhead, cancellation, atomic loading | Wave 2 | Serial; D6 transition tests and anti-strand gate. |
| 3B | One core worker | Arrival, derived allocation, retry, timeout, same-contract recovery | Integrated 3A | Serial; accounting reviewer after main integration. |
| 3C | One core worker | D12 selection/order, D13 schedule, profitability and retirement blocking | Integrated 3B | Serial; insertion permutations and ordinary-trade regression gate. |
| 4 | One core/content worker at a time | NPC archetype registry, spawn selection, content tuning | Stable 3C opportunity API | Core part remains serial; content-only follow-up may use isolated worktree. |
| 5A | App worker | Typed requests and immutable views | Core snapshots/commands frozen | Can run parallel with 5B because files are disjoint. |
| 5B | CLI worker | Ledgers, D10 diagnostics, long-run reporting | Core snapshots/diagnostics frozen | Can run parallel with 5A. |
| 5C | TUI worker | Contract, Energy-row, storage, and blocker presentation | Integrated 5A view contract | Sequential after app; both layouts and render tests gate completion. |
| 5D | Docs/content worker or main | Encyclopedia/current docs/README/CHANGELOG | Runtime and vocabulary stable | May parallelize with 5C if it does not edit shared implementation files. |
| 6 | Main agent + read-only reviewers | Cross-slice integration, full tests, architecture/spec/simplicity/lint review, long-run gates | All waves integrated | Reviews may run in parallel; fixes are assigned serially by finding. |

A wave is not complete because a worker reports success. It is complete only after the main agent independently verifies its gate on the integration branch.

### Review-agent routing

Use the most specific available reviewer after integration:

- Contract lifecycle, player/NPC flows, and edge cases: `cg-spec-flow-analyzer`.
- Runtime ownership, phase order, core/app/content boundaries: `cg-architecture-specialist`.
- Avoidable state duplication or alternate settlement paths: `cg-code-simplicity-reviewer`.
- Formatting, Clippy, and bounded static checks: `cg-lint-specialist`.
- Repository/content reconnaissance only when a slice still has a narrow evidence question: `cg-repo-researcher`.
- Ordinary implementation when no more specific coding agent exists: a bounded `general` worker.

The main agent must call `subagent_list` at implementation time because available agents/models can change. Choose the least costly model/thinking profile that reliably fits the bounded slice; use stronger reasoning for accounting, lifecycle, and cross-system review. Reviewer output is evidence for the main agent, not authority to change the plan silently.

### Per-slice and final gates

At each handoff boundary, the main agent runs:

- `git diff --check` and a focused diff review.
- `cargo fmt --all -- --check` when Rust changed.
- The exact main-authored targeted tests for that slice.
- Relevant neighboring regression tests named in the validation artifact.
- `cargo clippy` for the affected crate once the slice is behaviorally green.
- Invariant checks that compare pre/post physical Energy and claim/lot state.

Before final completion, the main agent runs the full workspace tests, workspace Clippy, content validation, insertion-permutation suites, 1,000-tick deterministic gate, 10,000-tick diagnostics, player-impact gate, and manual player contract/storage flows. Final evidence goes in `docs/energy-logistics-validation.md` and the required concise world-dynamics evidence goes in `docs/world-dynamics-validation.md`.

## Implementation Sequence

Each phase should be a focused, testable change governed by the coordination strategy above. Do not postpone exact accounting, old-path rejection, or observability until after the runtime is enabled.

### Phase -1: Main-owned delegation seam preparation

This is a mechanical refactor only. Complete and commit it separately before authoring new behavioral tests or delegating production implementation.

- [x] Record the clean pre-refactor output of `cargo fmt --all -- --check`, `cargo test --workspace`, and existing targeted economy/content tests.
- [x] Move the existing `game-core/src/lib.rs` root test module to `game-core/src/tests.rs` without changing test names, bodies, visibility, ordering assumptions, or expectations.
- [x] Leave only `#[cfg(test)] mod tests;` as root wiring and preserve all root public re-exports/API paths.
- [x] Create compile-safe `game-core/src/energy_logistics/mod.rs` and `game-core/src/energy_logistics/tests.rs` seams with no runtime behavior and no placeholder economic decisions.
- [x] Document in module comments that root scheduling remains authoritative and only the contract executor may mutate Energy logistics state.
- [x] Confirm no ordinary production code, tick phase, content value, event, snapshot, or ledger changed.
- [x] Run `git diff --check`, formatting, workspace tests, affected-crate Clippy, and the same targeted economy/content tests recorded before extraction.
- [x] Commit the mechanical seam preparation separately so later behavioral diffs remain reviewable.

Files: `crates/game-core/src/lib.rs`, `crates/game-core/src/tests.rs`, `crates/game-core/src/energy_logistics/mod.rs`, `crates/game-core/src/energy_logistics/tests.rs`.

The expensive 1,000/10,000-tick gates are not required solely for a byte-for-byte test move and empty module wiring. Any accidental production change invalidates that exemption and requires the normal economy gates.

### Phase 0: Main-agent test contract, deterministic economics, and content fixture

This phase is owned by the main agent. No production behavior is delegated until its gate is complete.

- [x] Confirm the post-Phase--1 baseline matches the recorded pre-refactor behavior and carry any explicitly documented pre-existing failures forward.
- [x] Create `docs/energy-logistics-validation.md` with the `EL-INV-*` map, transition table, exact arithmetic vectors, legacy rejection matrix, insertion permutations, boundary expectations, and evidence slots.
- [x] Add a test-only/pure modeling table over the authored 20-system graph for D1 offer projections, D3 gross sizing, all three route burns, fee stages, expected score, and system headroom.
- [x] Design shared fixtures/assertion helpers and add compile-safe executable tests or minimal API scaffolding for the first delegated slice.
- [x] Capture failure-first evidence for each first-wave behavior test; label tests that intentionally characterize existing behavior.
- [x] Choose repository values for fee bps, maximum freight rate, projection window, timeout, source overrides, Energy targets/caps, and at least two NPC archetypes.
- [x] Demonstrate at least one positive-profit Normal-stage Energy route and at least one correctly rejected burn-dominated route.
- [x] Demonstrate that accepted payloads retain recovery reserve under every partial-settlement boundary.
- [x] Freeze Wave 1 public signatures, allowed files, test names, expected results, and stop conditions before invoking an implementation worker.
- [x] Record the chosen values and representative table in the validation artifact before enabling runtime matching.

Files: `docs/energy-logistics-validation.md`, `crates/game-core/src/energy_logistics/tests.rs`, minimal signatures in `crates/game-core/src/energy_logistics/mod.rs`, and the explicitly tuned content files.

### Phase 1: Core types, arithmetic, and content compilation

Delegate Wave 1A and 1B only after the main agent's Phase 0 gate. Core and content workers receive separate bounded handoffs; the main agent integrates and validates between them.

- [x] Add `ContractId`, `BulkEnergyHold`, active contract states/records, resources, diagnostics, snapshots, events, and typed errors.
- [x] Add pure checked helpers for D1 projection/exportable/offers, D2 fee/rate, D3 gross sizing, D4 route utility, D7 settlement selection/allocation, and D8 timeout/recovery.
- [x] Extend trader/core definitions with bulk capacity and archetype identity.
- [x] Compile global/per-market logistics policy and NPC archetype registry in `game-content`.
- [x] Add semantic validations from D15 and repository content validation tests.
- [x] Update physical-stock test helpers to count tank + owned bulk + locked bulk, not generic Energy cargo.

Files: `crates/game-core/src/energy_logistics/mod.rs`, limited root wiring in `crates/game-core/src/lib.rs`, `crates/game-content/src/energy_logistics.rs`, limited root compiler wiring in `crates/game-content/src/lib.rs`, and content RON files.

### Phase 2: Remove ordinary Energy trading and add exact transfers

Before delegation, the main agent authors the complete `EL-INV-LEGACY` executable rejection matrix and exact transfer pre/post fixtures. One core worker implements this phase serially.

- [x] Reject `core:energy` from quotes, local buy/sell, trade limits, reservations, automated ordinary opportunity collection, liquidation, and reroute paths.
- [x] Remove Energy quote-only config and tests.
- [x] Replace generic cargo Energy paths and ledger dimensions with tank/bulk/contract transfer dimensions.
- [x] Bound market-to-tank withdrawal by D1 exportable Energy including active source claims/export reserve. This bound applies to every withdrawal path, including the automated NPC tank-balancing pass in D13 phase 8; list that path explicitly in the `EL-INV-LEGACY` rejection/bounding matrix.
- [x] Add exact owned-bulk-to-tank and owned-bulk-to-market commands.
- [x] Preserve `RecordExternalDelivery` as an explicitly external diagnostic boundary.
- [x] Prove every removed entry point rejects Energy without mutation while ordinary-goods behavior remains unchanged.

Files: `crates/game-core/src/lib.rs`, `crates/game-content/src/lib.rs`, `content/economy_config.ron`, `content/economy.ron`.

### Phase 3: Contract lifecycle and schedule integration

Execute this as serial Waves 3A–3C, not one broad delegation. Before each wave, the main agent authors the corresponding D6, D7/D8, or D12/D13 executable fixtures and observes the expected failure.

- [x] Add player/NPC intent collection and D12 resolution.
- [x] Implement remote source claims/deadhead, source revocation, atomic load/depart, and cancellation semantics from D6.
- [x] Implement arrival settlement, retries, exact timeout boundary, proportional fee conversion, recovery reserve, and terminal recovery on the same contract.
- [x] Add the D13 schedule phases in `GameSession::step`.
- [x] Block manual travel/transfer/liquidation and dynamic retirement around active contracts/locked lots.
- [x] Integrate earned fee/deadhead cost with trader profitability.
- [x] Preserve one active contract and one locked lot invariants at every transition.

Files: primarily `crates/game-core/src/energy_logistics/mod.rs`, with serial limited integration edits in `crates/game-core/src/lib.rs`; main-authored expectations remain in `crates/game-core/src/energy_logistics/tests.rs`.

### Phase 4: NPC archetypes and dynamic-fleet demand

- [x] Compare Energy and ordinary opportunities with canonical profit-per-tick utility before D12 contention.
- [x] Compile/spawn differentiated NPC archetypes and use archetype stable ID tie-breaks.
- [x] Feed genuinely unserved Energy opportunity into dynamic spawn persistence.
- [x] Exclude reimbursement from profit and block retirement during every active contract state.
- [x] Add deterministic tests showing bulk hauling demand selects a bulk-capable archetype and ordinary demand can still select a general freighter.

Files: `crates/game-core/src/lib.rs`, `crates/game-content/src/lib.rs`, `content/traders.ron`.

### Phase 5: App, TUI, diagnostics, and current documentation

Freeze core commands, snapshots, diagnostics, and vocabulary before Wave 5. App and CLI may proceed in parallel; TUI follows the integrated app views. The main agent owns cross-layer acceptance tests.

- [x] Add D16 app requests, immutable views, resolved names, typed rejections, and lifecycle presentation events.
- [x] Replace Energy bid/ask rows with request/offer/logistics state in both supported TUI layouts.
- [x] Show tank, owned bulk, locked bulk, contract details, transfer maxima, deadlines, and blockers.
- [x] Add exact ledger fields for source loading, destination delivery, allocation conversion, owned-bulk deposit, recovery return, and recovery curtailment.
- [x] Add exhaustive D10 starvation attribution and contract/fleet metrics to CLI diagnostics.
- [x] Update `docs/energy-economy.md`, `content/encyclopedia.ron`, README, and CHANGELOG in the same implementation.
- [x] Append pre-merge acceptance evidence to `docs/world-dynamics-validation.md`.

Files: `crates/game-app/src/energy_logistics.rs` plus limited root wiring, `crates/game-tui/src/screens/energy_logistics.rs` plus limited screen/input wiring, `crates/game-cli/src/main.rs`, and docs/content listed above.

## Acceptance Criteria

### Economic and physical correctness

- [x] Delivering Energy never reduces destination stock; every completed contract produces strictly positive net delivery.
- [x] A zero-Energy destination can request and receive Energy without pre-paying Energy.
- [x] Every gross payload reconciles into destination delivery, travel burn, carrier-owned allocation, returned locked Energy, or explicit recovery curtailment.
- [x] No unledgered Energy disappears under any tank, bulk, or market capacity shortfall.
- [x] Recovery curtailment is the only contract capacity-loss path and is separately ledgered.
- [x] Contract fee is never paid for undelivered Energy.
- [x] Reimbursement, fee, claims, locked lots, terminal events, and counters apply exactly once.

### Lifecycle and anti-strand

- [x] Remote acceptance creates one source claim; loading or terminal pre-load failure releases it once.
- [x] Source distress can revoke only before loading.
- [x] Cancellation during deadhead releases the claim but does not cancel physical travel.
- [x] Post-loading cancellation, redirection, spending, tank transfer, liquidation, and retirement are rejected without mutation.
- [x] Every incomplete delivery retains enough locked Energy for its accepted recovery route.
- [x] Every accepted contract reaches completion, a pre-load terminal outcome, or recovered failure; no recovery loop or permanent locked cargo remains.
- [x] A contract carrier is never left with no route fuel and no valid recovery/escape budget.

### Determinism and coexistence

- [x] Matching, pre-load maintenance, settlement, timeout, and recovery are invariant to system/trader/contract ECS insertion order.
- [x] Inbound commitments suppress duplicate request sizing without blocking generation or direct deposits.
- [x] NPCs choose Energy only when it beats their best ordinary positive-profit opportunity under the canonical score.
- [x] A higher brownout fee raises otherwise identical Energy opportunity score and can alter carrier selection.
- [x] Ordinary-goods trade, reservations, liquidation, route subsidies, investments, and population dynamics remain active.
- [x] Dynamic fleet spawning can select both Energy-hauler and general-freighter archetypes from actual unserved opportunities.

### Boundary and presentation

- [x] Every old ordinary trade entry point rejects `core:energy` atomically.
- [x] Player contract and bulk-transfer flows execute entirely through app requests and immutable views.
- [x] Stale contract acceptance reports a typed blocker/current maximum without mutation.
- [x] UI and docs show no Energy bid/ask explanation.
- [x] Contract cards show payload, all route burns, fee, net profit, net delivery, freight rate, recovery reserve, runway, ownership, deadline, and blockers.
- [x] D10 starvation attribution is mutually exclusive and exhaustive for every unsupplied destination tick.
- [x] The feature passes with service fleets absent.

## Testing Strategy

### Pure/unit tests

- D1 protection arithmetic with claims larger than stock, wide-intermediate overflow boundaries, seasonal projection, conservative burn, zero/base offers, and cap pressure.
- D2 fee floor and freight-rate ceil at zero/one/large boundaries.
- D3 largest-gross inversion at every fee-floor edge; net cap, freight cap, bulk cap, source cap, positive-profit, tank, and recovery rejection.
- D4 deadhead/local-source utility and exact tie scores.
- Bulk used/headroom across owned and the optional locked lot; occupied-slot acceptance rejects atomically.
- D7 settlement selection for zero/full/nearly-full headroom and every tank/owned-bulk combination.
- Multi-retry derived reimbursement/fee deltas never double-pay and always preserve `R` while incomplete.
- D8 exact deadline ticks and recovery conversion when tank is empty, partially full, and full, including zero prior settlement where both `B` and `R` convert.
- D12 total-order ties at every key level, captured-key stability, and exact stale-payload rejection.
- Content validation for fee schedule, bps, timeout/window, archetype IDs/caps/capacities, overrides, and removed Energy quote config.

### Core integration tests

- Local-source acceptance atomically loads and departs.
- Remote acceptance claims, deadheads, loads, releases, and departs.
- Player deadhead cancellation releases claim while physical travel continues.
- Source revocation before pickup; impossibility of revocation after loading.
- Full source → carrier → destination contract with exact ledger reconciliation.
- Zero-stock destination receives positive Energy.
- Two accepted inbound contracts suppress a third without reserving destination storage.
- Zero-headroom arrival makes no allocation payment and preserves recovery reserve.
- Partial settlement across several ticks converts reimbursement once and fee pro rata.
- Timeout occurs before the deadline tick's would-be settlement attempt.
- Recovery pre-pays fuel, returns remaining cargo, and records source overflow as recovery curtailment.
- A contract carrier at a zero-surplus destination is never stranded.
- Dynamic retirement is blocked by claims, locked lots, arrival waits, and recovery.
- All ordinary Energy quote/trade/reservation/liquidation/reroute paths reject without mutation.
- Ordinary goods continue trading while Energy contracts operate.
- System/trader/contract insertion permutations produce identical active contracts, terminal events, stocks, lots, and ledgers.

### App/TUI tests

- Accept and cancel through typed app requests; stale acceptance produces typed rejection.
- Exact owned-bulk transfer maxima and rejection messages.
- Immutable views resolve source/destination/carrier names and expose no ECS IDs.
- Energy rows omit quotes and show request/offer/inbound/cause.
- Contract views render all D16 terms and every lifecycle state/blocker in both layouts.
- Tank, owned bulk, locked bulk, and general cargo remain visually distinct.

### Long-run and manual gates

- Exact Energy reconciliation over 1,000 and 10,000 ticks with contracts enabled and service fleets absent.
- Contract movement reported separately from ordinary transactions.
- Nonzero accepted/completed contract activity on repository content.
- Plausible D10 attribution with no unclassified starvation ticks.
- No permanent claims, locked lots, repeated futile same-state matching, recovery loops, or stationary contract-carrier deadlocks.
- Continuing final-window ordinary production/trade and required world-dynamics metastability.
- Manual trace follows one shipment from source claim through deadhead, loading, travel burn, partial/full settlement, allocation, and destination stock.
- Manual capacity test demonstrates exact transfer rejection, automatic contract partial settlement, timeout, and visible recovery.

The existing enforced pre-merge commands in `docs/energy-economy.md` remain required. Extend their concise output with contract counts, D10 attribution, locked-lot state, recovery outcomes, archetype activity, and exact reconciliation before appending evidence to `docs/world-dynamics-validation.md`.

## Success Metrics for Phase 0 and Tuning

- Net Energy delivered per contract and per loaded/deadhead travel Energy burned.
- Carrier net profit per tick and per bulk-capacity unit versus ordinary goods profit per tick.
- Contract fill rate and time-to-accept/arrival by brownout stage.
- Offered Energy versus source curtailment.
- Request amount versus committed inbound amount.
- Starvation ticks by all five D10 states.
- Partial-settlement frequency, wait duration, timeout rate, and recovered/curtailed amount.
- Active and spawned trader count by archetype.
- Commercial Energy share versus ordinary-goods activity in the final diagnostic window.

The target is not universal service. The target is coherent, profitable-at-scale commercial hauling whose failures are physical, deterministic, and legible.

## Risks and Mitigations

| Risk | Consequence | Mitigation |
| --- | --- | --- |
| No authored source reaches projected glut | Contracts never activate. | Phase 0 graph table; `authored_export_base`; content validation/diagnostics. |
| Fee or burn consumes most payload | Nominal delivery provides little relief. | D3 freight cap, recovery viability, visible contract terms. |
| Deadhead omitted from utility | NPCs chase distant losing pickups. | D4 canonical net-profit score and tests. |
| Urgency order forces bad work | Fee schedule stops functioning as an incentive. | Carrier chooses positive best utility before D12 contention. |
| Duplicate inbound shipments overfill destinations | Chronic partial settlement/recovery churn. | D3 inbound suppression plus projected headroom; D10 metrics. |
| Partial delivery consumes recovery fuel | Carrier strands or recovery fails checked arithmetic. | D7 reserve invariant and exhaustive sequence tests. |
| Recovery becomes a second contract path | Double release, loops, divergent accounting. | Same `ContractId`, one transition executor, terminal source curtailment. |
| Public ordinary paths still move Energy | Two contradictory economies coexist. | D11 explicit rejection sweep and tests for every entry point. |
| Bulk storage creates hidden money | Player bypasses tank/cargo constraints. | Only tank spends; no tank/market-to-bulk commands. |
| Homogeneous NPC schema cannot create haulers | Low-margin demand remains unserved. | D15 archetype registry and demand-selected spawn logic. |
| Archetype work balloons scope | Energy logistics becomes a fleet rewrite. | Reuse global lifecycle settings; only physical profiles/count caps vary. |
| Terminal contract history grows without bound | Long simulations accumulate state. | Active map only; typed terminal events plus aggregate/bounded app history. |
| Recovery curtailment appears as deletion | Reconciliation or player trust breaks. | Separate ledger/event/view field and explicit acceptance exception. |
| Schedule changes regress world dynamics | Existing economic gates fail subtly. | D13 fixed order, permutation tests, mandatory 1k/10k gates. |
| Preparatory refactor expands into architecture rewrite | Large review surface and regressions precede gameplay value. | Phase -1 moves tests and creates feature seams only; no existing production-system extraction. |

## Files and Systems Affected

- `crates/game-core/src/lib.rs`
  - Root re-exports/wiring, public commands/events/snapshots as retained by the chosen internal visibility, old-path rejection, and authoritative tick phase integration.
- `crates/game-core/src/tests.rs`
  - Existing moved characterization/regression suite; main-agent-owned expected behavior.
- `crates/game-core/src/energy_logistics/mod.rs`
  - Contract/storage/config types, resources, checked helpers, prepared transitions, contract phase methods, ledgers, diagnostics, and snapshots.
- `crates/game-core/src/energy_logistics/tests.rs`
  - Main-authored `EL-INV-*` executable fixtures and Energy logistics behavior tests.
- `crates/game-content/src/lib.rs` and `crates/game-content/src/energy_logistics.rs`
  - Root compiler wiring plus logistics source schemas, per-market overrides, NPC archetype registry, and semantic validation.
- `crates/game-app/src/lib.rs` and `crates/game-app/src/energy_logistics.rs`
  - Actor/root wiring plus typed requests, immutable request/offer/opportunity/contract/storage views, resolved names, and builders.
- `crates/game-tui/src/lib.rs` and `crates/game-tui/src/screens/energy_logistics.rs`
  - Screen/root wiring plus Energy logistics rows, focused contract flow, storage/transfers, lifecycle/blocker rendering, help, and layout tests.
- `crates/game-cli/src/main.rs`
  - Contract/archetype/D10 metrics, exact flow reporting, deadlock gates, and long-run diagnostics.
- `content/economy_config.ron`
  - Global logistics defaults; removal of Energy quote policy.
- `content/economy.ron`
  - Import targets, source/destination overrides; removal of Energy import-priority overrides; possible storage/target tuning.
- `content/traders.ron`
  - Player bulk capacity and stable NPC archetype list.
- `content/encyclopedia.ron`
  - Unit-of-account and contract explanation.
- `docs/energy-economy.md`
  - Canonical physical stores, contracts, tick order, reconciliation, and diagnostics.
- `README.md`, `CHANGELOG.md`, `docs/world-dynamics-validation.md`
  - Player-facing behavior and acceptance evidence.

## References

- `docs/architecture.md` — headless simulation, stable IDs, content compilation, deterministic schedule, typed commands, immutable views, and dependency direction.
- `docs/energy-economy.md` — current physical Energy, reserves, claims, settlement, anti-strand, phase order, and required long-run gates.
- `docs/plans/2026-07-12-feature-energy-denominated-economy-foundation-plan.md` — Energy-as-numéraire and exact physical-settlement foundation.
- `docs/plans/2026-07-14-feature-system-service-fleets-plan.md` — later public resilience layer that reuses these contracts.
- `docs/plans/2026-07-14-feature-advanced-goods-development-projects-plan.md` — parallel durable-goods progression design; coordinate content/schema changes but do not combine implementations.
- `notes/2026-07-14-playtest-notes.md` — observed Energy-for-Energy drain and UI legibility problems.
- `docs/solutions/rust-ecs-validate-before-mutate.md` — required atomic transition pattern.
- `crates/game-core/src/lib.rs:3014-3031` — current centralized tick order.
- `crates/game-core/src/lib.rs:708-719,1723-1770` — current trader/tank/cargo and ordinary reservation state.
- `crates/game-core/src/lib.rs:1428-1488` — current Energy flow ledgers.
- `crates/game-core/src/lib.rs:3474-4027` — current quote/local/funded settlement paths that support Energy and must reject it.
- `crates/game-core/src/lib.rs:4043-4122` — direct tank transfer boundary.
- `crates/game-core/src/lib.rs:4516-4592` — current route burn at departure and immutable stored-route travel.
- `crates/game-core/src/lib.rs:5357-5435` — current one-shot ordinary reservation settlement, not reusable for contract retries.
- `crates/game-core/src/lib.rs:5863-5925` — current stable ordinary arrival settlement seam.
- `crates/game-core/src/lib.rs:6369-6585` — current ordinary NPC opportunity scoring and dynamic unserved-opportunity seam.
- `crates/game-content/src/lib.rs:154-188,313-343,1388-1548` — current economy/trader source schemas and homogeneous NPC compilation.
- `crates/game-app/src/lib.rs:50-430` — typed request and immutable-view boundary.
- `content/economy_config.ron`, `content/economy.ron`, `content/traders.ron` — current authored policy, systems, and trader values.

No external framework research is required. The curtailment analogy is economic/worldbuilding grounding, not a dependency.
