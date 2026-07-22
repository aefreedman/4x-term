---
title: Engine Invariant Registry
type: reference
date: 2026-07-20
status: active
source_direction: docs/plans/2026-07-20-testing-stance-correction.md
---
# Engine Invariant Registry

## Purpose

This registry is the reviewed source of truth for current engine invariants. An
invariant is active only when it has an exact oracle, an applicability rule, a
non-vacuity witness, and resolvable focused test evidence. A test name containing
`invariant` does not create a contract.

Tier 1 mechanism tests may protect useful behavior without making it a global
invariant. A generated-world check may fail only on an active applicable entry
below or on the active constructive origin guarantee. Do not add survival,
activity, profitability, population shape, exact frontier count, connectivity,
or one generated universe as an oracle.

Changing an oracle, applicability rule, or status requires review of this file
with the implementation change.

## Status vocabulary

- **Active:** applies whenever its applicability rule is met.
- **Conditional:** active only for the explicitly named setup; an assertion must
  prove the setup was entered before checking the oracle.
- **Reserved:** names future work but specifies no executable contract yet.

## Active invariants

### INV-ORDER-001 — Deterministic normalization and stable semantic order

- **Status:** active.
- **Exact oracle:** Semantically identical unordered definition collections and
  normalized profile maps produce equal normalized definitions, canonical
  bytes, fingerprints, and complete diagnostic snapshots. Authored body/slot
  order, FIFO queue order, and route tie-breaking order remain intentional state.
- **Applicability:** Stage 4b authored-world/profile compilation,
  `WorldState::new`, and deterministic geometric route selection.
- **Non-vacuity witness:** Fixtures permute nonempty definition collections and
  profile resources/rational representations; a separate fixture preserves
  non-lexical body/slot order, and an equal-distance route has two candidates.
- **Current Tier 1 evidence:**
  - `unordered_definition_collection_permutations_produce_equal_snapshots`
  - `normalized_map_retains_semantic_body_and_slot_order_separately_from_runtime`
  - `semantic_input_permutations_have_identical_canonical_bytes_and_fingerprint`
  - `shortest_route_uses_stable_sequence_tie_break_and_redacts_hidden_stops`
- **Failure evidence:** The unequal normalized value/snapshot/canonical bytes,
  the input permutation, or the selected route and candidate sequence.

### INV-ENERGY-001 — Exact physical Energy reconciliation

- **Status:** active.
- **Exact oracle:** Every applicable `core:energy` production, commitment,
  refund, operation, travel, founding receipt, retention, overflow, and loss
  channel reconciles exactly. Capacity-aware receipts and cancellation record
  retained and overflow quantities explicitly; no received quantity is counted
  twice.
- **Applicability:** Successful Stage 4b commands and ticks with complete
  resource-accounting evidence.
- **Non-vacuity witness:** The retained bootstrap produces and spends Energy;
  expansion fixtures enqueue, cancel, complete, launch, settle one expedition,
  lose another, and assert nonzero production, travel, overflow, receipt, and
  loss channels.
- **Current Tier 1 evidence:**
  - `retained_stage4_bootstrap_runs_on_body_resources_and_global_time`
  - `construction_cancellation_refunds_and_never_reuses_project_ids`
  - `complete_knowledge_reserves_typed_slots_and_success_unlocks_only_after_report`
  - `project_cancel_complete_launch_arrival_overflow_and_loss_reconcile_exactly`
  - `two_system_tick_orders_production_arrival_observation_receipt_and_retention_exactly`
- **Failure evidence:** Resource ID, initial/final stores, every accounting
  channel, overflow evidence, and unequal expected/actual totals.

### INV-ATOMIC-001 — Validate before complete-state mutation

- **Status:** active.
- **Exact oracle:** Every rejected world construction, command, transmission
  merge, or tick leaves all affected and globally coordinated state equal to its
  pre-operation value. A tick commits only after all ten phases, runtime
  integrity validation, and player-view construction succeed.
- **Applicability:** Typed invalid-state/reference/arithmetic/insufficiency
  rejection paths. Routine scarcity and typed founding loss are advancing
  gameplay outcomes, not operation rejections.
