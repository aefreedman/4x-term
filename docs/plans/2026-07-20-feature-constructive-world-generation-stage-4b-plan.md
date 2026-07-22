---
title: "Stage 4b: Constructive Frontier and Bounded Expansion"
type: feature
status: implemented
date: 2026-07-20
---
# Stage 4b: Constructive Frontier and Bounded Expansion

## Objective

Implement the approved procedural frontier, global multi-system simulation,
Habitat-backed population, scouting knowledge, Shipyards, probes, expedition
travel, and bounded founding loop.

This is a hard replacement of the Stage 4 authored map schema. Preserve approved
resource-engine mechanics in rewritten fixtures, but do not preserve obsolete
explicit topology, standalone deposits, origin-only ticking, or writable
population aggregates.

## Authoritative design context

Implementation agents should load only the pages relevant to their slice:

| Concern | Design source |
| --- | --- |
| Design index | [Game Design Wiki](../design/README.md) |
| Origin/frontier generation and routing | [World Generation](../design/current/world-generation.md) |
| Generator identity and compatibility | [Generator Identity](../design/current/generator-identity.md) |
| Exact revision-1 generator algorithm | [Frontier Generator Revision 1](../design/current/generator-revision-1.md) |
| Editable RON values and `starter` profile | [Tuning Profiles](../design/current/tuning-profiles.md) |
| Map/runtime ownership and body resources | [Systems and Resources](../design/current/systems-and-resources.md) |
| Seasonal Energy and spending | [Energy and Seasons](../design/current/energy-and-seasons.md) |
| Population tokens and Habitats | [Population and Habitats](../design/current/population-and-habitats.md) |
| Global phases, IDs, timing, and atomicity | [Simulation Timing](../design/current/simulation-timing.md) |
| Knowledge, observations, and messages | [Scouting and Knowledge](../design/current/scouting-and-knowledge.md) |
| Shipyards, ships, founding, and control | [Ships and Expansion](../design/current/ships-and-expansion.md) |
| Retained Stage 4 gameplay contract | [Completed Stage 4 Plan](2026-07-20-feature-constructive-world-generation-stage-4-plan.md) |
| Retained Stage 4 tuning fixture | [`stage4_origin.ron`](../../crates/game-content/tests/fixtures/stage4_origin.ron) |
| Current repository boundary | [Architecture](../architecture.md) |
| Testing policy | [Testing Stance](2026-07-20-testing-stance-correction.md) |
| Active/reserved invariants | [Invariant Registry](../2026-07-20-engine-invariant-registry.md) |

The wiki pages are the durable gameplay contract. This plan owns implementation
order, migration scope, delegation, tests, and completion tracking.

## Starting state at base `d8118fd`

The workspace contained only `game-core` and `game-content`. Stage 4 still used
floating-point coordinates, authored topology, standalone deposits, optional
per-system engine records, a writable origin-population count, and origin-scoped
ticking. It had no generated-world artifacts/profiles, ships, transit,
knowledge/transmissions, Habitats, or Shipyards.

The project had no save format, production startup, CLI, or TUI, so this stage
required no disk-save migration or compatibility adapter.

## Execution record

