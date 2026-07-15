---
title: Physical Energy Logistics Design Plan
type: feature
date: 2026-07-14
revised: 2026-07-15
status: draft
---
# Physical Energy Logistics Design Plan

## Status and Purpose

This is a **working design plan**, not an implementation-ready specification. It defines the direction for inter-system Energy transport: delivery contracts, trader Energy storage, and bulk hauling. The blocking design questions from the first draft are now resolved in **Resolved Decisions** below; the remaining open questions are listed in **Remaining Open Questions** and none of them block Phase 0.

This plan deliberately excludes system-controlled service fleets. Commercial Energy logistics must work on its own and be testable without public rescue behavior obscuring its results. Service fleets are tracked in `docs/plans/2026-07-14-feature-system-service-fleets-plan.md` and build on the contracts defined here.

The accepted foundation is unchanged: Energy is physical. The game does not add an instantly transferable fiat currency. Physical storage, delayed transport, travel burn, local scarcity, and settlement capacity are intentional sources of gameplay friction.

## Summary of the Model

Ordinary Energy-for-Energy market trading is removed and replaced with **bulk Energy delivery contracts**. A source system consigns surplus Energy to a carrier; the carrier transports it; the destination receives the payload minus a small **carrier allocation** (route-burn reimbursement plus a profit fee). No Energy is ever bought with Energy.

Two related percentages exist and must never be conflated in code, content, or UI:

- **Carrier fee** — the authored, urgency-scaled profit percentage (`carrier_fee_bps`). This is the tuning knob.
- **Freight rate** — the all-in displayed number: total carrier allocation (fee + burn reimbursement) as a fraction of payload. This is the UI headline and diagnostic value.

The player-facing story, to be used consistently in UI, encyclopedia, and docs:

> Energy is the unit of account. It is never bought or sold — only generated, moved, and burned.

## Design Principles

1. **Energy is always physical.** Generation, storage, cargo, payment, travel burn, production burn, curtailment, and delivery must reconcile exactly.
2. **No instant universal currency.** Value remains locally embodied and physically transferable.
3. **Energy cannot buy itself at a scarcity price.** Scarcity affects shipment urgency, requested volume, and the freight rate a destination tolerates — never the exchange rate between identical Energy units.
4. **Bulk logistics is volume-driven.** A small percentage fee becomes attractive through shipment size.
5. **Capacity is a gameplay parameter.** Energy is treated as dense; trader and system Energy capacities need not resemble ordinary cargo capacity.
6. **Ship roles emerge from configuration.** Capacity, speed, burn, and range create couriers, freighters, and Energy haulers without hard-coded behavioral subclasses.
7. **Insufficient capacity causes partial or blocked settlement, never deletion.** Energy cannot disappear because a tank or store is full.
8. **The headless simulation remains authoritative.** Contracts, ownership, stores, claims, and settlement belong in `game-core`; frontends expose immutable views and typed commands.

## Problem Statement

### Energy-for-Energy trading reverses the intended flow

The current settlement treats `core:energy` like every other market good. If a destination buys `q` Energy cargo at unit bid `p`, it receives `q` but pays `p × q` from the same physical stock, so its net change is `q × (1 − p)`. A scarcity bid above 1 drains the deficient destination; at exactly 1 it gains nothing. Across a route, ignoring travel burn:

```text
origin change      = +(ask − 1) × quantity
destination change = −(bid − 1) × quantity
trader change      = +(bid − ask) × quantity
```

Energy is conserved but transferred economically in the wrong direction. This is a model contradiction, not a balance problem.

Note that the contract model does not eliminate the "price" of Energy — it bounds it. A 10,000-Energy payload with a 120-Energy carrier allocation is economically a delivery at an effective rate of 1.2%, paid in kind from the shipment. The contradiction was never that moving Energy has a cost; it was that a deficient system paid more than 1 Energy per Energy received. Under contracts the destination always nets strictly positive Energy.

### Trader capacities are currently arbitrary

The player has a 2,000-Energy tank and 400 general cargo slots. Secondary goods have bootstrap values around 85–110 Energy each, so a full cargo can represent roughly 34,000–44,000 Energy of value while immediate proceeds must fit a 2,000-Energy tank. Partial settlement prevents overflow, but the relationship between cargo value, Energy storage, travel range, and working capital is not designed. Current values act as accidental class constraints.

