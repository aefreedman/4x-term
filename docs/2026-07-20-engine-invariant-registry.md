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

### INV-ORDER-001 — Deterministic substrate normalization

- **Status:** active.
- **Exact oracle:** Given semantically identical input, changing only resource,
  location, system, stock, body, slot, deposit, site, or topology-edge order,
  including undirected endpoint order, produces an equal normalized definition
  and complete snapshot whenever that order is not domain state.
- **Applicability:** Stage 3/4 RON compilation and `WorldState` construction;
  construction-queue order remains intentional domain state.
- **Non-vacuity witness:** The fixtures contain three locations and nonempty
  resources, deposits, sites, and topology edges before permutation and
  comparison.
- **Current Tier 1 evidence:**
  - `input_permutations_produce_equal_snapshots`
  - `resource_engine_definition_order_does_not_change_state`
  - `stage4_input_permutations_compile_to_the_same_definition`
- **Failure evidence:** The unequal compiled definition or complete snapshot and
  the input permutation that produced it.
- **Generated-world applicability:** A future generator must register its own
  ordering inputs and replay identity before extending this oracle.

### INV-ENERGY-001 — Exact physical Energy reconciliation

- **Status:** active.
- **Exact oracle:** Every applicable `core:energy` source, sink, retention, and
  overflow channel reconciles exactly. Generic transfers conserve source plus
  destination and advance the flow ledger; capacity-aware receipts reconcile
  source decrease as retained plus overflow; the 20-tick fixture reconciles
  `10 starting + 560 produced = 30 construction + 100 upkeep + 110 retained +
  330 overflow`.
- **Applicability:** Successful generic Energy transfers and enabled Stage 4
  system receipts/ticks with complete accounting evidence.
- **Non-vacuity witness:** One fixture transfers five units and asserts a
  nonzero ledger delta; the bootstrap produces `560` Energy and exercises
  construction, upkeep, retention, and nonzero overflow.
- **Current Tier 1 evidence:**
  - `energy_transfer_reconciles_exactly`
  - `incoming_energy_at_capacity_reconciles_retained_and_overflow`
  - `cancellation_refund_overflow_is_atomic_and_explicit`
  - `exact_twenty_tick_bootstrap_matches_the_approved_fixture`
- **Failure evidence:** Resource ID, quantity, initial and final source and
  destination stores, ledger delta, and the unequal totals.
- **Generated-world applicability:** Future Energy sources, sinks, in-flight
  stores, or external channels require reviewed extensions to this oracle when
  those operations exist.

### INV-ATOMIC-001 — Validate before mutate

- **Status:** active.
- **Exact oracle:** Every rejected transfer, system receipt, construction
  enqueue/cancellation, resource-engine definition, and complete tick leaves all
  affected stocks, deposits, cycles, commitments, reservations, sequences,
  ledgers, overflow evidence, and time equal to their pre-operation values.
- **Applicability:** Typed invalid-command, invalid-state, insufficiency, ID/
  reference, and checked-arithmetic rejection paths; routine scarcity is an
  advancing gameplay outcome rather than a rejection.
- **Non-vacuity witness:** Focused cases reach each named operation, force its
  rejection after supplying otherwise-applicable state, and compare complete
  affected before/after values.
- **Current Tier 1 evidence:**
  - `resource_transfer_rejections_are_atomic_on_every_path`
  - `rejected_enqueue_and_sequence_overflow_leave_everything_unchanged`
  - `extractor_reservation_cancellation_release_and_begun_rejection_are_atomic`
  - `transfer_to_system_rejects_unknown_resource_atomically`
  - `stage3_world_snapshots_but_tick_rejects_atomically`
- **Failure evidence:** The typed rejection and structural before/after
  difference for every affected store, system/deposit state, queue/reservation,
  sequence, ledger, evidence record, and time value.
- **Generated-world applicability:** A future multi-surface mutation needs its
  own applicable rejection setup and complete affected-state comparison.

### INV-ARITH-001 — Checked quantity arithmetic