- **Non-vacuity witness:** Focused fixtures force launch insufficiency,
  reservation collision, immutable-fact contradiction, and failures in movement,
  due-message receipt, retention, and a late system phase after earlier work.
- **Current Tier 1 evidence:**
  - `no_population_and_insufficient_launch_energy_reject_without_mutation`
  - `begun_shipyard_project_cancellation_rejects_without_mutating_world`
  - `complete_knowledge_reserves_typed_slots_and_success_unlocks_only_after_report`
  - `immutable_contradiction_rejects_whole_transmission_and_duplicate_is_idempotent`
  - `late_system_failure_rolls_back_the_whole_world_and_clock`
  - `forced_movement_failure_rolls_back_earlier_phases_clock_and_counters`
  - `forced_due_message_failure_rolls_back_movement_clock_and_observer_counter`
  - `forced_retention_failure_rolls_back_population_id_allocation_and_clock`
- **Failure evidence:** Typed rejection plus a structural before/after difference
  covering time, counters, systems, body resources, queues/reservations, assets,
  populations, transit, knowledge, accounting, and evidence.

### INV-ARITH-001 — Checked quantity, distance, rate, and identity arithmetic

- **Status:** active.
- **Exact oracle:** Resource quantities, accounting totals, progress, counters,
  coordinate distance, travel duration/cost, and communication delay either
  produce the exact integer result or return the typed overflow/insufficiency
  error before affected state changes. Distance and rate rounding use the
  specified ceiling arithmetic.
- **Applicability:** Every Stage 4b core command/tick and generator/profile path
  using checked physical quantities, fixed-point positions, rates, or stable
  sequences.
- **Non-vacuity witness:** Tests cover a 3-4-12 distance, extreme coordinate
  overflow, jump-boundary equality, fractional ceiling, positive communication
  delay, sequence overflow, and late retention overflow.
- **Current Tier 1 evidence:**
  - `squared_distance_is_exact_and_checked`
  - `fixed_point_jump_boundary_and_ceiling_arithmetic_are_exact`
  - `communication_delay_supports_same_tick_and_exact_positive_receipt`
  - `forced_retention_failure_rolls_back_population_id_allocation_and_clock`
  - `invalid_configuration_returns_no_artifact`
- **Failure evidence:** Operation, operands, exact expected result or typed error,
  and complete affected before/after state.

### INV-ID-001 — Stable typed identities and monotonic allocation

- **Status:** active.
- **Exact oracle:** Stable content IDs remain independent of runtime storage.
  Project, ship, population, observer, transmission, and reservation-owner
  identities are typed; system-scoped counters allocate deterministic IDs and
  never reuse a committed sequence after cancellation, departure, arrival, or
  loss. Initial-origin observer identity cannot collide with a ship observer.
- **Applicability:** Accepted authored/generated definitions and every runtime
  allocation/transition implemented in Stage 4b.
- **Non-vacuity witness:** Fixtures allocate multiple projects/ships across two
  Shipyards, cancel a queued item, generate two populations, move both, settle
  one and lose one, and compare the synthetic origin observer with ship zero.
- **Current Tier 1 evidence:**
  - `construction_cancellation_refunds_and_never_reuses_project_ids`
  - `shipyards_have_independent_fifo_queues_pause_cancel_and_never_reuse_ids`
  - `ready_habitat_creates_on_a_following_tick_and_never_reuses_ids`
  - `generated_population_ids_remain_unique_through_departure_arrival_and_loss`
  - `initial_origin_knowledge_uses_one_leg_summaries_and_three_leg_indications`
- **Failure evidence:** Conflicting typed IDs, counter before/after values,
  allocation context, and any duplicate registry/queue/transit key.

### INV-VALIDATE-001 — Strict source-aware Stage 4b inputs

- **Status:** active.
- **Exact oracle:** Authored-world and profile RON reject unknown/missing fields
  and retain logical source provenance in the returned diagnostic. Removed
  topology and writable-population fields do not parse. Invalid generator
  configuration returns no artifact, and accepted generation requests carry the
  normalized configuration and logical provenance used to create them.