### Ordinary cargo capacity is a poor fit for bulk Energy

Every cargo unit currently consumes one general slot, including one unit of Energy. That assumes Energy has the same transport volume as machinery. Bulk Energy needs a dedicated capacity dimension for high-volume hauling to be viable. This does not require generalized mass/volume for all goods — Energy is intentionally the only exception.

## Resolved Decisions

These decisions are settled. The implementing agent must follow them and must not reopen them without flagging the conflict to the plan owner. Each is labeled for cross-reference.

### D1. Source motivation is curtailment-graded consignment

A source consigns Energy because surplus approaching `energy_storage_cap` has marginal value near zero — it is about to be curtailed and ledgered as waste. (Real-world anchor: electricity grids near surplus reach zero or negative prices.) Consequences:

- A source's **offered payload** is graded by proximity to curtailment, not merely floored at its reserves. Provisional formula, to be validated in Phase 0:

```text
exportable      = stock − life_support_protection − active_claims
                  − operating_reserve − anti_strand_budget − export_reserve
projected_glut  = max(0, stock + (generation − burn) × W − energy_storage_cap)
offered_payload = min(exportable, projected_glut + authored_export_base)
```

  where `W` is an authored `curtailment_projection_window` in ticks, `export_reserve` and `authored_export_base` are authored per-system with global defaults, and all arithmetic is checked integer math. `burn` means the system's deterministic per-tick sinks only — life support plus source/recipe operating burn — not speculative outflows such as pending trades or contracts (those are already excluded via `active_claims`).
- A system far from its cap with no authored export base offers nothing. Energy supply is therefore **supply-driven**: a needy destination can escalate its request, but nothing flows unless surplus exists somewhere. This is intended behavior for this slice; systems dying for lack of surplus are handled later by service fleets.
- The vague "authored willingness to support a connected destination" clause from the first draft is replaced by the concrete `authored_export_base` knob (default 0).

### D2. Freight is priced by an urgency-scaled fee schedule, not negotiation

There is no per-contract negotiation. The carrier's profit fee is set deterministically from the destination's current brownout stage via an authored schedule:

- `carrier_fee_bps`: authored per brownout stage, global defaults with optional per-system override. Illustrative only: 50 / 100 / 200 / 300 bps for healthy / Throttled / Emergency / Starvation. The schedule schema must use whatever representation `game-core` actually has for the healthy (non-brownout) state rather than inventing a new "Nominal" stage name.
- `carrier_profit = floor(payload × carrier_fee_bps / 10,000)`
- `carrier_allocation = planned_route_burn + carrier_profit`
- `net_destination_delivery = payload − carrier_allocation`

The rising schedule is the system's stabilizing feedback loop: scarcer destinations pay a higher fee, pulling carriers toward the systems that need Energy most. Without it, carriers would optimize for shortest route only and distant deficient systems would deterministically starve. The schedule values are tuning content, not code constants.

### D3. Contract viability is expressed as a maximum freight rate

There is no separate "minimum net delivery" knob. A contract is viable only when:

```text
carrier_allocation × 10,000 ≤ payload × max_allocation_bps
```

with `max_allocation_bps` authored globally with per-system destination override. This single check guarantees a minimum net-delivery fraction, composes automatically with payload size and route length (long routes need bigger payloads to stay viable), and is the same number the UI displays. Additional viability requirements: `net_destination_delivery > 0`, the source stays at or above its protections after loading (D1), the destination has useful projected headroom (D6), and the carrier can physically execute the route (bulk capacity, tank fuel, route existence).

### D4. Route-burn reimbursement uses planned burn, fixed at acceptance

`planned_route_burn` is computed from the planned route at acceptance and never adjusted for realized burn. This keeps the contract deterministic, computable up front, and means the contract card shown at acceptance is exactly true at settlement. Reroutes or wandering are at the carrier's expense.

### D5. Two Energy stores on a trader, not three

| Store | Purpose | Spendable? | Powers travel? | Capacity field |
| --- | --- | --- | --- | --- |
| Tank (drive/spendable) | Wallet, immediate proceeds, route fuel — unchanged from today | Yes | Yes | `energy_tank_capacity` |
| Bulk Energy hold | Dense Energy in **typed lots**: `Owned` or `ContractLocked(contract_id)` | No | No | new `bulk_energy_capacity` |

Rules, stated once and applied everywhere:

