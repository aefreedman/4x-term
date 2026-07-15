---
title: Physical Energy Logistics Validation Contract
type: validation
date: 2026-07-15
status: active
plan: docs/plans/2026-07-14-feature-physical-energy-logistics-plan.md
---
# Physical Energy Logistics Validation Contract

This file freezes the executable expectations for the physical Energy logistics plan. The plan's D1–D16 decisions remain authoritative. Expected values in this file may be changed only by first updating the accepted plan.

## Invariant map

| ID | Contract | Required evidence |
| --- | --- | --- |
| `EL-INV-PHYSICAL` | Whole-world Energy reconciles exactly across markets, tanks, owned bulk, locked bulk, generation/inflow, all burns, ordinary curtailment, and recovery curtailment. | Unit ledgers; 1,000/10,000-tick reconciliation. |
| `EL-INV-LOCKED` | `ContractLocked(id)` Energy is inaccessible outside the contract executor. | Rejection matrix and post-state equality. |
| `EL-INV-LOT` | One loaded active contract corresponds to exactly one optional locked lot; pre-load/terminal contracts have none. | Transition assertions and insertion permutations. |
| `EL-INV-CLAIM` | Source claims exist only in active pre-load contracts, allocate oldest-first under distress, and release once. | D1/D6 claim table and terminal counters. |
| `EL-INV-RECOVERY` | Every incomplete delivery preserves `R`; recovery uses the same contract and always terminates. | D7/D8 sequences and bounded lifecycle tests. |
| `EL-INV-ALLOCATION` | Reimbursement and fee are derived from cumulative settlement and never paid twice. | Multi-retry and timeout vectors. |
| `EL-INV-ORDER` | Matching, maintenance, settlement, and recovery do not depend on insertion order. | D12 permutations. |
| `EL-INV-LEGACY` | Every ordinary Energy trade path rejects atomically. | Complete rejection matrix below. |
| `EL-INV-ANTISTRAND` | Acceptance retains deadhead, loaded-route, recovery, and escape budgets. | Gross sizing and lifecycle fixtures. |
| `EL-INV-BOUNDARY` | Frontends submit typed commands and render immutable views without ECS IDs or float economics. | App/TUI tests. |

## Phase 0 repository policy

The repository fixture uses:

| Value | Chosen setting |
| --- | ---: |
| Carrier fee bps (`Normal/Throttled/Emergency/Starvation`) | `50 / 100 / 200 / 300` |
| Maximum allocation | `1,000 bps` |
| Curtailment projection window | `20 ticks` |
| Global export reserve/base | `0 / 0 Energy` |
| Settlement timeout | `20 ticks` |
| `frontier:system_15` starting Energy | `5,000` |
| `frontier:system_15` authored export base override | `3,200` |
| `frontier:system_14` Energy target | `5,000` |
| General freighter | initial/max `5/10`, tank `1,000/1,500`, bulk `0`, cargo `300`, speed/burn `8/1` |
| Bulk Energy hauler | initial/max `4/10`, tank `1,000/1,500`, bulk `4,000`, cargo `100`, speed/burn `8/1` |

The initial fleet total remains nine. At least one bulk hauler starts at `frontier:system_15`; remaining archetype distribution is stable-ID ordered. The tuned source begins at capacity so the runtime gate does not depend on a several-hundred-tick warmup.

### Representative authored-map table

Assumptions: phase-10 Normal state, no claims/inbound commitments, speed 8, burn 1, source `frontier:system_15`, destination `frontier:system_14`, loaded/recovery distance `13.379`, 2 ticks and 14 Energy each. Source generation is 30; frozen life plus operating burn is 18. A saturated projection returns to 4,982 stock each tick and produces 12 curtailment pressure per tick, or 240 over the 20-tick window. With the 3,200 base, the offer ceiling is 3,440 before protections.

| Case | Origin | Gross `P` | Deadhead | Loaded `B` | Fee `F` | Allocation | Net `N` | Freight | Recovery `R` | Score | Expected |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Normal local pickup | system 15 | 3,029 | 0 | 14 | 15 | 29 | 3,000 | 96 bps | 14 | 7,500,000 | viable maximum for 3,000 net headroom |
| Normal remote pickup | system 13 | 3,029 | 23 | 14 | 15 | 29 | 3,000 | 96 bps | 14 | negative | reject: fee does not exceed deadhead burn |
| Worked full delivery | local fixture | 4,000 | 10 | 20 | 40 | 60 | 3,940 | 150 bps | 20 | 6,000,000 | viable |

