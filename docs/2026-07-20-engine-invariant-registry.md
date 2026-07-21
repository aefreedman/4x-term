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
- **Exact oracle:** Given semantically identical substrate input, changing only
  resource, location, deposit, site, or topology-edge order, including
  undirected endpoint order, produces an equal normalized `WorldDefinition` and
  an equal complete `WorldSnapshot` after instantiation.
- **Applicability:** Stage 3 RON compilation and `WorldState` construction, whose
  input collection order is not domain state.
- **Non-vacuity witness:** The fixtures contain three locations and nonempty
  resources, deposits, sites, and topology edges before permutation and
  comparison.
- **Current Tier 1 evidence:**
  - `input_permutations_produce_equal_snapshots`
  - `normalizes_permuted_input`
- **Failure evidence:** The unequal compiled definition or complete snapshot and
  the input permutation that produced it.
- **Generated-world applicability:** A future generator must register its own
  ordering inputs and replay identity before extending this oracle.

### INV-ENERGY-001 — Exact physical Energy reconciliation

- **Status:** active.
- **Exact oracle:** A successful nonzero `core:energy` resource transfer reduces
  the source and increases the destination by exactly the transferred quantity,
  increases the successful-flow ledger by that quantity, and leaves the exact
  checked source-plus-destination total unchanged. The current fixture proves
  `9 + 4 = 4 + 9` for a transfer and ledger delta of `5`.
- **Applicability:** The current applicable Energy mutation is a successful
  nonzero `transfer_resource` operation for `core:energy`.
- **Non-vacuity witness:** The fixture transfers five units and asserts the
  ledger delta is nonzero before checking exact conservation.
- **Current Tier 1 evidence:**
  - `energy_transfer_reconciles_exactly`
- **Failure evidence:** Resource ID, quantity, initial and final source and
  destination stores, ledger delta, and the unequal totals.
- **Generated-world applicability:** Future Energy sources, sinks, in-flight
  stores, or external channels require reviewed extensions to this oracle when
  those operations exist.

### INV-ATOMIC-001 — Validate before mutate

- **Status:** active.
- **Exact oracle:** A rejected `transfer_resource` call leaves its complete
  source store, destination store, and successful-flow ledger exactly equal to
  their pre-operation values.
- **Applicability:** Every current transfer rejection path: zero quantity,
  insufficient source quantity, destination overflow, and ledger overflow.
- **Non-vacuity witness:** Each case reaches `transfer_resource`, forces its
  named rejection, and compares all three affected values with a saved tuple.
- **Current Tier 1 evidence:**
  - `resource_transfer_rejections_are_atomic_on_every_path`
- **Failure evidence:** The rejection and structural before/after difference for
  the source, destination, and ledger.
- **Generated-world applicability:** A future multi-surface mutation needs its
  own applicable rejection setup and complete affected-state comparison.

### INV-ARITH-001 — Checked quantity arithmetic

- **Status:** active.
- **Exact oracle:** `Energy` addition, subtraction, and multiplication return the
  exact in-range integer result or `CoreError::Overflow`. Resource-transfer
  source subtraction, destination addition, and ledger addition reject
  underflow or overflow before any affected value changes.
- **Applicability:** The current `Energy` checked methods and the bounded `u64`
  arithmetic at the atomic physical-resource transfer boundary.
- **Non-vacuity witness:** Exact accepted Energy cases and explicit add,
  subtract, multiply, and conversion overflow cases are exercised; transfer
  fixtures force insufficient-source, destination-overflow, and ledger-overflow
  boundaries.
- **Current Tier 1 evidence:**
  - `energy_arithmetic_is_checked`
  - `resource_transfer_rejections_are_atomic_on_every_path`
- **Failure evidence:** Operation, operands, expected exact result or typed
  error, actual result, and any changed transfer state on rejection.
- **Generated-world applicability:** Generated values become applicable only
  when a generator or consuming operation exists.

### INV-ID-001 — Stable content identifiers and references

- **Status:** active for stable content identity; dynamic allocation is
  reserved until a dynamic domain exists.
- **Exact oracle:** An accepted `ContentId` round-trips unchanged through
  `as_str` and `Display`, malformed IDs return `CoreError::InvalidId`, and a
  successfully compiled substrate resolves its stable location and resource
  references before `WorldState` instantiation. Stable IDs do not depend on ECS
  entity IDs.
- **Applicability:** Stage 3 content IDs and references in resource, location,
  origin, deposit, site, and topology definitions.
- **Non-vacuity witness:** The ID fixture checks accepted and rejected strings;
  the content fixture compiles three locations with an origin, deposit, site,
  and edge and successfully instantiates the result.
- **Current Tier 1 evidence:**
  - `content_id_validation_and_display_are_stable`
  - `compiles_a_dead_isolated_location_and_instantiates_world_state`
- **Failure evidence:** Raw ID or reference, expected accepted/rejected result,
  and the unresolved definition/field context where applicable.
- **Reserved dynamic clause:** If a domain later supports dynamic creation, its
  monotonicity, rejection, and allocator-overflow contract must be specified and
  evidenced then; Stage 3 has no dynamic allocator.
- **Generated-world applicability:** Stable generated identity remains part of
  reserved replay work and does not define generator versioning here.

### INV-VALIDATE-001 — Deterministic source-aware validation

- **Status:** active.
- **Exact oracle:** Stage 3 world-source parsing reports document provenance for
  syntax failures and rejects unknown top-level or nested fields. Semantic
  compilation rejects malformed/duplicate IDs, unresolved references,
  non-finite positions, zero required values, and invalid topology while
  returning all independent diagnostics in exact sorted source, definition,
  field, and message order, independent of duplicate-record order.
- **Applicability:** The retained single-document Stage 3 RON world schema.
- **Non-vacuity witness:** One fixture injects independent failures across
  resources, locations, origin, deposits, sites, and topology and compares all
  thirteen exact diagnostics; a separate malformed document proves parse
  provenance.
- **Current Tier 1 evidence:**
  - `aggregates_exact_source_aware_diagnostics`
  - `parse_errors_include_document_provenance`
  - `unknown_fields_are_rejected_in_top_level_and_nested_sources`
  - `location_diagnostics_are_complete_and_permutation_independent`
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

## Stage 3 completion evidence and classification

Stage 3 completed on 2026-07-20. The retained workspace contains only
`game-core` and `game-content`: nine core tests plus six content tests, for 15
focused Tier 1 tests and no ignored tests. `game-app`, `game-tui`, `game-cli`,
and the production `content/` bundle are absent. The exact names under the
active entries above resolve in the retained crates; no removed market, trader,
fleet, population, or commercial-contract test is current evidence.

| Current family | Stage 3 classification | Current evidence or disposition |
| --- | --- | --- |
| Substrate compile/instantiation ordering | Active invariant/Tier 1 | Nonempty location, deposit, site, and edge permutations normalize exactly under INV-ORDER-001. |
| Physical `core:energy` transfer | Active invariant/Tier 1 | Exact nonzero transfer conservation and successful-flow ledger delta under INV-ENERGY-001. |
| Checked Energy and transfer arithmetic | Active invariant/Tier 1 | Exact accepted/boundary arithmetic and atomic rejection under INV-ARITH-001 and INV-ATOMIC-001. |
| Stable content IDs and references | Active invariant/Tier 1 | Stable parse/display plus successful compile/instantiation under INV-ID-001; dynamic allocation is reserved. |
| Stage 3 RON validation | Active invariant/Tier 1 | Exact aggregated semantic diagnostics and parse provenance under INV-VALIDATE-001. |
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