- **Only the tank spends.** Bulk Energy never pays for purchases and never powers travel, in any state.
- Bulk-to-tank transfer of `Owned` lots is allowed **only while docked**, bounded by tank headroom.
- `ContractLocked` lots cannot be spent, burned, transferred to tank, redirected, or converted to `Owned` by cancellation. They change ownership only through settlement or the recovery rule (D8).
- Bulk Energy consumes **zero** general cargo slots. General cargo capacity is unchanged and Energy no longer occupies it.
- Sale proceeds that exceed tank headroom trigger the existing disclosed partial-settlement primitive. Proceeds never silently spill into the bulk hold.
- Direct market↔trader transfers under the existing `refuel_policy` remain, with one change: **withdrawals draw only from the market's exportable surplus** as computed in D1, not from raw unreserved stock.

### D6. Destination headroom claims are soft

Matching uses the destination's *projected* headroom (current headroom plus projected net burn over the transit time) to size contracts, but no hard reservation blocks the destination's own generation or other deliveries during transit. On arrival, settlement runs against **actual** physical headroom with disclosed partial settlement. An occasional partial delivery is preferable to curtailing a destination's generation for an entire transit because of a claim.

### D7. Carrier allocation settles from cargo already aboard

The allocation units are physically part of the payload in the bulk hold, so settlement cannot lose them. On a full settlement the carrier receives the entire allocation. On a partial settlement the allocation converts **proportionally to the settled quantity**, in this order:

1. Transfer `min(net_destination_delivery, actual destination headroom)` from the `ContractLocked` lot into destination storage; call the amount actually transferred `settled`.
2. Convert allocation units from `ContractLocked` to trader property, burn reimbursement first, then fee pro-rata:
   - Reimbursement converts in full on the first settlement event, whatever `settled` is — the carrier genuinely spent that fuel.
   - Fee converts as `floor(carrier_profit × cumulative_settled / net_destination_delivery)`, minus fee already converted on earlier retries.
   - Converted units go to the tank up to tank headroom (a docked bulk→tank move); the remainder becomes an `Owned` bulk lot.
3. Any undelivered remainder stays `ContractLocked` and follows D8. Unconverted fee travels with the recovery consignment back to the source; the carrier is never paid profit for Energy it did not deliver.

No step can create or destroy Energy; each claim releases exactly once; retries (D8) use `cumulative_settled` so repeated partial settlements never double-pay the fee.

### D8. Settlement shortfall and stranded-cargo recovery

If actual destination headroom cannot absorb the full net delivery at arrival, the carrier remains docked and the contract remains `Arrived`; settlement retries each tick as the destination burns down its stock. If the contract is not fully settled within an authored `settlement_timeout_ticks`, it transitions to `Failed`, and the remaining `ContractLocked` lot converts to a zero-fee **recovery consignment** back to the source. Three rules make recovery safe:

1. **Recovery fuel is pre-paid.** At recovery initiation, the planned return burn converts from the locked lot into the carrier's tank immediately (bounded by tank headroom, remainder as an `Owned` lot). The carrier may otherwise be unable to fly: it can have burned its tank down waiting, and a deficient destination's exportable surplus — the only pool withdrawals may draw from under D5 — is zero by definition.
2. **Recovery terminates.** A recovery consignment's settlement at the source may overflow into curtailment: whatever the source's storage cannot absorb is ledgered as curtailed waste, exactly like generation overflow. This deliberately bends "insufficient capacity never deletes" — curtailment-at-cap is already an accepted, reconciled loss channel in the foundation design — and it guarantees every contract reaches `Completed` or a fully unwound `Failed` state. Recovery consignments never spawn further recovery consignments.
3. **Anti-strand still covers contract carriers.** The foundation slice's anti-strand guarantee must be restated to include carriers whose only cargo is `ContractLocked`: locked lots are not liquidatable, so the "sell a sub-quantity to fund a jump" escape does not apply to a pure hauler with an empty general hold. Rules 1 and 2, plus departure validation (a carrier may not accept a contract it cannot fuel round-trip to the nearest refuel-eligible system), are the mechanisms that satisfy the guarantee; the implementing agent must verify no remaining path leaves a contract carrier with an empty tank, no exportable surplus, and no pre-paid recovery fuel.

Recovery is the only path by which locked cargo changes destination, and locked cargo never becomes trader-owned except through the D7 allocation conversion.

### D9. No trust or reputation modeling for player carriers