The source protection fixture uses stock 5,000, ordinary claims 0, operating reserve 54, liquidation budget 55, and export reserve 0: exportable and claim capacity are 4,891. Claims `(id 1: 3,200)`, `(id 2: 1,800)`, `(id 3: 100)` allocate oldest-first: id 1 safe, id 2 revoked, id 3 safe, final unused capacity 1,591.

## Exact arithmetic vectors

### D1 projection and offers

| ID | Inputs | Expected |
| --- | --- | --- |
| `EL-D1-PROTECT-01` | stock 5,000; claims 0; operating 54; liquidation 55; export reserve 0 | exportable 4,891 |
| `EL-D1-PROTECT-02` | stock 100; ordinary 80; pre-load 50; operating 30; liquidation 20; export 10 | exportable 0, never negative |
| `EL-D1-PROJECT-01` | start 4,982; cap 5,000; generation 30; life 3; operating 15; 20 ticks | each glut 12; final 4,982; total glut 240 |
| `EL-D1-OFFER-01` | exportable 4,891; projected glut 240; base 3,200 | offered 3,440 |
| `EL-D1-CLAIM-01` | capacity 4,891; ordered claims 3,200/1,800/100 | safe/revoked/safe; remaining 1,591 |
| `EL-D1-OVERFLOW-01` | any checked signed-wide accumulation beyond representable Energy | `CoreError::Overflow`, no mutation |

### D2/D3 fee, ceil, and largest gross

| ID | `P/B/fee bps` and caps | Expected |
| --- | --- | --- |
| `EL-D2-WORKED-01` | 4,000 / 20 / 100 | profit 40; allocation 60; net 3,940; freight 150 bps |
| `EL-D2-FLOOR-01` | 199 / 0 / 50 | profit 0 |
| `EL-D2-FLOOR-02` | 200 / 0 / 50 | profit 1 |
| `EL-D3-MAX-01` | offer 3,440; bulk 4,000; net cap 3,000; `B=14`; fee 50; allocation cap 1,000; deadhead 0; tank covers routes; `R=14` | largest gross 3,029; net 3,000 |
| `EL-D3-MAX-02` | same, net cap 2,999 | largest gross 3,028; net 2,999 |
| `EL-D3-DEADHEAD-01` | `EL-D3-MAX-01` with deadhead 23 | no viable payload |
| `EL-D3-RECOVERY-01` | any candidate with `N <= R` | no viable payload |
| `EL-D3-STALE-01` | command asks 3,029 after recomputed maximum falls to 3,028 | typed stale/maximum rejection; no mutation |
| `EL-D3-OVERFLOW-01` | checked multiplication/addition overflows | `CoreError::Overflow`; no fallback loop |

Sizing uses bounded monotonic binary search. Tests include zero/one capacities and every fee-floor boundary around payloads 199/200 and 3,028/3,029.

## D6 transition table

| Transition | Before | Exact apply result |
| --- | --- | --- |
| Remote accept | carrier idle away from source; tank covers deadhead + loaded burn; no lot/contract | allocate one monotonic ID; burn deadhead; create one claim; enter `DeadheadingToSource`; begin stored route |
| Local accept | carrier idle at source; source has protected payload; tank covers loaded burn | subtract gross source stock; create one matching locked lot; burn loaded burn; enter `InTransit`; release no separate claim |
| Cancel deadhead | active player pre-load contract and claim | release claim/remove contract/emit `CancelledBeforeLoad`; already-started travel continues |
| Distress maintenance | ordered claims exceed capacity | ascending ID full-fit allocation; revoke losers once; revoked deadhead travel continues |
| Load on arrival | safe claim, carrier docked at source, complete validation succeeds | source stock minus gross; same-ID locked lot; claim released; loaded burn; `InTransit`; departure/events exactly once |
| Load integrity failure | safe claim but route/state/lot integrity invalid | release claim; remove contract; carrier remains docked; no lot/source subtraction; `RejectedBeforeLoad` once |
| Post-load cancellation/travel/transfer/liquidation | loaded active contract/lot | typed rejection and byte-for-byte physical state equality |

Player cancellation commands execute synchronously at the command boundary between steps. A successful cancellation therefore precedes the next travel/maintenance phase; after phase-6 loading it is rejected as post-load cancellation.