- **Status:** active.
- **Exact oracle:** Physical-resource subtraction, addition, multiplication,
  sequence allocation, capacity derivation, accounting totals, and tick/work
  progression either produce their exact `u64` result or reject with
  `CoreError::Overflow`/the typed insufficiency error before affected state
  changes.
- **Applicability:** Generic transfers and every Stage 4 resource-engine command
  or tick path that changes stocks, commitments, progress, deposits, capacity,
  overflow, or ledgers.
- **Non-vacuity witness:** Fixtures force source insufficiency, destination and
  ledger overflow, queue-sequence overflow, maximum-capacity cancellation, and
  accepted exact reconciliation.
- **Current Tier 1 evidence:**
  - `resource_transfer_rejections_are_atomic_on_every_path`
  - `rejected_enqueue_and_sequence_overflow_leave_everything_unchanged`
  - `cancellation_refund_at_max_capacity_does_not_overflow_arithmetic`
- **Failure evidence:** Operation, operands, expected exact result or typed
  error, actual result, and complete affected before/after state.
- **Generated-world applicability:** Generated values become applicable only
  when a generator or consuming operation exists.

### INV-ID-001 — Stable content identifiers and references

- **Status:** active for stable authored identity and Stage 4 constructed-development allocation.
- **Exact oracle:** An accepted `ContentId` round-trips unchanged through
  `as_str` and `Display`, malformed IDs reject, authored references resolve
  before instantiation, and each accepted construction reserves a deterministic
  system-scoped ID that cannot collide with installed or queued developments.
  Stable IDs do not depend on ECS entity IDs.
- **Applicability:** Stage 3/4 content IDs and references plus deterministic,
  system-scoped constructed-development IDs reserved at enqueue.
- **Non-vacuity witness:** The ID fixture checks accepted and rejected strings;
  the content fixture compiles three locations with an origin, deposit, site,
  and edge and successfully instantiates the result.
- **Current Tier 1 evidence:**
  - `content_id_validation_and_display_are_stable`
  - `compiles_stage3_system_stocks_without_enabling_the_engine`
  - `authored_development_id_collision_rejects_enqueue_atomically`
  - `constructed_development_ids_are_deterministic_and_unique_across_systems`
- **Failure evidence:** Raw ID or reference, expected accepted/rejected result,
  unresolved definition/field context, or constructed ID collision and complete
  pre/post command state.
- **Generated-world applicability:** Stable generated identity remains part of
  reserved replay work and does not define generator versioning here.

### INV-VALIDATE-001 — Deterministic source-aware validation

- **Status:** active.
- **Exact oracle:** Stage 3/4 world-source parsing reports document provenance
  for syntax failures and rejects unknown top-level or nested fields. Semantic
  compilation rejects malformed/duplicate IDs, unresolved references,
  non-finite positions, zero required tuning, invalid profiles/recipes/
  development assignments, and invalid topology while returning independent
  diagnostics in sorted source, definition, field, and message order.
- **Applicability:** The retained single-document Stage 3/4 RON world schema.
- **Non-vacuity witness:** Fixtures inject independent Stage 3 and Stage 4
  failures across resources, locations, systems, stocks, tuning, bodies, slots,
  developments, deposits, sites, and topology; a malformed document separately
  proves parse provenance.
- **Current Tier 1 evidence:**
  - `invalid_stage4_content_reports_complete_sorted_source_aware_diagnostics`
  - `parse_errors_include_document_provenance`
  - `unknown_fields_are_rejected_at_top_level_and_deeply_nested`
  - `stage4_input_permutations_compile_to_the_same_definition`
- **Failure evidence:** The complete ordered diagnostic list, including source,
  definition, field, and message.
- **Generated-world applicability:** Stage 4b may extend this entry only after
  a generator configuration or output-validation schema exists.

## Reserved entries

### INV-LOGISTICS-001 — Bounded recovery of accepted physical work

- **Status:** reserved until player-owned automated logistics exists.
- **Undefined here:** Accepted-work state, locked physical lot, carrier/claim
  identity, timeout and recovery transitions, finite terminal outcomes, and
  complete accounting channels.