- **Applicability:** Stage 4b `compile_str`, profile compilation/loading,
  `CompiledProfile` creation, and `GenerationRequest`.
- **Non-vacuity witness:** Fixtures inject unknown and missing profile fields,
  removed schema fields, malformed/invalid configuration, and complete
  expedition commitment derivation.
- **Current Tier 1 evidence:**
  - `strict_profile_errors_retain_logical_provenance`
  - `strict_schema_rejects_removed_topology_and_population_fields`
  - `invalid_configuration_returns_no_artifact`
  - `tuning_derives_the_complete_expedition_commitment`
  - `generation_request_carries_normalized_configuration_and_artifact_provenance`
- **Failure evidence:** Diagnostic logical source, definition, field, and
  message, or a partially returned artifact.

### INV-WORLD-OWNERSHIP-001 — Sole map, resource, and runtime authorities

- **Status:** active.
- **Exact oracle:** Every location has exactly one immutable system map record and
  one persistent runtime system. Initial body-resource quantities exist only in
  map definitions, remaining quantities only in runtime bodies, and derived
  totals equal the corresponding body sums. Stocks, developments, queues,
  assets, reservations, and accounting belong to systems. No explicit route
  graph, standalone deposit, or writable population total is accepted.
- **Applicability:** Every accepted Stage 4b `WorldDefinition` and its
  `WorldState`.
- **Non-vacuity witness:** Fixtures contain multiple systems and body resources,
  mutate extraction state, execute neutral-system ticks, compare map/runtime
  shape, and reject removed source fields.
- **Current Tier 1 evidence:**
  - `body_resources_keep_distinct_initial_and_remaining_authorities`
  - `same_body_extractors_contend_in_stable_slot_order`
  - `normalized_map_retains_semantic_body_and_slot_order_separately_from_runtime`
  - `global_tick_persists_and_advances_neutral_systems`
  - `strict_schema_rejects_removed_topology_and_population_fields`
- **Failure evidence:** Duplicate/missing authority, map/runtime shape mismatch,
  unequal derived total, or accepted obsolete field.

### INV-POPULATION-001 — Population-token uniqueness and reconciliation

- **Status:** active.
- **Exact oracle:** `PopulationRegistry` is the sole mutable population authority.
  Every token is in exactly one resident or in-transit state; resident occupancy
  is at most one token per Habitat; transit tokens and expedition payloads form
  a bijection; initialized plus generated tokens reconcile exactly against live
  plus removed tokens; authored resident IDs advance birth-system counters past
  every seeded sequence; IDs remain globally unique.
- **Applicability:** Validated authored resident initialization, Habitat
  generation/support, expedition departure/transit, settlement, and loss.
- **Non-vacuity witness:** A strict authored fixture initializes one resident and
  advances its birth-system sequence; runtime fixtures generate multiple tokens,
  derive occupancy and work, remove Habitat support, transfer two tokens to
  simultaneous expeditions, settle one, and lose one.
- **Current Tier 1 evidence:**
  - `authored_resident_population_is_strictly_compiled_and_initialized`
  - `authored_population_diagnostics_cover_duplicates_references_and_transit`
  - `population_registry_is_the_derived_population_and_occupancy_authority`
  - `habitat_support_loss_removes_and_accounts_for_the_token_once`
  - `runtime_requires_a_bijection_between_transit_tokens_and_expeditions`
  - `generated_population_ids_remain_unique_through_departure_arrival_and_loss`
  - `simultaneous_summary_arrivals_succeed_then_lose_in_ship_id_order`
- **Failure evidence:** Population ID, duplicate/missing state, Habitat/ship
  reference, initialized/generated/removed counters, and transition ledger entries.

### INV-KNOWLEDGE-001 — Delayed, monotonic, origin-owned facts

- **Status:** active.
- **Exact oracle:** Each fact key merges independently. Greater detail wins;
  dynamic facts use observation freshness and stable observer tie-breaking;
  immutable contradictions reject the complete transmission atomically;
  duplicate receipt is idempotent. Receipt tick is observation tick plus exact
  direct-distance communication delay, including same-tick zero delay. Physical
  mission outcome does not enter the player-facing mission ledger before its
  final report is received.
