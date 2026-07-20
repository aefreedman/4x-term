---
title: Engine Invariant Registry
type: reference
date: 2026-07-20
status: active
source_direction: docs/2026-07-20-testing-stance-correction.md
---
# Engine Invariant Registry

## Purpose

This registry is the reviewed source of truth for engine invariants during the
migration from the authored market prototype. An invariant is active only when
it has an exact oracle, an applicability rule, a non-vacuity witness, and
resolvable focused test evidence. A test name containing `invariant` does not
create a contract.

Tier 1 mechanism tests may protect useful behavior without making it a global
invariant. Tier 2 generated-world checks may fail only on an active applicable
entry below or on a constructive guarantee whose reserved entry has been made
active by its owning stage.

Changing an oracle, applicability rule, or status requires review of this file
with the implementation change. Do not add survival, activity, profitability,
population shape, or one repository universe as an oracle.

## Status vocabulary

- **Active:** applies whenever its applicability rule is met.
- **Conditional:** active only for the explicitly named setup; an assertion must
  prove the setup was entered before checking the oracle.
- **Reserved:** names future work but specifies no executable contract yet.

## Active invariants

### INV-ORDER-001 — Deterministic scheduled resolution

- **Status:** active.
- **Exact oracle:** Given identical initial state and inputs, changing only the
  insertion order covered by a resolution policy produces identical complete
  retained state and events. Where contention has a winner, the winner follows
  the documented stable ordering key rather than ECS insertion order.
- **Applicability:** A schedule or resolver advertises insertion-order
  independence for the participating entity/intent set.
- **Non-vacuity witness:** The fixture constructs at least two competing or
  simultaneously settled intents and asserts that a transfer, settlement, or
  recovery transition occurs.
- **Current Tier 1 evidence:**
  - `energy_intent_contention_is_insertion_order_invariant`
  - `destination_settlement_order_is_insertion_order_invariant`
  - `recovery_arrival_order_is_insertion_order_invariant`
  - `same_tick_contention_winner_is_invariant_to_trader_insertion_order`
  - `population_updates_are_atomic_and_insertion_order_invariant`
- **Failure evidence:** The assertion reports the ordering variant and unequal
  snapshots/events or winner identifiers.
- **Generated-world applicability:** Stage 6 may run the same oracle only for a
  generated setup that proves applicable contention or simultaneous work.

### INV-ENERGY-001 — Exact physical Energy reconciliation

- **Status:** active.
- **Exact oracle:** For the measured interval, the checked sum of initial stored
  and in-flight Energy plus generation and recorded external inflow equals the
  checked sum of final stored and in-flight Energy plus every named sink and
  curtailment channel. The difference is exactly zero.
- **Applicability:** Every retained simulation path that mutates physical
  Energy. A path with external delivery must record that inflow explicitly.
- **Non-vacuity witness:** The fixture records a nonzero generation, transfer,
  burn, curtailment, or external-inflow channel and asserts a corresponding
  state or ledger delta before reconciling.
- **Current Tier 1 evidence:**
  - helper `assert_physical_delta_reconciles`
  - `full_energy_delivery_settles_net_and_allocation_exactly_once`
  - `zero_energy_destination_receives_contract_energy_without_prepayment`
  - `partial_delivery_retries_then_recovers_same_contract`
  - `zero_settlement_timeout_returns_or_curtails_every_locked_unit`
  - `energy_flow_reconciles_external_delta`
  - `recorded_external_delivery_is_atomic_and_reconciles_a_stage_intervention`
- **Failure evidence:** Expected, actual, difference, and every included source,
  sink, store, in-flight amount, and external channel.
- **Generated-world applicability:** Active in Stage 6 for every generated run
  that mutates physical Energy.

### INV-ATOMIC-001 — Validate before mutate

- **Status:** active.
- **Exact oracle:** If any rule or checked arithmetic operation rejects a
  mutation, all affected domain state, ledgers, reservations, events, and ID
  allocators equal their pre-operation values. On success, validated values are
  applied together and success events are emitted only afterward.
- **Applicability:** Every operation advertised as atomic across more than one
  field, resource, entity, ledger, event stream, reservation, or ID allocator.
- **Non-vacuity witness:** The fixture reaches the mutation boundary, forces a
  named late validation or arithmetic failure, and compares complete affected
  snapshots and events before and after rejection.
- **Current Tier 1 evidence:**
  - `dynamic_generated_namespace_collision_is_rejected_at_startup_and_atomic_at_runtime`
  - `dynamic_spawn_overflows_are_atomic_and_retry_uses_unique_monotonic_ids`
  - `preload_arithmetic_failure_propagates_without_terminalizing_contract`
  - `settlement_timeout_and_recovery_failures_are_atomic`
  - `buy_tank_transfer_and_travel_are_atomic_on_ledger_overflow`
  - `brownout_history_overflow_is_atomic`