- **No current evidence:** Stage 3 contains no carrier, claim, contract, fleet,
  or automated logistics operation. Removed commercial logistics tests are not
  evidence for a future player-owned mechanism.

### G18-ORIGIN-STRUCTURE — Constructed origin prerequisites

- **Status:** reserved for Stage 4b.
- **Undefined here:** Exact mandatory origin records, references, placement
  relationships, and generator identity derived from completed Stage 4 gameplay.
- **Excluded oracles:** Economic solvency, seasonal surplus, affordability,
  tick-zero action availability, long-run survival, and favorable quantities.
- **No current test may claim this guarantee.** The authored Stage 4 bootstrap
  fixture is mechanism evidence, not a universal generator floor.

### G18-NEIGHBORHOOD-STRUCTURE — Constructed nearby expansion prerequisites

- **Status:** reserved for Stage 4b.
- **Undefined here:** The first bounded outward action, required nearby element
  kinds, scopes, references, topology relationships, and whether the unchanged
  standalone site record is structurally required.
- **Excluded oracles:** Resource quantity or affordability floors, favorable
  distribution, and a mandatory reclaimable site unsupported by gameplay.
- **No current test may claim this guarantee.** Authored exporter/importer roles
  and nearest-three market routes are not substitutes.

### INV-GENERATION-IDENTITY-001 — Complete generation identity

- **Status:** reserved for Stage 4b.
- **Undefined here:** Generator version, seed representation, validated
  configuration fingerprint, source provenance, stable generated identity, and
  compatibility rules for reproducing generated output.
- **A seed alone is not a complete generation identity.**

### INV-REPLAY-001 — Complete runtime replay identity

- **Status:** reserved for Stage 6.
- **Undefined here:** Complete generation identity, initial runtime state,
  event-log requirements, runtime compatibility, and replay verification.
- **Generation identity alone is not a complete runtime replay identity.**

## Stage 4 completion evidence and classification

Stage 4 completed on 2026-07-20. The retained workspace contains only
`game-core` and `game-content`: 31 core tests plus nine content tests, for 40
focused Tier 1 tests and no ignored tests. `game-app`, `game-tui`, `game-cli`,
generated-world code, outward actions, and the production `content/` bundle are
absent. The exact names under active entries resolve in the retained crates; no
removed market, trader, fleet, or commercial-contract test is current evidence.

| Current family | Stage 4 classification | Current evidence or disposition |
| --- | --- | --- |
| Substrate compile/instantiation ordering | Active invariant/Tier 1 | Nonempty location, deposit, site, and edge permutations normalize exactly under INV-ORDER-001. |
| Physical `core:energy` transfer | Active invariant/Tier 1 | Exact nonzero transfer conservation and successful-flow ledger delta under INV-ENERGY-001. |
| Checked resource-engine arithmetic | Active invariant/Tier 1 | Exact transfer, sequence, capacity, commitment, tick, and accounting boundaries under INV-ARITH-001 and INV-ATOMIC-001. |
| Stable content IDs and references | Active invariant/Tier 1 | Stable parse/display plus successful compile/instantiation under INV-ID-001; dynamic allocation is reserved. |
| Stage 3/4 RON validation | Active invariant/Tier 1 | Exact aggregated semantic diagnostics, strict nested fields, and parse provenance under INV-VALIDATE-001. |
| Authored origin resource engine | Tier 1 mechanism evidence | Exact 20-tick bootstrap, shortages, conditions, production cycles, construction FIFO/reservations/cancellation, overflow, and deterministic role/body/slot ordering. |
| Automated logistics | Reserved | No current mechanism or evidence; future player-owned automation must define a new applicable contract. |
| G18 origin/neighborhood structural guarantees | Reserved | Stage 4b owns exact mandatory records and placement/relationship oracles; economic inequalities are excluded. |
| Generated-world identity | Reserved | Stage 4b owns complete generation identity; Stage 6 owns full runtime event-log replay. |
| Market/trader/fleet/pricing/population/legacy acceptance | Removed obsolete surface | No legacy test is retained as current invariant evidence. |

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