- **Applicability:** Initial origin knowledge and all probe/expedition
  observations and transmissions.
- **Non-vacuity witness:** Fixtures exercise all four knowledge levels,
  zero/positive delay, receipt permutations, stale facts, an immutable
  contradiction, duplicate delivery, and a physically arrived expedition with
  an awaiting player outcome.
- **Current Tier 1 evidence:**
  - `initial_origin_knowledge_uses_one_leg_summaries_and_three_leg_indications`
  - `complete_stop_observation_keeps_exact_map_and_dynamic_fields_separate`
  - `communication_delay_supports_same_tick_and_exact_positive_receipt`
  - `dynamic_fact_merge_is_fresh_monotonic_and_receipt_order_independent`
  - `immutable_contradiction_rejects_whole_transmission_and_duplicate_is_idempotent`
  - `player_view_recomputes_active_route_revelation_and_hides_arrived_outcome`
- **Failure evidence:** Transmission identity/timing, fact key and competing
  values/ranks, before/after knowledge, and premature player-visible outcome.

### INV-TICK-001 — Global phase-major deterministic tick

- **Status:** active.
- **Exact oracle:** One world clock drives all ten phases. Every system executes a
  phase in stable system-ID order before any system starts the next phase;
  Shipyards and developments use stable body/slot/FIFO order; movement uses
  stable ship-ID order. Newly completed developments/assets and arrivals first
  operate on the approved following tick. Rejection preserves the complete
  world and clock.
- **Applicability:** Every `WorldState::advance_tick`.
- **Non-vacuity witness:** A two-system scenario performs production, travel,
  settlement, observation, delayed receipt, and retention; simultaneous ships
  compete for settlement; forced failures occur in movement, message receipt,
  and retention after earlier phases have changed candidate state.
- **Current Tier 1 evidence:**
  - `global_tick_persists_and_advances_neutral_systems`
  - `two_system_tick_orders_production_arrival_observation_receipt_and_retention_exactly`
  - `simultaneous_summary_arrivals_succeed_then_lose_in_ship_id_order`
  - `forced_movement_failure_rolls_back_earlier_phases_clock_and_counters`
  - `forced_due_message_failure_rolls_back_movement_clock_and_observer_counter`
  - `forced_retention_failure_rolls_back_population_id_allocation_and_clock`
- **Failure evidence:** Tick/phase/system/ship identity and complete structural
  before/after world state.

### G18-ORIGIN-STRUCTURE — Constructed origin prerequisites

- **Status:** active for `core:frontier_world@1`.
- **Exact oracle:** Revision 1 places the origin at `(0, 0, 0)`, with
  strength/eccentricity `1.0`, `4..=12` bodies, `3..=8` slots per body, every
  mandatory naturally deposit-bearing resource present on at least one origin
  body, exactly one functional Collector in the first body's first slot, no
  other starting development, and the profile's approved starting stocks.
- **Applicability:** A successfully generated revision-1 world.
- **Non-vacuity witness:** The fixture generates a nonempty artifact, identifies
  the origin, checks all ranges and stocks, finds a nonzero mandatory origin
  resource, and proves exactly one development at the exact coordinate.
- **Current Tier 1 evidence:**
  - `generated_world_has_exact_constructive_origin_and_bounded_frontier_facts`
- **Failure evidence:** Complete generation identity and the unequal origin
  position, range, resource, stock, or development fact.
- **Excluded oracles:** Solvency, affordability, seasonal surplus, tick-zero
  action availability, long-run survival, nearby witness, or favorable quantity.

### INV-GENERATION-IDENTITY-001 — Complete generation identity

- **Status:** active for `core:frontier_world@1`.
- **Exact oracle:** Generation identity is generator family/revision, unsigned
  64-bit seed, and SHA-256 of canonical normalized profile bytes. Equal complete
  identity produces equal normalized `WorldDefinition` and `WorldSnapshot`.
  Semantically equivalent profiles have equal canonical bytes/fingerprints;
  changing an output-affecting profile field changes the fingerprint. Logical
  source provenance accompanies but does not alter generation identity. A seed
  alone is not complete identity.