- **Failure evidence:** The rejected operation and a field-by-field or structural
  snapshot difference for every advertised mutation surface.
- **Generated-world applicability:** Usually remains Tier 1 evidence. A Stage 6
  generated check is applicable only when the harness deliberately forces the
  registered rejection path.

### INV-ARITH-001 — Checked quantity arithmetic

- **Status:** active.
- **Exact oracle:** Quantity addition, subtraction, multiplication, conversion,
  and rounding either return the mathematically defined in-range integer result
  or a typed error; overflow, underflow, division by zero, and invalid ranges do
  not wrap, saturate silently, or partially mutate state.
- **Applicability:** Physical-resource, capacity, cost, duration, counter, and ID
  arithmetic represented by bounded integers.
- **Non-vacuity witness:** Each operation has at least one exact accepted case
  and one explicit invalid or boundary case.
- **Current Tier 1 evidence:**
  - `el_inv_lot_bulk_usage_and_headroom_are_checked`
  - `el_inv_claim_contract_ids_are_monotonic_and_atomic_on_overflow`
  - `logistic_population_delta_rejects_invalid_inputs_without_mutating_carry`
  - `fixed_point_generation_checks_ranges_rounding_and_overflow`
- **Failure evidence:** Operation, operands, expected result or error, and actual
  result; atomic callers additionally satisfy INV-ATOMIC-001.
- **Generated-world applicability:** Active wherever generated values enter the
  applicable arithmetic; range-validation failures must include provenance.

### INV-ID-001 — Stable monotonic domain identifiers

- **Status:** active.
- **Exact oracle:** Stable content IDs resolve independently of ECS entity IDs.
  For a dynamic domain allocator, each accepted creation receives the next
  unique monotonic ID; rejection consumes no ID, and allocator overflow rejects
  atomically.
- **Applicability:** Stable external/content references and domains that support
  dynamic creation.
- **Non-vacuity witness:** The fixture creates at least two accepted objects and
  forces one rejected or overflowing allocation between observable allocator
  snapshots where the domain supports rejection.
- **Current Tier 1 evidence:**
  - `el_inv_claim_contract_ids_are_monotonic_and_atomic_on_overflow`
  - `dynamic_generated_namespace_collision_is_rejected_at_startup_and_atomic_at_runtime`
  - `dynamic_spawn_obeys_cooldown_and_monotonic_ids`
  - `dynamic_spawn_overflows_are_atomic_and_retry_uses_unique_monotonic_ids`
  - `dynamic_trader_namespace_collision_reports_source_context`
- **Failure evidence:** Allocator state, accepted/rejected IDs, and any duplicate
  or skipped identifier.
- **Generated-world applicability:** Stable generated content identity belongs
  to Stage 6 replay identity; this entry does not define generator versioning.

### INV-VALIDATE-001 — Deterministic source-aware validation

- **Status:** active.
- **Exact oracle:** Retained schemas reject duplicate IDs, unresolved
  references, malformed or out-of-range values, and independent semantic
  failures with deterministic diagnostics containing source, definition ID when
  available, and field path. Independent failures are aggregated rather than
  hidden by an unrelated first error.
- **Applicability:** Every retained authored input schema and, once introduced,
  validated generator configuration. Obsolete market-quality predicates are not
  retained schema rules.
- **Non-vacuity witness:** A fixture injects at least two independent failures in
  explicitly named source records and asserts both exact contexts.
- **Current Tier 1 evidence:**
  - `removed_energy_import_priorities_report_exact_source_contexts`
  - `malformed_world_dynamics_report_source_contexts`
  - `malformed_energy_logistics_policy_reports_exact_source_contexts`
  - `duplicate_archetype_id_reports_source_context`
  - `investment_effect_bound_errors_retain_source_context`
  - `dynamic_trader_namespace_collision_reports_source_context`
  - `nonzero_seasonal_amplitude_rejects_odd_period_with_source_context`
  - `duplicate_market_schedules_report_source_contexts`
  - `rejects_duplicate_recipe_inputs_and_outputs_with_source_context`
  - `graph_errors_aggregate_with_independent_schema_errors`
- **Failure evidence:** Deterministically ordered diagnostics with source, ID,
  field, and rejected value/reference as applicable.
- **Generated-world applicability:** Stage 4 extends this entry to generator
  parameters and generated output only after their schemas exist.

### INV-LOGISTICS-001 — Bounded recovery of accepted physical work

- **Status:** conditional.
- **Exact oracle:** Once retained automated logistics accepts and locks a
  nonzero physical lot, every configured timeout/recovery transition preserves
  exact accounting and reaches one of its explicitly finite terminal outcomes
  within the mechanism's configured bounds; no locked unit is stranded or
  duplicated.