- Stage 4 dependency: PR [#14](https://github.com/aefreedman/4x-term/pull/14),
  merged to `main` as `3dfb7793a5c3bdfc6c7a4a2e9f8cdf9efcce6749`.
- Design-plan branch: `stage-4b-design`.
- Implementation branch: `feat/stage-4b-constructive-frontier`.
- Implementation base commit: `d8118fd`.
- Implementation commit: `458a522` (the implementation phases were delivered
  together rather than as the suggested incremental phase commits).
- Delivery PR: [#15](https://github.com/aefreedman/4x-term/pull/15).
- Pre-change validation: workspace tests were run and had one pre-existing
  failure, `stage4_input_permutations_compile_to_the_same_definition`; the
  failing obsolete permutation assertion was replaced by current Stage 4b
  normalization/fingerprint evidence.
- Git workflow reference: package path `references/cg-work/git-workflow.md`.
- Quality reference: package path `references/cg-work/quality-checklist.md`.
- PR template: package path `references/cg-work/assets/pr-template.md`.

## Scope boundaries

### Included

- hard schema migration to fixed-point map definitions and body resources;
- one global world clock and phase-major atomic tick;
- strict designer-editable profiles under `content/profiles/`;
- deterministic procedural generation and complete generation identity;
- geometric ship routing with ship-specific jump limits;
- origin knowledge, delayed fact transmissions, and fact-level merge;
- Habitat/Shipyard developments and project queues;
- stable population tokens with resident/transit states;
- probes, expedition payloads, travel, founding, and explicit loss;
- rewritten Stage 4 mechanism coverage under the replacement schema; and
- current-state architecture, README, changelog, and invariant updates.

### Excluded

- qualitative generated-world or seed scoring;
- exact target-system-count assertions;
- connectivity, reachability, solvency, or favorable-distribution guarantees;
- save files and runtime event-log replay;
- terminal UI or playable startup;
- reclamation, automated freight, general logistics, delegation, or cultural
  influence; and
- compatibility fields or archives for removed Stage 3/4 map schemas.

## Implementation architecture

### Core module boundary

Before parallel feature work, split the current monolithic core into focused
internal modules without creating a new crate:

```text
crates/game-core/src/
  lib.rs          public re-exports
  ids.rs          stable IDs and counters
  world.rs        map definitions, runtime state, snapshots
  resources.rs    stocks, body resources, developments, construction
  population.rs   communities, population tokens, Habitats
  routing.rs      fixed-point distance and geometric routefinding
  knowledge.rs    facts, observations, transmissions, merge
  ships.rs        Shipyards, projects, assets, transit, founding
  simulation.rs   global clock, phase orchestration, atomic tick
```

Module extraction is an implementation-enabling boundary, not a crate split.
Keep public APIs minimal and use stable IDs rather than exposing ECS entities.

### Content module boundary

```text
crates/game-content/src/
  lib.rs          public loading/compilation API
  schema.rs       strict RON source types
  diagnostics.rs  deterministic source-aware validation
  profile.rs      normalized gameplay/generator profile
  generator.rs    seeded revisioned generation
  fingerprint.rs  canonical encoding and SHA-256
```

Add `sha2` to `game-content` for the approved fingerprint. Do not add a general
random dependency; implement the approved domain-separated SplitMix64 generator
locally. RON, filesystem, provenance, hashing, and generation remain outside
`game-core`.

### Hard schema migration

- Replace floating-point positions with checked fixed-point three-coordinate
  positions; revision-1 generation emits `z = 0`.
- Remove explicit topology and derive routes from committed positions and jump
  limits.
- Move strength, bodies, eccentricity, slots, and initial body-resource
  quantities into always-present map definitions.
- Give every system persistent mutable remaining body-resource quantities.
- Remove standalone deposits, deposit IDs, Extractor-deposit assignments, and
  exclusive deposit reservations.
- Make Extractors target `(body_id, resource_id)` and resolve shared extraction
  in body/slot order.
- Move recipes, capacities, seasonal shape, and action tuning into one validated
  world profile instead of optional copies on neutral systems.
- Replace the single writable population aggregate with stable communities plus
  a world-owned tagged population-token registry.
- Move `SimulationTime` to `WorldState` and execute each phase globally in
  stable system order.
- Add typed stable IDs/counters for projects, ships, populations,
  transmissions, and reservation owners.
- Keep `ReclaimableSiteDefinition` unchanged and behaviorless.
- Delete superseded source fields, constructors, snapshots, tests, and fixtures.

## Progress checklist

### Phase 0 — Execution setup and contract lock

- [x] Confirm the Stage 4 PR is merged or choose an explicit stacked-branch base.
- [x] Create/continue a dedicated Stage 4b implementation branch.
- [x] Load the Git workflow and quality checklist references.
- [x] Record the active branch and base commit in this plan.
- [x] Verify the workspace is clean and run the pre-change workspace tests.
  One pre-existing permutation assertion failed, as recorded above.
- [x] Confirm the design wiki links resolve.
- **Execution variance (closed):** Phase 0 was recorded in the final execution
  evidence; no separate planning-baseline commit was created.

### Phase 1 — Foundation and module extraction

This phase is intentionally single-owner because it changes shared contracts.

- [x] Extract the internal `game-core` modules listed above without behavior
  changes.
- [x] Extract the internal `game-content` modules listed above without behavior
  changes and freeze their module declarations/public hook signatures.
- [x] Add `sha2` to `game-content`.
  **Execution variance (closed):** the manifest/lockfile update was included in
  the combined implementation commit rather than a pre-worktree commit.
- [x] Preserve retained Stage 4 mechanism coverage during extraction. The
  pre-existing obsolete schema-permutation assertion was replaced with Stage 4b
  normalization and fingerprint evidence, as required by the hard migration.
- [x] Add fixed-point coordinate types and checked squared-distance helpers.
- [x] Add always-present system/body map definitions and world-level tuning.
- [x] Add persistent runtime state for every generated/authored system.
- [x] Add global `SimulationTime` and phase-major tick scaffolding.
- [x] Add final community identity, population-token tagged states, registry
  storage, counters, validation, and snapshot shape.
- [x] Add typed project/ship/transmission ID counters and reservation-owner
  foundations.
- [x] Freeze Habitat/Shipyard development representations and leaf-module phase
  hook interfaces without implementing their behavior.
- [x] Freeze aggregate `WorldState`, `WorldDefinition`, snapshot, and public
  re-export contracts needed by every Phase 2 slice.
- [x] Remove explicit topology, standalone deposits, and writable population
  fields in one hard migration.
- [x] Rewrite retained Stage 4 fixtures against body resources and the new map/
  runtime split.
- [x] Prove existing Collector/Battery/Extractor/Refinery/construction behavior
  still passes under the replacement schema.
- **Execution variance (closed):** The foundation shipped in the combined
  implementation commit rather than a separate pre-parallel commit.

### Phase 2 — Parallel generator, population, and knowledge slices

These slices start from the same Phase 1 commit and own disjoint modules/files.

#### 2A — Profiles, generator, and identity

- [x] Add strict profile/source schemas with unknown-field rejection.
- [x] Add all design/profile validation and deterministic diagnostics.
- [x] Add canonical normalized encoding and SHA-256 fingerprinting.
- [x] Add `core:frontier_world@1` identity and generated-world artifacts.
- [x] Add domain-separated SplitMix64 streams and unbiased bounded draws.
- [x] Implement triangular distributions and resource-body selection.
- [x] Implement the origin scaffold before optional origin variation.
- [x] Implement weighted-cell 2D frontier generation with approximate target
  count and no post-generation count repair.
- [x] Add `content/profiles/starter.ron` containing the approved editable values.
- [x] Add tiny deterministic generator/profile fixtures.
- [x] Verify no test treats target count or output quality as an oracle.

#### 2B — Population and Habitat runtime

- [x] Consume the frozen community/token/ID schema without changing it.
- [x] Implement Habitat construction behavior, enabled/disabled state, progress,
  ready state, capacity, and derived occupancy.
- [x] Implement automatic Energy accumulation in stable body/slot order.
- [x] Implement next-tick population creation and stable population IDs.
- [x] Implement life support/work derivation from resident tokens.
- [x] Implement origin/remote commandability rules at population zero.
- [x] Add explicit population generation/removal accounting.
- [x] Add focused Habitat/population tests independent of generated worlds.
- **Execution variance (closed):** Population/Habitat behavior was implemented
  in the coordinated working tree and then wired by the shared integration owner;
  no independently merged slice was used.

#### 2C — Routing and origin knowledge

- [x] Implement fixed-point jump eligibility and ceiling-distance arithmetic.
- [x] Implement deterministic shortest routes and stable tie-breaking.
- [x] Allow redacted unidentified intermediate systems.
- [x] Add `Unknown`, `Anonymous`, `IdentifiedSummary`, and `Complete` knowledge.
- [x] Generate initial origin knowledge from geometric probe range/depth.
- [x] Add keyed facts, observations, pending transmissions, and stable IDs.
- [x] Implement exact communication delay and same-tick zero-delay receipt.
- [x] Implement fact-level monotonic merge, dynamic-field freshness, tie-breaking,
  immutable-fact contradiction rejection, and idempotent receipt.
- [x] Add focused route/knowledge tests using authored tiny positions.
- **Execution variance (closed):** Routing/knowledge behavior was implemented in
  the coordinated working tree and then wired by the shared integration owner;
  no independently merged slice was used.

### Phase 3 — Integrate Phase 2

- **Execution variance (closed):** The three Phase 2 concerns were coordinated
  directly in one working tree rather than merged from separate worktrees.
  Focused tests ran after each delegated concern and the final integrated quality
  gate passed; therefore merge-scoped checks and cross-slice handoff verification
  were not applicable to the execution used.
- [x] Wire population and knowledge leaf hooks into `world.rs`, `resources.rs`,
  `simulation.rs`, `ids.rs`, and `lib.rs` under one integration owner.
- [x] Freeze the integrated APIs needed by ships/founding.
- **Execution variance (closed):** The integrated Phase 2 baseline shipped in
  the combined implementation commit.

### Phase 4 — Shipyards, ships, transit, and founding

This phase depends on the integrated population, routing, knowledge, and profile
contracts and should have one implementation owner.

- [x] Add Shipyard development state and independent FIFO queues.
- [x] Add probe/expedition project commitments, progress, pause, cancellation,
  completion, and stable IDs.
- [x] Add system-owned completed assets and world-owned in-transit ships.
- [x] Implement atomic launch funding and fixed route commitment.
- [x] Implement probe adjustable jump limits, stop observations, reveal scans,
  delayed transmissions, and final consumption.
- [x] Implement expedition population transfer to transit.
- [x] Implement complete-knowledge two-slot reservations.
- [x] Implement summary-knowledge unreserved landing selection.
- [x] Implement successful Habitat/Collector/founding-stock settlement.
- [x] Implement deterministic insufficient-slot loss and typed loss evidence.
- [x] Implement post-arrival typed mission outcomes, `AwaitingOutcome` redaction,
  delayed command unlock/loss feedback, and transmission survival.
- [x] Implement project/payload/transit/arrival/loss reconciliation.
- [x] Add exact duration-one, multi-leg, simultaneous-arrival, reservation,
  success, failure, and atomic-rejection fixtures.
- **Execution variance (closed):** The bounded expansion loop shipped in the
  combined implementation commit.

### Phase 5 — Global tick and integration scenarios

- [x] Wire all ten approved phases through one world-level atomic tick.
- [x] Process systems, developments, Shipyards, and ships in approved stable
  orders.
- [x] Ensure new developments/assets/arrivals first operate on the approved tick.
- [x] Add a late-phase forced-failure test proving whole-world rollback.
- [x] Add a two-system test covering production, travel, arrival, observation,
  delayed receipt, and retention in one exact scenario.
- [x] Add exact physical-resource reconciliation across project enqueue,
  cancellation, completion, launch, arrival, overflow, and loss.
- [x] Add population-token uniqueness/reconciliation across generation,
  departure, transit, arrival, and loss.
- [x] Verify Stage 4 starter bootstrap mechanics remain covered after migration.
- **Execution variance (closed):** Integrated simulation behavior shipped in the
  combined implementation commit.

### Phase 6 — Documentation and evidence

- [x] Update `docs/architecture.md` from planned Stage 4b boundaries to actual
  implemented modules and ownership.
- [x] Activate/add implemented invariants in the invariant registry with exact
  test evidence.
- [x] Update README current status without claiming playable startup.
- [x] Update `CHANGELOG.md` under `Unreleased`.
- [x] Verify design wiki pages match implemented contracts; no design change was
  required.
- [x] Record final test counts and exact validation commands.
- [x] Check every completed item in this plan.
- [x] Commit documentation/evidence separately from implementation commits.

## Multi-agent delegation strategy

### Orchestrator responsibilities

Only the root/orchestrator session delegates work. It owns:

- branch/worktree creation and cleanup;
- this plan and checkbox updates;
- shared contract changes;
- merge order and conflict resolution;
- workspace-wide validation;
- review synthesis; and
- final commits, push, and PR creation.

Delegated agents must not spawn subagents, broaden gameplay rules, edit the plan,
or perform VCS operations unless their task explicitly requests a commit.

### Worktree strategy

Use the project `git-worktree` skill and its manager script; never invoke
`git worktree add` directly. Create all Phase 2 worktrees from the exact Phase 1
foundation commit:

| Slice | Suggested branch | Exclusive implementation ownership |
| --- | --- | --- |
| Generator/profile | `stage-4b-generator` | `game-content` `schema.rs`, `diagnostics.rs`, `profile.rs`, `generator.rs`, `fingerprint.rs`, `content/profiles/`, and generator/content tests |
| Population/Habitat | `stage-4b-population` | `population.rs` leaf behavior and `tests/population_habitats.rs` only |
| Routing/knowledge | `stage-4b-knowledge` | `routing.rs`, `knowledge.rs`, and `tests/routing_knowledge.rs` only |

The foundation and ships/founding phases remain single-owner because they change
shared types or coordinate several runtime authorities. During Phase 2,
`world.rs`, `resources.rs`, `simulation.rs`, `ids.rs`, `lib.rs`, and public
manifests are reserved for the orchestrator's Phase 3 integration; parallel
runtime slices must use frozen hooks rather than edit those files.

### Delegation sequence

1. **Foundation agent:** module extraction and hard shared-schema migration.
2. **Parallel Phase 2 agents:** generator/profile, population/Habitat, and
   routing/knowledge in isolated worktrees.
3. **Orchestrator integration:** merge one slice at a time in the table order,
   validating after each merge, then wire leaf hooks through the reserved shared
   files under one owner.
4. **Ships/founding agent:** implement against the frozen integrated APIs.
5. **Integration agent:** global tick scenarios and reconciliation only; no new
   design.
6. **Read-only reviewers in parallel:** architecture, data integrity, and
   player/spec flow.
7. **Focused resolver agents:** one bounded task per concrete review finding.
8. **Lint specialist:** formatting, Clippy, and static checks with fixes limited
   to reported issues.
9. **Orchestrator:** full quality gate, checkbox/evidence update, commits, push,
   and PR.

At execution time, call `subagent_list` before delegation and prefer the most
specific available specialists. Use `general` only for bounded implementation
slices without a more specific agent.

### Agent task contract

Every implementation delegation must specify:

- exact design pages to read;
- exact files/modules owned;
- APIs it may consume but not change;
- required focused tests and commands;
- stop conditions;
- prohibition on gameplay invention and qualitative seed tests; and
- handoff format: changed files, tests run, commit hash if requested, unresolved
  contract mismatch, and integration notes.

An agent encountering a missing gameplay rule stops and returns the question. It
does not choose a tuning default or broaden scope.

### Review gates

- [x] Architecture review confirms map/runtime ownership, module boundaries,
  global tick ownership, and `game-content -> game-core` dependency direction.
- [x] Data-integrity review confirms sole authorities, typed IDs/reservations,
  atomicity, accounting, population uniqueness, and knowledge merge.
- [x] Spec-flow review confirms Habitat bootstrap, probe flow, reserved and
  unreserved founding, depopulation/repopulation, and failure feedback.
- [x] Simplicity review identifies no duplicate topology/resource/population
  authority or unnecessary crate/dependency.
- [x] Every P1/P2 finding is resolved or explicitly returned for designer review.

## Required focused test matrix

### Core schema and retained mechanisms

- [x] fixed-point coordinate bounds and checked squared-distance overflow;
- [x] body-resource initial/remaining ownership and derived system totals;
- [x] multiple same-body Extractors contending in stable slot order;
- [x] retained Collector/Battery/Refinery/construction exact behavior;
- [x] whole-world definition/reference validation; and
- [x] input-order-independent normalized snapshots where order is not state.

### Generator and profiles

- [x] strict unknown/missing field diagnostics with provenance;
- [x] profile permutation produces equal normalized fingerprint;
- [x] one profile-field change changes fingerprint;
- [x] equal family/revision/seed/fingerprint reproduces equal output;
- [x] domain-separated streams isolate unrelated generation stages;
- [x] exact origin scaffold construction;
- [x] valid worlds above and below target system count;
- [x] body/slot/strength/eccentricity/resource outputs stay in configured ranges;
- [x] no partial artifact on invalid configuration/arithmetic/reference; and
- [x] no generated-world quality or statistical acceptance test.

### Population and Habitats

- [x] empty Habitat automatic accumulation and body/slot priority;
- [x] disable/enable with retained progress and no refund;
- [x] ready-on-one-tick, population-on-following-tick timing;
- [x] stable globally unique population IDs;
- [x] resident/transit token exclusivity and derived occupancy/population;
- [x] origin and remote zero-population commandability rules; and
- [x] explicit removal/loss accounting.

### Routing and knowledge

- [x] jump boundary equality and route tie-breaking;
- [x] adjustable probe jump limit and hidden intermediate stops;
- [x] exact initial summary/anonymous/unknown states;
- [x] probe and expedition complete stop observations;
- [x] reveal radius only for probes;
- [x] zero/positive communication delay timing;
- [x] fact merge under every receipt permutation;
- [x] immutable-fact contradiction atomic rejection; and
- [x] duplicate transmission receipt idempotence.

### Shipyards and expansion

- [x] independent Shipyard FIFO queues and stable Energy priority;
- [x] project pause, completion, unstarted cancellation, and begun rejection;
- [x] complete package ownership through enqueue/completion/launch;
- [x] source population transfer and vacated-Habitat behavior;
- [x] complete-knowledge reservation success and collision rejection;
- [x] summary-knowledge unreserved success and insufficient-slot loss;
- [x] simultaneous unreserved arrivals ordered by ship ID;
- [x] post-outcome inhabited observation and transmission survival;
- [x] no immediate outcome/ledger leak before delayed mission receipt;
- [x] successful receipt command unlock and failed receipt typed loss feedback;
- [x] founding receipt/overflow and next-tick activation; and
- [x] exact resource/population reconciliation for success and loss.

### Tick atomicity

- [x] duration-one and multi-leg movement timing;
- [x] all systems execute each phase before the next phase;
- [x] stable system/body/slot/Shipyard/ship ordering;
- [x] late movement/message/retention failure rolls back the complete world; and
- [x] clock/counters/IDs remain unchanged after rejection.

## Acceptance checklist

- [x] Designer values are loaded from editable RON profiles, not hard-coded in
  `game-core`.
- [x] `starter` is an editable baseline, not a canonical world or test oracle.
- [x] Generated output count may differ from target without failing acceptance.
- [x] Only the origin scaffold is a constructive generated-world guarantee.
- [x] Systems are the sole owners of physical stocks and infrastructure.
- [x] Initial and remaining body-resource quantities have distinct sole owners.
- [x] Population tokens are the sole population authority.
- [x] No explicit route graph, standalone deposit, or writable population count
  survives the migration.
- [x] Every command and tick rejection is atomic over complete affected state.
- [x] Every physical resource and population transition has explicit accounting.
- [x] Scouting facts remain delayed, monotonic by field, and origin-owned.
- [x] Probe and expedition behavior matches the design wiki.
- [x] Stage 4 resource-engine mechanisms remain covered after schema replacement.
- [x] No save/UI/startup/reclamation/logistics scope is added.

## Quality gates

Use `C:/Users/aaron/.cargo/bin/cargo.exe` when bare `cargo` is unavailable.

- [x] `cargo fmt --all -- --check`
- [x] `cargo check --workspace --all-targets --all-features`
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [x] `cargo test --workspace --all-features`
- [x] `git diff --check`
- [x] Search executable tests and CI for `is_solvent`, `is_playable`, exact
  target-count assertions, seed pass-rate thresholds, and generated-world
  quality gates.
- [x] Confirm no ignored test was added.
- [x] Confirm the working tree contains no tracked generated build output or
  machine-local configuration.

### Final validation evidence

The final all-feature workspace suite reported **56 passed, 0 failed, 0
ignored**: 28 tests in `game-core` and 28 in `game-content`. The exact quality
commands were:

```text
C:/Users/aaron/.cargo/bin/cargo.exe fmt --all -- --check
C:/Users/aaron/.cargo/bin/cargo.exe check --workspace --all-targets --all-features
C:/Users/aaron/.cargo/bin/cargo.exe clippy --workspace --all-targets --all-features -- -D warnings
C:/Users/aaron/.cargo/bin/cargo.exe test --workspace --all-features
git diff --check
rg -n -i 'is_solvent|is_playable|target[_ -]?count|pass[_ -]?rate|quality[_ -]?gate|world[_ -]?quality|solvenc|playable' crates .github
rg -n '#\[(ignore|test[^]]*ignore)|#\[ignore' crates .github
```

The two raw searches returned no executable test/CI quality-oracle or ignored-
test matches. Screenshots are not applicable because the stage is headless and
non-visual.

## Commit and delivery plan

Use incremental conventional commits, for example:

1. `refactor(core): establish stage 4b world model`
2. `feat(content): add frontier generation profiles`
3. `feat(engine): add habitat population runtime`
4. `feat(engine): add geometric scouting knowledge`
5. `feat(engine): add shipyard expansion loop`
6. `test(stage-4b): cover global tick reconciliation`
7. `docs(stage-4b): record implementation evidence`

- **Execution variance (closed):** Checklist evidence was consolidated into the
  documentation commits because the implementation phases shipped together.
- [x] Include AI attribution in the final commit and PR body.
- [x] Push the implementation branch.
- [x] Create a PR using the project template.
- [x] Report screenshots as not applicable because the stage is headless and
  non-visual.

## Implementation readiness

- [x] All design links resolve.
- [x] No open gameplay or tuning decision remains in this plan.
- [x] Module/file ownership permits the Phase 2 parallel slices after Phase 1
  freezes aggregate state and hook contracts.
- [x] The foundation phase precedes every dependent worktree.
- [x] Test expectations protect mechanics and invariants rather than one
  generated universe.
- [x] Human designer approves this implementation plan.