Player-controlled carriers accept contracts under exactly the same rules as NPCs. Because contract cargo is `ContractLocked` (D5), the worst a player can do is strand it — a recovery problem already solved by D8, not a theft problem. Reputation systems are out of scope indefinitely, not just for this slice.

### D10. Diagnostics must distinguish the two starvation causes

Long-run diagnostics must separately report, per deficient destination: (a) ticks starved while **no exportable surplus existed anywhere reachable**, versus (b) ticks starved while surplus existed but **no contract was commercially viable or accepted**. These require different tuning responses (world/content authoring versus fee schedule) and cannot be distinguished from outcomes alone.

### D11. Presentation commits to the unit-of-account story

The Energy market row stops showing bid/ask and shows logistics state: stock, cap, runway, brownout stage, open request, open offer. Encyclopedia, help, and README adopt the sentence in the Summary. There must not be two live explanations of Energy trade.

### D12. Matching order is fully specified

Same-tick contract matching resolves in one deterministic total order. Candidate (request, offer, carrier) triples that pass D3 viability are sorted by:

1. Destination brownout stage, most severe first.
2. Destination projected runway, ascending.
3. Payload, descending.
4. Destination stable ID, then source stable ID, then carrier stable ID, ascending.

After each acceptance, the source's protections (D1) and the carrier's availability are re-checked before the next candidate is considered. The specific key order above is tunable content-facing policy; that a documented, tested total order exists is not. The implementing agent must not substitute a different order without updating this decision.

## Proposed Model

### 1. Destination requests and source offers

A destination Energy request exposes: current stock and storage headroom, current and projected runway, requested **net** delivery, brownout stage (which selects the fee via D2), earliest useful arrival window, and its `max_allocation_bps` (D3).

A source Energy offer exposes: `offered_payload` (D1), the protections backing it, expected curtailment pressure, contract size limits, and expiry/refresh behavior.

Requests and offers are recomputed deterministically from market state; they are views over state, not independently mutated objects.

### 2. Source-backed consignment

A carrier never purchases contract Energy. The source consigns a payload from `offered_payload` and locks it to the accepted contract. From loading until settlement the payload is a `ContractLocked` bulk lot on the carrier (D5): it cannot pay for goods, power travel, be redirected, be cancelled into trader stock, or mix with spendable Energy.

Diplomacy, taxation, factions, and alliance obligations remain outside this plan. The economic loop still closes without them: importer systems are the resource/goods-rich systems by authored anti-correlation, so goods flow toward exporters and Energy flows toward importers as two halves of one relationship.

### 3. Contract record

A contract records at least: source, destination, carrier; gross payload; planned route and `planned_route_burn` (D4); `carrier_fee_bps` captured at acceptance (D2); `carrier_allocation` and `net_destination_delivery`; the source claim; lifecycle state and timestamps; and the `ContractLocked` lot reference once loaded.

Worked example:

```text
payload:                  10,000 Energy
planned route burn:           20 Energy
carrier_fee_bps:          100 (1%)
carrier profit:              100 Energy
carrier allocation:          120 Energy
net destination delivery:  9,880 Energy
effective freight rate:      1.2%
```

All arithmetic is checked integer math with floor rounding on the fee. A 400-Energy load may be physically possible but fail D3 viability after route burn; a bulk carrier makes the same schedule worthwhile through volume. This is intended.

### 4. Contract lifecycle

```text
Open request/offer → Matched → Loading → InTransit → Arrived
    → Settled | PartiallySettled (retrying, D8)
    → Completed | Failed (recovery consignment, D8)
```

Failure states requiring explicit, tested transitions:

- Expired before loading (offer/request no longer valid): release claims, no cargo moved.
- Source revoked before loading because protections became unsafe: release claims, no cargo moved. (Revocation is only possible **before** loading; after loading the cargo is aboard and the contract proceeds.)
- Carrier unable to depart (insufficient tank fuel): contract fails before loading via departure validation; loading and departure are one atomic step.
- Route invalidated mid-transit: carrier continues to the destination if reachable; otherwise D8 recovery applies at the next dock.
- Destination headroom shortfall at arrival: D8.

Every transition releases each claim exactly once and preserves physical cargo ownership. All transitions follow the repository's validate-before-mutate pattern.

### 5. Ship configurations