- **Applicability:** `GenerationRequest` and successful revision-1 generation.
- **Non-vacuity witness:** Tests compare equivalent reordered/reduced profiles,
  change target count, generate the same nonempty world twice, and retain
  distinct logical provenance.
- **Current Tier 1 evidence:**
  - `semantic_input_permutations_have_identical_canonical_bytes_and_fingerprint`
  - `one_output_affecting_profile_change_changes_fingerprint`
  - `equal_identity_reproduces_equal_normalized_world`
  - `generation_request_carries_normalized_configuration_and_artifact_provenance`
  - `sha256_hook_is_stable`
  - `primitive_and_container_encoding_vectors_are_fixed`
- **Failure evidence:** Version, seed, fingerprint, canonical bytes, provenance,
  and unequal normalized definition/snapshot.

## Reserved and explicitly unadopted entries

### G18-NEIGHBORHOOD-STRUCTURE — No Stage 4b neighborhood guarantee

- **Status:** not adopted for Stage 4b.
- **Decision:** The procedural frontier has no mandatory nearby witness,
  connectivity, reachability, exact target count, resource floor, favorable
  distribution, solvency, or reclaimable site. Scouting/founding mechanics use
  focused authored fixtures rather than requiring a generated seed to support
  them.
- **Bug boundary:** A frontier outcome is defective only when generation violates
  configuration, identity, references, arithmetic, the active origin guarantee,
  or another named invariant. Qualitative difficulty and inaccessible local
  texture are not failures.

### INV-LOGISTICS-001 — Bounded recovery of accepted physical work

- **Status:** reserved until player-owned automated logistics exists.
- **Undefined here:** Accepted-work state, locked lot, carrier/claim identity,
  timeout/recovery transitions, terminal outcomes, and accounting channels.

### INV-REPLAY-001 — Complete runtime replay identity

- **Status:** reserved for Stage 6.
- **Undefined here:** Complete initial runtime state, event-log requirements,
  runtime compatibility, and replay verification. Generation identity alone is
  not complete runtime replay identity.

## Stage 4b completion evidence

Stage 4b was implemented on branch `feat/stage-4b-constructive-frontier` from
base `d8118fd`; implementation commit `458a522` delivered the phase work as one
implementation commit. The workspace contains 56 focused Tier 1 tests: 28 in
`game-core` and 28 in `game-content`, with no ignored tests found. Exact current
test names under active entries resolve in the retained crates.

The all-feature suite is required because privileged diagnostic snapshots and
four integration-test targets are gated by `test-support`. The production player
boundary remains `PlayerWorldView`; complete snapshots are evidence support, not
a frontend API.

| Current family | Stage 4b classification | Current evidence or disposition |
| --- | --- | --- |
| Fixed-point map/runtime and body resources | Active invariant/Tier 1 | INV-ORDER-001, INV-ARITH-001, and INV-WORLD-OWNERSHIP-001. |
| Physical Energy and expansion payloads | Active invariant/Tier 1 | INV-ENERGY-001 and INV-ATOMIC-001. |
| Stable IDs and population tokens | Active invariant/Tier 1 | INV-ID-001 and INV-POPULATION-001. |
| Global phase-major simulation | Active invariant/Tier 1 | INV-TICK-001 and complete-state rollback evidence. |
| Scouting, reports, and player redaction | Active invariant/Tier 1 | INV-KNOWLEDGE-001 plus delayed mission-outcome fixtures. |
| Constructive origin | Active revision-1 guarantee | G18-ORIGIN-STRUCTURE. |
| Generator identity | Active revision-1 invariant | INV-GENERATION-IDENTITY-001. |
| Frontier count/connectivity/quality | Not adopted | No executable test/CI oracle found; local texture is not acceptance. |
| Automated logistics/runtime replay | Reserved | No current mechanism or executable contract. |
| Stage 3/4 topology/deposits/writable population | Removed obsolete surface | Removed fields and test names are not current evidence. |

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