- **Applicability:** An accepted/loaded carrier or active claim exists, a
  nonzero lot is locked, and the configured recovery path is exercised. Empty
  fleets, zero work, rejected creation, and ordinary commercial profitability
  are outside this invariant.
- **Non-vacuity witness:** Tests assert the active contract/locked quantity and
  then observe retry, recovery arrival, return, or curtailment before checking
  the terminal state and INV-ENERGY-001.
- **Current Tier 1 evidence:**
  - `recovery_arrival_order_is_insertion_order_invariant`
  - `partial_delivery_retries_then_recovers_same_contract`
  - `zero_settlement_timeout_returns_or_curtails_every_locked_unit`
  - `settlement_timeout_and_recovery_failures_are_atomic`
- **Failure evidence:** Contract/claim ID, locked quantity, configured timeout
  and recovery bound, transitions observed, terminal outcome, and complete
  reconciliation channels.
- **Generated-world applicability:** Deferred until retained player-owned
  automation exists. Stage 6 must prove the applicability setup before running
  this oracle.

## Reserved entries

### G18-ORIGIN-SURPLUS — Constructed origin solvency

- **Status:** reserved for Stage 4.
- **Undefined here:** Units, surplus margin, time horizon, starting stores,
  required inputs, inequality, and generator configuration identity.
- **No Stage 2 test may claim this guarantee.** Legacy bootstrap solvency and
  market runway are not substitutes.

### G18-NEIGHBORHOOD-AFFORDANCE — Constructed nearby expansion path

- **Status:** reserved for Stage 4.
- **Undefined here:** Starting range, resource floor, reclaimable-site types,
  affordability equation, topology requirement, and generation identity.
- **No Stage 2 test may claim this guarantee.** Authored exporter/importer roles
  and nearest-three market routes are not substitutes.

### INV-REPLAY-001 — Complete generated-world replay identity

- **Status:** reserved for Stage 6.
- **Undefined here:** Generator version, validated-configuration fingerprint,
  seed/state representation, event-log requirements, and compatibility rules.
- **A seed alone is not a complete replay identity.**

## Stage 2 migration classification

This matrix classifies the Stage 1 audit families. It is not a requirement to
replace removed tests one-for-one.

| Family | Classification | Stage 2 disposition |
| --- | --- | --- |
| Checked arithmetic, overflow, conservation, atomic rejection, stable ordering and IDs | Active invariant/Tier 1 | Retain focused exact tests under the entries above. |
| Source-aware schema, duplicate/reference, range, and aggregated validation | Active invariant/Tier 1 | Retain focused local-source tests; remove market-quality predicates. |
| Recipe, seasonal, brownout, population, route, and logistics arithmetic | Tier 1 mechanism or conditional invariant | Keep only small hand-computable tests for responsibilities still consumed. Do not preserve current balance constants. |
| Exact-20, authored identities/counts, system roles, source correlation, nonuniform distances | Obsolete premise | Delete validation and tests. |
| Bootstrap market runway, commercial liquidation adequacy, and Energy numeraire cost `1` | Obsolete premise | Delete gates; retain only independently consumed checked arithmetic. |
| Repository content structure/activity smoke and ignored long acceptance | Obsolete premise | Delete without replacement. |
| Metastability, universal survival, population shape/ratchets, trade churn, NPC profitability/activity | Obsolete premise | Delete validators, summaries, tests, and CI gates. |
| Pricing comparison and required player-impact divergence | Obsolete premise | Delete probes and modes; focused external-inflow reconciliation remains under INV-ENERGY-001. |
| Authored headless play, content-validation command, multi-hop player trade acceptance | Obsolete product acceptance | Delete commands/tests/CI gates. Headless architecture is not the legacy command. |
| Legacy economy reports and texture fields | Obsolete diagnostic | Delete. Stage 6 may design new non-gating generated-world diagnostics. |
| G18 origin and neighborhood guarantees | Deferred constructive guarantee | Reserved for Stage 4; no invented Stage 2 values or fixtures. |
| Generated-world invariant soaks and replay identity | Deferred generated coverage | Reserved for Stage 6. |

## Review checklist

For every proposed active or conditional entry:

1. Is the oracle exact rather than statistical or descriptive?
2. Is applicability explicit, and does the cited fixture assert it?
3. Does the fixture produce relevant work rather than pass by doing nothing?
4. Does failure output identify expected and actual state plus required replay
   or source context?
5. Is the responsibility current, or is the test preserving deleted gameplay?
6. For atomic mutation, are all rule and arithmetic checks complete before any
   state or event mutation?