Ship roles are content-defined configurations, not behavioral subclasses. Trader content gains `bulk_energy_capacity` alongside the existing tank, cargo, speed, and burn fields. Illustrative roles, not balance commitments:

| Role | Tank capacity | General cargo | Bulk Energy | Character |
| --- | ---: | ---: | ---: | --- |
| Courier | Low | Low | Low | Fast advanced-goods delivery |
| General freighter | Medium/high | High | Medium | Flexible ordinary trade |
| Energy hauler | Medium | Low | Very high | Low-margin bulk contracts |
| Long-range hauler | High | Low/medium | High | Expensive remote logistics |

No ship-selection or construction UI is required in this slice. The content model must simply stop assuming every trader has the same capacity profile. NPC opportunity scoring must evaluate Energy contracts and ordinary goods trades on a comparable basis (expected profit per tick) so haulers emerge from configuration rather than special-cased AI. That scoring must include the **deadhead leg**: travel burn from the carrier's current position to the source is real, unreimbursed tank cost (D4 reimburses the loaded leg only), and omitting it would make NPCs systematically overvalue distant contracts.

### 6. System storage

System Energy storage caps represent grid infrastructure, not cargo volume, and can be large. Storage headroom determines useful contract size (D6). Transfer throughput (loading rate limits) is explicitly **not** modeled in this slice; total capacity is the only constraint.

## Player Experience

Energy logistics answers different questions from market trading: who requests, who has surplus, what fits this ship, what does the destination actually gain, what does the carrier earn, who owns the cargo, and what is blocking progress. Suggested contract card:

```text
Payload                 10,000 Energy
Route burn (planned)        20 Energy
Carrier profit             100 Energy
Net delivery             9,880 Energy
Freight rate               1.2%
Destination runway       3 → 42 ticks
```

Manual direct transfers remain (D5): market ↔ tank per refuel policy (withdrawals bounded by exportable surplus), owned bulk ↔ tank while docked, owned bulk → local market storage. These are 1:1 physical movements; the UI must never present them as sales.

## Scope

### In scope

- Commercial source-backed Energy delivery contracts (single source → single destination).
- Destination requests and source offers as deterministic views.
- Fee-schedule pricing, viability, and net-delivery arithmetic (D2, D3).
- Dedicated `bulk_energy_capacity` and typed ownership lots (D5).
- Soft headroom projection, arrival settlement, retry, and recovery (D6–D8).
- Energy logistics views, encyclopedia updates, and diagnostics (D10, D11).
- Removal of `core:energy` from ordinary bid/ask trading and goods reservations.

### Out of scope

- System-controlled service fleets (separate plan); guaranteed rescue.
- Ship construction, shipyards, technology trees.
- Diplomacy, factions, alliances, tariffs, reputation (D9).
- Generalized mass/volume for all goods.
- Piracy, interception, combat, insurance.
- Multi-stop or combined contracts (one source, one destination, one carrier).
- Loading/unloading throughput limits.
- Maintenance and crew simulation.

A critical system may still fail because no carrier serves it. That is acceptable — and per D10, diagnostics must say *why*.

## Compatibility Stance

Full replacement of ordinary inter-system Energy trading:

- `core:energy` remains a physical inventory good and the unit of account for all other goods' prices.
- Remove Energy from ordinary bid/ask arbitrage and ordinary goods reservations.
- Ordinary Energy-denominated trade for non-Energy goods is unchanged.
- Direct transfers adapt to D5 semantics.
- Documentation and encyclopedia are updated in the same change (D11); no parallel explanations.
- Remove config that only made sense in the bid/ask Energy world rather than leaving it as zombie policy. Known instances in `content/economy_config.ron`: the `core:energy` entry in `import_priorities`, and `emergency_energy_bid_ceiling` in `brownouts`. The implementing agent should sweep for any other Energy-quote-specific knobs during Phase 2.
- No save migration; persistence does not exist.

The existing RouteSubsidy investment may later influence the fee schedule but is not assumed here. It must never make a deficient destination pay more Energy from existing stock to receive Energy.

## SpecFlow Sketch

### Commercial delivery, happy path

1. A destination in deficit publishes a request; its brownout stage selects `carrier_fee_bps` (D2).
2. A source near curtailment publishes `offered_payload` (D1).
3. A carrier with bulk capacity and tank fuel evaluates payload, planned burn, profit, and timing; D3 viability passes.
4. Acceptance claims source Energy in the D12 total order — same contention discipline as existing reservations.
5. Loading and departure execute atomically; the payload becomes a `ContractLocked` lot.
6. Travel burns only tank Energy.
7. Arrival settles per D7; shortfall follows D8.
8. Claims release exactly once; the ledger records every physical movement.