## D7 allocation and retry vectors

Constants: `P=4,000`, `B=20`, `F=40`, `N=3,940`, `R=20`.

| Attempt | Headroom | Cumulative before → after | Locked before → after | Reimbursement delta | Fee delta |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 2,000 | 0 → 2,000 | 4,000 → 1,960 | 20 | 20 |
| 2 | 1,000 | 2,000 → 3,000 | 1,960 → 950 | 0 | 10 |
| 3 | 940 | 3,000 → 3,940 | 950 → 0 | 0 | 10 |
| retry after each apply | unchanged state | unchanged | unchanged | 0 | 0 |

Reserve boundary: with headroom 3,939 on the first incomplete attempt, the maximum partial settlement is 3,921, leaving exactly 20 locked. Headroom 3,940 completes immediately as required by D7. The final 19 after the partial boundary may then complete. Zero headroom changes nothing and pays no reimbursement or fee. Converted allocation fills tank headroom first and places the exact overflow in owned bulk.

## D8 timeout and recovery ledgers

For arrival tick `A` and timeout 20, settlement attempts occur on `A..A+19`; timeout runs before any attempt on `A+20`.

| ID | State at timeout | Conversion/departure | Recovery arrival |
| --- | --- | --- | --- |
| `EL-D8-ZERO-01` | cumulative 0; locked 4,000 | convert `B=20` and `R=20`; burn 20; locked 3,960; no fee | deposit up to source headroom; ledger excess as recovery curtailment; terminate same ID |
| `EL-D8-PARTIAL-01` | cumulative 2,000; locked 1,960; prior reimbursement 20 and fee 20 | convert `R=20`; burn 20; locked 1,940; no new reimbursement/fee | return/curtail 1,940; terminate |
| `EL-D8-FULL-TANK-01` | tank full but owned bulk has room | conversion overflow enters owned bulk; tank still supplies reimbursed burn without changing total carrier-owned Energy | terminate normally |

No recovery creates a new contract or recovery loop.

## D12 deterministic order fixtures

Each fixture is run in forward, reverse, and stable pseudo-random insertion order and must produce identical stocks, lots, contracts, terminal events, ledgers, and counters.

- Energy intents: severity descending, runway ascending, payload descending, destination/source/carrier stable IDs.
- Pre-load maintenance: `ContractId` ascending.
- Settlement: destination ID, arrived tick, `ContractId` ascending.
- Recovery: source ID, recovery-arrival tick, `ContractId` ascending.
- Opportunity ties: score; kind (`EnergyContract` before ordinary only at exact equality); source; destination; good; carrier.
- Dynamic archetype scoring: build hypothetical candidates at each opportunity source using the archetype's initial tank/capacities. Require that source to fund the starting tank. Compare canonical opportunity score, then opportunity kind/source/destination/good and archetype stable ID. Spawn at the winning opportunity source. This avoids deriving demand from an already-existing compatible carrier.

A failed/stale Energy intent is not downsized and does not select ordinary work until the next tick.

## Ordinary Energy rejection and transfer matrix

Every row asserts typed rejection plus identical market stock, tank, owned bulk, locked lot, cargo, claims/reservations, travel, ledgers, events except the rejection event, and pending queues.

| Surface | `core:energy` expectation |
| --- | --- |
| Quote/bid/ask API | reject as not ordinarily tradable |
| Player local buy | reject |
| Player local sell | reject |
| Local trade limits | zero/typed unavailable, never an executable quote |
| Player reservation/`CommitTrade` | reject before queue mutation |
| NPC opportunity collection | omit Energy ordinary intents |
| Pending ordinary commitment resolution | reject stale/injected Energy intent |
| Funded sale executor | inaccessible to Energy contract settlement; direct Energy invocation rejects |
| Laden arrival settlement | reject Energy cargo and do not reroute it |
| Liquidation and retirement cleanup | never accept Energy generic cargo |
| Ordinary reroute/reposition path | never create Energy work |
| Generic cargo insertion | `core:energy` absent; locked/owned bulk are typed stores |
| Tank withdrawal, including NPC balancing | exact D1 exportable bound including active claims/export reserve |
| Tank deposit | exact amount and market storage headroom |
| Owned bulk → tank | exact amount and tank headroom |
| Owned bulk → current market | exact amount and storage headroom |
| Tank/market → owned bulk | no command/path |
| `RecordExternalDelivery` | remains allowed and ledgered as external inflow; never used by contract settlement |