### Important variations

- **No matching source:** request stays open; D10 counts it as cause (a).
- **No viable carrier:** request stays open; D10 counts it as cause (b).
- **Source distress before loading:** revoke per lifecycle rules; loaded cargo is never revoked.
- **Destination fills during transit:** partial settlement, retry, then recovery (D6, D8).
- **Allocation exceeds tank headroom:** remainder becomes an `Owned` bulk lot (D7); nothing is lost.
- **Several contracts compete for one source:** D12 total order; the source's protections are re-checked after each acceptance.
- **Player cancels after loading:** cancellation is not available after loading; the contract must be delivered or fail into D8 recovery.

## Remaining Open Questions

None of these block Phase 0; each has a recommended default the implementing agent should assume unless overridden.

1. **Can a carrier hold multiple concurrent contracts?** Recommended default: no — one active Energy contract per carrier in this slice. Revisit after diagnostics.
2. **Should player-owned Energy hauling outside contracts exist?** Recommended default: defer. Owned bulk Energy in this slice comes only from carrier allocations (D7) and can be deposited to a local market as a plain transfer. A "spot delivery earns the fee schedule" mechanic is coherent but adds a second settlement path; evaluate after contracts prove out.
3. **Exact `curtailment_projection_window` and offer formula constants (D1).** Phase 0 tables decide the defaults.
4. **`settlement_timeout_ticks` default (D8).** Phase 0 decides; must exceed typical destination burn-down time for a viable contract.
5. **Where contract phases slot into the existing tick order.** Recommended default: compute offers/requests after generation and life-support burn; resolve contract matching in the same stable-ordered resolution phase as existing reservations; settle at arrivals. The implementing agent should confirm against the current phase functions in `game-core` and document the final order.

## Provisional Implementation Sequence

Implementation must not begin until Phase 0 confirms the arithmetic and the store model against the resolved decisions.

### Phase 0: Economic and accounting spike

- [ ] Model representative payloads, fee schedules, route burns, capacities, and curtailment pressure in deterministic tables or tests; validate D1's offer formula and D3's viability threshold produce sensible offers across the authored 20-system map.
- [ ] Confirm the two-store model (D5) against the existing tank/refuel code paths.
- [ ] Prove load, travel, allocation, deposit, partial settlement, retry, and recovery (D7, D8) conserve Energy in a paper/test model.
- [ ] Choose provisional ship configurations that make low-margin hauling viable (a bulk hauler must beat goods trading on some real route at Nominal-stage fees, or the fee schedule needs adjustment).

### Phase 1: Pure contracts and content definitions

- [ ] Add checked helpers: exportable surplus and offered payload (D1), fee and allocation arithmetic (D2), viability (D3), projected headroom (D6), settlement split (D7).
- [ ] Define contract, lot, claim, offer/request view, and lifecycle types with the D8 transitions.
- [ ] Define stable contention ordering and validate-before-mutate rules for every transition.
- [ ] Extend trader content schema with `bulk_energy_capacity` and differentiated configurations; extend economy config with the fee schedule, `max_allocation_bps`, `curtailment_projection_window`, `export_reserve`, `authored_export_base`, and `settlement_timeout_ticks`.

### Phase 2: Runtime commercial contracts

- [ ] Remove `core:energy` from ordinary bid/ask route planning, quoting, and settlement; bound refuel withdrawals by exportable surplus (D5).
- [ ] Implement request/offer computation, matching, loading, transit, settlement, retry, and recovery.
- [ ] Integrate contract opportunities into NPC opportunity scoring on a profit-per-tick basis, including the deadhead leg.
- [ ] Ensure contract profits feed the dynamic-NPC spawn/retirement signal (`opportunity_threshold` and friends in `content/traders.ron`); otherwise sustained hauling demand will never spawn haulers.
- [ ] Preserve ordinary-goods pricing and funded settlement unchanged.

### Phase 3: Views, diagnostics, and tuning

- [ ] Replace the Energy market row per D11; show tank, owned bulk, and contract-locked Energy distinctly with a contract card matching the Player Experience sketch.
- [ ] Add contract lifecycle and physical-flow diagnostics, including the D10 starvation-cause split.
- [ ] Tune fee schedule, capacities, and offer constants through deterministic short and long runs.
- [ ] Update `docs/energy-economy.md`, encyclopedia content, README, and CHANGELOG.

## Acceptance Direction

- [ ] Delivering Energy can never reduce destination stock; every completed contract reports strictly positive net delivery and exact physical reconciliation.
- [ ] A zero-Energy destination can request and receive a delivery without pre-paying anything.
- [ ] Commercial hauling is profitable through batch volume with integer arithmetic; a small load can be correctly non-viable.
- [ ] Contract cargo cannot be spent, burned, redirected, transferred to tank, or cancelled into trader-owned Energy (D5).
- [ ] No proceeds or cargo disappear under any capacity shortfall (D7, D8).
- [ ] Source protections, claims, and locked lots release exactly once per contract, on every lifecycle path.
- [ ] A carrier is never paid fee for Energy it did not deliver (D7), and every contract terminates — no recovery loops and no stranded contract carriers (D8).
- [ ] A higher brownout stage yields a higher carrier fee for otherwise identical contracts (D2), and diagnostics show carriers preferentially serving urgent destinations.
- [ ] Diagnostics separate no-surplus starvation from no-viable-carrier starvation (D10).
- [ ] Ordinary-goods trade remains active and unchanged.
- [ ] Same-tick contract contention is invariant to ECS insertion order.
- [ ] The UI shows payload, burn, profit, net delivery, freight rate, ownership, and blockers per the contract card sketch.
- [ ] The feature is fully testable with no service fleets present.

## Success Metrics to Define During Tuning

- Net Energy delivered per contract and per travel Energy burned.
- Carrier profit per tick and per bulk-capacity unit, versus goods-trading profit per tick.
- Contract fill rate by destination brownout stage.
- Time from request to acceptance and to arrival, by stage.
- Source Energy curtailed versus consigned (D1 effectiveness).
- Starvation ticks split by cause (D10).
- Failed/recovered contract count and recovery time (D8).
- Late-window ordinary production and trade activity (regression guard).

The desired result is not that every request is served. It is that accepted Energy work is physically coherent, profitable at scale, and fails for visible, correctly attributed logistical reasons.

## Testing Strategy

### Pure/unit tests

- Fee, allocation, and net-delivery arithmetic (D2), including overflow boundaries and floor rounding at small and large payloads.
- D3 viability rejection when allocation exceeds `max_allocation_bps`, including the burn-dominated small-payload case.
- Offered-payload protection under every reserve and claim (D1), including zero and negative projected glut.
- Settlement split and proportional allocation conversion (D7) at every combination of destination headroom, tank headroom, and bulk state, including multi-retry sequences that must never double-pay reimbursement or fee.
- D12 total order, including ties at every key level.

### Integration tests

- Full source → carrier → destination contract with exact ledger reconciliation.
- Zero-stock destination receives positive Energy.
- Source revocation before loading; impossibility of revocation after loading.
- Destination fills during transit → partial settlement → retry → timeout → recovery consignment (D8), conserving Energy throughout; verify pre-paid recovery fuel, pro-rata fee return, and source-side curtailment overflow when the source is at cap.
- A contract carrier waiting at a zero-surplus destination is never left unable to move (D8 rule 3).
- ContractLocked cargo rejected from spending, travel burn, tank transfer, and cancellation paths (D5).
- Refuel withdrawal bounded by exportable surplus (D5).
- Ordinary goods continue trading while Energy contracts operate.
- ECS insertion-order permutation produces identical matching results.

### Long-run diagnostics

- Exact Energy reconciliation over 1,000- and 10,000-tick runs with contracts active.
- Contract activity reported separately from ordinary goods transactions.
- D10 starvation-cause attribution present and plausible.
- No permanent locked cargo, repeated futile matching, or stationary-laden deadlocks.
- Runs executed with service fleets absent.

### Manual validation

- Inspect a request, an offer, and an accepted contract card; confirm the freight rate and net delivery match settlement results.
- Follow Energy through source stock, locked cargo, travel burn, allocation, and destination stock in the UI.
- Fill trader and destination storage and verify disclosed partial behavior at each point.
- Confirm an unfilled critical request is understandable and correctly attributed (D10) rather than silently reversing Energy flow.

## Risks