Ordinary non-Energy goods retain existing quotes, reservations, funded partial settlement, liquidation, rerouting, and subsidies.

## App/TUI boundary expectations

- Typed requests: accept exact gross payload; cancel by `ContractId`; owned-bulk-to-tank; owned-bulk deposit.
- Player carrier is implicit. Requests contain stable domain IDs/quantities only.
- Structurally valid but stale acceptance returns a typed blocker and current maximum without mutation.
- Immutable views resolve names and expose request/offer, inbound commitment, runway/stage/cause, route facts, integer fee/allocation/net/freight/score, active state/progress/deadline, tank/owned/locked/general storage, and aggregate diagnostics.
- No ECS `Entity`, mutable world access, or float-based economic reconstruction crosses the app boundary.
- Both TUI layouts omit Energy bid/ask and render all D16 contract fields. Tank, owned bulk, locked bulk, and general cargo remain visually distinct.
- Unsupplied-life-support attribution is exactly one of `ArrivedSettlementBlocked`, `AcceptedDeliveryPending`, `NoReachableSurplus`, `NoViableCandidate`, or `ViableButUnaccepted`.

## Long-run and manual gates

| Gate | Pass condition | Evidence |
| --- | --- | --- |
| Workspace | format, tests, Clippy, content validation green | pending |
| 1,000 ticks | deterministic replay; exact reconciliation; nonzero accepted/completed contracts; late ordinary production/trade; no permanent claim/lot | pending |
| 10,000 ticks | exact reconciliation; no extinction/global ratchet; final-window ordinary and contract activity; no deadlock/recovery loop; all starvation ticks classified | pending |
| Player impact | exact baseline/intervention reconciliation and bounded stage/population divergence | pending |
| Manual contract trace | claim → deadhead/load → travel burn → settlement/allocation → destination | pending |
| Manual capacity trace | exact command rejection; partial settlement; deadline; recovery/curtailment visibility | pending |

Required metrics: gross/net delivery, loaded/deadhead/recovery burn, carrier fee/net profit per tick and bulk unit, fill/accept/arrival rates by stage, offer versus curtailment, request versus inbound, all five starvation causes, partial/wait/timeout/recovery/curtailment amounts, archetype counts/activity, and ordinary versus contract activity in the final window.

## Evidence log

### Baseline before Phase -1

- `cargo fmt --all -- --check`: pass.
- `cargo test --workspace`: pass (168 routine tests plus boundary/doc targets; one long content test ignored by default).
- `repository_content_loads_with_structural_roles`: pass.
- 1,000-tick ignored acceptance: pass; reconciliation difference 0, 45 stage transitions, 926,262 Energy loaded, 924,168 delivered, 5,790 trades after tick 300, 325 production operations after tick 300.

### Phase -1 seam verification

- Production behavior/API unchanged; root tests moved to `crates/game-core/src/tests.rs` and all 72 retained names pass.
- Workspace tests, game-core Clippy with warnings denied, repository content test, and 1,000-tick acceptance pass with the same concise metrics.

### Phase 0 failure-first evidence

- `energy_logistics::tests::el_d2_fee_floor_and_freight_ceil_vectors_are_exact` compiled and failed at the deliberate `EL-D2 integer fee and freight arithmetic` scaffold (`status 101`).
- The other first-wave tests share the same compile-safe scaffold and are not characterization tests: D1, D3, D4, D7, and D8 still call explicit unimplemented helpers.
- Wave 1A interfaces are frozen in `crates/game-core/src/energy_logistics/mod.rs`; expected values are main-owned in `crates/game-core/src/energy_logistics/tests.rs`.

### Implementation evidence

#### Wave 1A pure arithmetic

- Main rerun: all seven `energy_logistics::tests` pass; all 79 game-core tests pass.
- `cargo clippy -p game-core --all-targets -- -D warnings`, format check, and `git diff --check`: pass.
- Covered: D1 protection/projection, D2 integer fee/freight, D3 bounded gross sizing including tank recovery capacity, D4 positive profit utility, D7 retry/reserve arithmetic, and D8 timeout conversion preparation.

Pending lifecycle/content waves. Do not mark an invariant complete solely from a worker report; record the main-agent rerun here.