| Risk | Consequence | Mitigation |
| --- | --- | --- |
| Fee consumes most payload on marginal routes | Delivery provides little relief. | D3 viability cap; expose the split on the contract card. |
| No source has surplus when importers need it | Commercial logistics rarely fires. | D1 grades offers by curtailment pressure; D10 attributes the cause; service fleets are the later backstop. |
| Flat carrier incentives ignore urgent systems | Distant deficient systems calcify and die. | D2 urgency-scaled fee schedule; acceptance criterion requires observable preference for urgent destinations. |
| Consignment becomes free loot | Carriers extract public surplus. | D5 locked lots; D8 recovery; no post-loading cancellation. |
| Large bulk holds trivialize range | Capacity distinctions disappear. | Bulk Energy never powers travel (D5); range is bounded by tank alone. |
| Tiny shipments profit disproportionately via rounding | Degenerate micro-routes dominate. | Floor arithmetic plus D3 viability threshold. |
| Store model confuses players | Physical economy becomes illegible. | Two stores only (D5); one spending rule; consistent ownership language (D11). |
| Soft headroom claims cause chronic partial settlement | Carriers idle at full destinations. | D6 projected-headroom sizing; D8 retry and timeout; metric for time-to-settle. |
| Removing Energy quotes breaks NPC survival routing | Deficient systems receive nothing. | Contracts enter NPC scoring directly; validated independently before service fleets. |

## Files and Systems Likely Affected

Candidate surfaces, not commitments to exact type names.

- `docs/energy-economy.md` — replace Energy pricing/settlement sections with logistics contracts.
- `content/traders.ron` — `bulk_energy_capacity`, differentiated configurations.
- `content/economy_config.ron` — fee schedule, `max_allocation_bps`, offer constants, `settlement_timeout_ticks`.
- `content/economy.ron` — per-system `export_reserve`, `authored_export_base`, overrides.
- `content/encyclopedia.ron` — the D11 unit-of-account explanation.
- `crates/game-core/src/lib.rs` — contracts, stores, lots, claims, settlement, ledgers, snapshots.
- `crates/game-content/src/lib.rs` — schema compilation and validation.
- `crates/game-app/src/lib.rs` — immutable contract/storage views and typed commands.
- `crates/game-tui/src/lib.rs` — Trade, ship-storage, and help presentation.
- `crates/game-cli/src/main.rs` — contract diagnostics, D10 attribution, long-run gates.
- `CHANGELOG.md` and `README.md` — player-facing behavior after implementation.

## References & Research

### Internal references

- `docs/architecture.md` — headless simulation, immutable frontend views, content pipeline, dependency direction.
- `docs/energy-economy.md:11-20,44-81,93-104,151-181` — current physical Energy, tank/cargo, pricing, settlement, phase order, diagnostics.
- `docs/plans/2026-07-12-feature-energy-denominated-economy-foundation-plan.md:25-75,106-143` — Energy-as-numéraire and physical-settlement decisions; authored solar/resource anti-correlation that closes the goods-for-Energy loop without diplomacy.
- `docs/plans/2026-07-14-feature-system-service-fleets-plan.md` — later public resilience layer.
- `notes/2026-07-14-playtest-notes.md` — the observed Energy-for-Energy drain and UI legibility complaints motivating this plan.
- `content/traders.ron:1-35` — current tank, cargo, speed, burn, refuel policy values.
- `content/economy_config.ron:16-29` — existing brownout stages that D2's fee schedule keys off.
- `crates/game-core/src/lib.rs:708-737,1723-1733,2120-2126` — trader definition/state, refuel policy, direct transfer commands.
- `crates/game-core/src/lib.rs:3040-3174,3442-3694` — local capacity limits and current Energy-compatible quote path (to be removed for Energy).
- `crates/game-core/src/lib.rs:3700-4027` — current local buy/sell and funded settlement (unchanged for ordinary goods).
- `crates/game-core/src/lib.rs:4046-4165` — direct tank transfer and idle NPC balancing.
- `crates/game-core/src/lib.rs:5585-5685` — route affordability, travel burn, tank headroom, destination funding.

### External references

None required. The curtailment analogy (zero/negative electricity prices at surplus) is worldbuilding grounding, not a dependency.

### Institutional knowledge

- `docs/solutions/rust-ecs-validate-before-mutate.md` — all rule checks and checked calculations complete before atomic state mutation; applies to every contract lifecycle transition.
