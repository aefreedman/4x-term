---
title: "Stage 4b: Constructive Frontier and Bounded Expansion"
type: feature
status: review
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
| Origin/frontier generation and routing | [World Generation](../design/world-generation.md) |
| Generator identity and compatibility | [Generator Identity](../design/generator-identity.md) |
| Exact revision-1 generator algorithm | [Frontier Generator Revision 1](../design/generator-revision-1.md) |
| Editable RON values and `starter` profile | [Tuning Profiles](../design/tuning-profiles.md) |
| Map/runtime ownership and body resources | [Systems and Resources](../design/systems-and-resources.md) |
| Seasonal Energy and spending | [Energy and Seasons](../design/energy-and-seasons.md) |
| Population tokens and Habitats | [Population and Habitats](../design/population-and-habitats.md) |
| Global phases, IDs, timing, and atomicity | [Simulation Timing](../design/simulation-timing.md) |
| Knowledge, observations, and messages | [Scouting and Knowledge](../design/scouting-and-knowledge.md) |
| Shipyards, ships, founding, and control | [Ships and Expansion](../design/ships-and-expansion.md) |
| Retained Stage 4 gameplay contract | [Completed Stage 4 Plan](2026-07-20-feature-constructive-world-generation-stage-4-plan.md) |
| Retained Stage 4 tuning fixture | [`stage4_origin.ron`](../../crates/game-content/tests/fixtures/stage4_origin.ron) |
| Current repository boundary | [Architecture](../architecture.md) |
| Testing policy | [Testing Stance](../2026-07-20-testing-stance-correction.md) |
| Active/reserved invariants | [Invariant Registry](../2026-07-20-engine-invariant-registry.md) |

The wiki pages are the durable gameplay contract. This plan owns implementation
order, migration scope, delegation, tests, and completion tracking.

## Current state

The workspace contains only `game-core` and `game-content`.

Current Stage 4 limitations that Stage 4b replaces:

- `Position3` uses floating-point coordinates.
- Authored `TopologyDefinition` edges are the route authority.
- `ResourceDepositDefinition` is standalone and Extractors reserve deposit IDs.
- bodies and seasonal profiles live inside optional per-system engine records.
- one origin community stores a writable population count.
- ticking is origin-scoped rather than world-global.
- there are no generated-world artifacts, profiles, ships, transit records,
  knowledge facts, pending transmissions, Habitats, or Shipyards.
- `game-content` compiles one strict authored-world RON source and has no
  generator/profile pipeline.

The project has no save format, production startup, CLI, or TUI, so this stage
requires no disk-save migration or compatibility adapter.

## Execution record

- Stage 4 dependency: PR [#14](https://github.com/aefreedman/4x-term/pull/14),
  merged to `main` as `3dfb7793a5c3bdfc6c7a4a2e9f8cdf9efcce6749`.
- Design-plan branch: `stage-4b-design`.
- Implementation branch: _record during Phase 0_.
- Implementation base commit: _record during Phase 0 after the design-plan PR is
  merged_.
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

- [ ] Confirm the Stage 4 PR is merged or choose an explicit stacked-branch base.
- [ ] Create/continue a dedicated Stage 4b implementation branch.
- [ ] Load the Git workflow and quality checklist references.
- [ ] Record the active branch and base commit in this plan.
- [ ] Verify the workspace is clean and run the pre-change workspace tests.
- [ ] Confirm the design wiki links resolve.
- [ ] Mark this phase complete in the plan and commit the planning baseline.

### Phase 1 — Foundation and module extraction

This phase is intentionally single-owner because it changes shared contracts.

- [ ] Extract the internal `game-core` modules listed above without behavior
  changes.
- [ ] Extract the internal `game-content` modules listed above without behavior
  changes and freeze their module declarations/public hook signatures.
- [ ] Add `sha2` to `game-content` and commit the manifest/lockfile update before
  creating Phase 2 worktrees.
- [ ] Preserve all current Stage 4 tests during extraction.
- [ ] Add fixed-point coordinate types and checked squared-distance helpers.
- [ ] Add always-present system/body map definitions and world-level tuning.
- [ ] Add persistent runtime state for every generated/authored system.
- [ ] Add global `SimulationTime` and phase-major tick scaffolding.
- [ ] Add final community identity, population-token tagged states, registry
  storage, counters, validation, and snapshot shape.
- [ ] Add typed project/ship/transmission ID counters and reservation-owner
  foundations.
- [ ] Freeze Habitat/Shipyard development representations and leaf-module phase
  hook interfaces without implementing their behavior.
- [ ] Freeze aggregate `WorldState`, `WorldDefinition`, snapshot, and public
  re-export contracts needed by every Phase 2 slice.
- [ ] Remove explicit topology, standalone deposits, and writable population
  fields in one hard migration.
- [ ] Rewrite retained Stage 4 fixtures against body resources and the new map/
  runtime split.
- [ ] Prove existing Collector/Battery/Extractor/Refinery/construction behavior
  still passes under the replacement schema.
- [ ] Commit the foundation before parallel work begins.

### Phase 2 — Parallel generator, population, and knowledge slices

These slices start from the same Phase 1 commit and own disjoint modules/files.

#### 2A — Profiles, generator, and identity

- [ ] Add strict profile/source schemas with unknown-field rejection.
- [ ] Add all design/profile validation and deterministic diagnostics.
- [ ] Add canonical normalized encoding and SHA-256 fingerprinting.
- [ ] Add `core:frontier_world@1` identity and generated-world artifacts.
- [ ] Add domain-separated SplitMix64 streams and unbiased bounded draws.
- [ ] Implement triangular distributions and resource-body selection.
- [ ] Implement the origin scaffold before optional origin variation.
- [ ] Implement weighted-cell 2D frontier generation with approximate target
  count and no post-generation count repair.
- [ ] Add `content/profiles/starter.ron` containing the approved editable values.
- [ ] Add tiny deterministic generator/profile fixtures.
- [ ] Verify no test treats target count or output quality as an oracle.

#### 2B — Population and Habitat runtime

- [ ] Consume the frozen community/token/ID schema without changing it.
- [ ] Implement Habitat construction behavior, enabled/disabled state, progress,
  ready state, capacity, and derived occupancy.
- [ ] Implement automatic Energy accumulation in stable body/slot order.
- [ ] Implement next-tick population creation and stable population IDs.
- [ ] Implement life support/work derivation from resident tokens.
- [ ] Implement origin/remote commandability rules at population zero.
- [ ] Add explicit population generation/removal accounting.
- [ ] Add focused Habitat/population tests independent of generated worlds.
- [ ] Do not edit aggregate world layout, global simulation orchestration, public
  re-exports, ID schemas, or development-role definitions in this slice.

#### 2C — Routing and origin knowledge

- [ ] Implement fixed-point jump eligibility and ceiling-distance arithmetic.
- [ ] Implement deterministic shortest routes and stable tie-breaking.
- [ ] Allow redacted unidentified intermediate systems.
- [ ] Add `Unknown`, `Anonymous`, `IdentifiedSummary`, and `Complete` knowledge.
- [ ] Generate initial origin knowledge from geometric probe range/depth.
- [ ] Add keyed facts, observations, pending transmissions, and stable IDs.
- [ ] Implement exact communication delay and same-tick zero-delay receipt.
- [ ] Implement fact-level monotonic merge, dynamic-field freshness, tie-breaking,
  immutable-fact contradiction rejection, and idempotent receipt.
- [ ] Add focused route/knowledge tests using authored tiny positions.
- [ ] Do not edit aggregate world layout, global simulation orchestration, public
  re-exports, ID schemas, or development-role definitions in this slice.

### Phase 3 — Integrate Phase 2

- [ ] Merge the generator/profile slice into the orchestrator branch.
- [ ] Run focused and workspace tests; resolve integration issues centrally.
- [ ] Merge the population/Habitat slice.
- [ ] Run focused and workspace tests; resolve integration issues centrally.
- [ ] Merge the routing/knowledge slice.
- [ ] Run focused and workspace tests; resolve integration issues centrally.
- [ ] Confirm no slice changed another slice's owned files without handoff.
- [ ] Wire population and knowledge leaf hooks into `world.rs`, `resources.rs`,
  `simulation.rs`, `ids.rs`, and `lib.rs` under one integration owner.
- [ ] Freeze the integrated APIs needed by ships/founding.
- [ ] Commit the integrated Phase 2 baseline.

### Phase 4 — Shipyards, ships, transit, and founding

This phase depends on the integrated population, routing, knowledge, and profile
contracts and should have one implementation owner.

- [ ] Add Shipyard development state and independent FIFO queues.
- [ ] Add probe/expedition project commitments, progress, pause, cancellation,
  completion, and stable IDs.
- [ ] Add system-owned completed assets and world-owned in-transit ships.
- [ ] Implement atomic launch funding and fixed route commitment.
- [ ] Implement probe adjustable jump limits, stop observations, reveal scans,
  delayed transmissions, and final consumption.
- [ ] Implement expedition population transfer to transit.
- [ ] Implement complete-knowledge two-slot reservations.
- [ ] Implement summary-knowledge unreserved landing selection.
- [ ] Implement successful Habitat/Collector/founding-stock settlement.
- [ ] Implement deterministic insufficient-slot loss and typed loss evidence.
- [ ] Implement post-arrival typed mission outcomes, `AwaitingOutcome` redaction,
  delayed command unlock/loss feedback, and transmission survival.
- [ ] Implement project/payload/transit/arrival/loss reconciliation.
- [ ] Add exact duration-one, multi-leg, simultaneous-arrival, reservation,
  success, failure, and atomic-rejection fixtures.
- [ ] Commit the bounded expansion loop.

### Phase 5 — Global tick and integration scenarios

- [ ] Wire all ten approved phases through one world-level atomic tick.
- [ ] Process systems, developments, Shipyards, and ships in approved stable
  orders.
- [ ] Ensure new developments/assets/arrivals first operate on the approved tick.
- [ ] Add a late-phase forced-failure test proving whole-world rollback.
- [ ] Add a two-system test covering production, travel, arrival, observation,
  delayed receipt, and retention in one exact scenario.
- [ ] Add exact physical-resource reconciliation across project enqueue,
  cancellation, completion, launch, arrival, overflow, and loss.
- [ ] Add population-token uniqueness/reconciliation across generation,
  departure, transit, arrival, and loss.
- [ ] Verify Stage 4 starter bootstrap mechanics remain covered after migration.
- [ ] Commit integrated simulation behavior.

### Phase 6 — Documentation and evidence

- [ ] Update `docs/architecture.md` from planned Stage 4b boundaries to actual
  implemented modules and ownership.
- [ ] Activate/add implemented invariants in the invariant registry with exact
  test evidence.
- [ ] Update README current status without claiming playable startup.
- [ ] Update `CHANGELOG.md` under `Unreleased`.
- [ ] Verify design wiki pages match implemented contracts; change design only
  with explicit designer approval.
- [ ] Record final test counts and exact validation commands.
- [ ] Check every completed item in this plan.
- [ ] Commit documentation/evidence separately from implementation commits.

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

- [ ] Architecture review confirms map/runtime ownership, module boundaries,
  global tick ownership, and `game-content -> game-core` dependency direction.
- [ ] Data-integrity review confirms sole authorities, typed IDs/reservations,
  atomicity, accounting, population uniqueness, and knowledge merge.
- [ ] Spec-flow review confirms Habitat bootstrap, probe flow, reserved and
  unreserved founding, depopulation/repopulation, and failure feedback.
- [ ] Simplicity review identifies no duplicate topology/resource/population
  authority or unnecessary crate/dependency.
- [ ] Every P1/P2 finding is resolved or explicitly returned for designer review.

## Required focused test matrix

### Core schema and retained mechanisms

- [ ] fixed-point coordinate bounds and checked squared-distance overflow;
- [ ] body-resource initial/remaining ownership and derived system totals;
- [ ] multiple same-body Extractors contending in stable slot order;
- [ ] retained Collector/Battery/Refinery/construction exact behavior;
- [ ] whole-world definition/reference validation; and
- [ ] input-order-independent normalized snapshots where order is not state.

### Generator and profiles

- [ ] strict unknown/missing field diagnostics with provenance;
- [ ] profile permutation produces equal normalized fingerprint;
- [ ] one profile-field change changes fingerprint;
- [ ] equal family/revision/seed/fingerprint reproduces equal output;
- [ ] domain-separated streams isolate unrelated generation stages;
- [ ] exact origin scaffold construction;
- [ ] valid worlds above and below target system count;
- [ ] body/slot/strength/eccentricity/resource outputs stay in configured ranges;
- [ ] no partial artifact on invalid configuration/arithmetic/reference; and
- [ ] no generated-world quality or statistical acceptance test.

### Population and Habitats

- [ ] empty Habitat automatic accumulation and body/slot priority;
- [ ] disable/enable with retained progress and no refund;
- [ ] ready-on-one-tick, population-on-following-tick timing;
- [ ] stable globally unique population IDs;
- [ ] resident/transit token exclusivity and derived occupancy/population;
- [ ] origin and remote zero-population commandability rules; and
- [ ] explicit removal/loss accounting.

### Routing and knowledge

- [ ] jump boundary equality and route tie-breaking;
- [ ] adjustable probe jump limit and hidden intermediate stops;
- [ ] exact initial summary/anonymous/unknown states;
- [ ] probe and expedition complete stop observations;
- [ ] reveal radius only for probes;
- [ ] zero/positive communication delay timing;
- [ ] fact merge under every receipt permutation;
- [ ] immutable-fact contradiction atomic rejection; and
- [ ] duplicate transmission receipt idempotence.

### Shipyards and expansion

- [ ] independent Shipyard FIFO queues and stable Energy priority;
- [ ] project pause, completion, unstarted cancellation, and begun rejection;
- [ ] complete package ownership through enqueue/completion/launch;
- [ ] source population transfer and vacated-Habitat behavior;
- [ ] complete-knowledge reservation success and collision rejection;
- [ ] summary-knowledge unreserved success and insufficient-slot loss;
- [ ] simultaneous unreserved arrivals ordered by ship ID;
- [ ] post-outcome inhabited observation and transmission survival;
- [ ] no immediate outcome/ledger leak before delayed mission receipt;
- [ ] successful receipt command unlock and failed receipt typed loss feedback;
- [ ] founding receipt/overflow and next-tick activation; and
- [ ] exact resource/population reconciliation for success and loss.

### Tick atomicity

- [ ] duration-one and multi-leg movement timing;
- [ ] all systems execute each phase before the next phase;
- [ ] stable system/body/slot/Shipyard/ship ordering;
- [ ] late movement/message/retention failure rolls back the complete world; and
- [ ] clock/counters/IDs remain unchanged after rejection.

## Acceptance checklist

- [ ] Designer values are loaded from editable RON profiles, not hard-coded in
  `game-core`.
- [ ] `starter` is an editable baseline, not a canonical world or test oracle.
- [ ] Generated output count may differ from target without failing acceptance.
- [ ] Only the origin scaffold is a constructive generated-world guarantee.
- [ ] Systems are the sole owners of physical stocks and infrastructure.
- [ ] Initial and remaining body-resource quantities have distinct sole owners.
- [ ] Population tokens are the sole population authority.
- [ ] No explicit route graph, standalone deposit, or writable population count
  survives the migration.
- [ ] Every command and tick rejection is atomic over complete affected state.
- [ ] Every physical resource and population transition has explicit accounting.
- [ ] Scouting facts remain delayed, monotonic by field, and origin-owned.
- [ ] Probe and expedition behavior matches the design wiki.
- [ ] Stage 4 resource-engine mechanisms remain covered after schema replacement.
- [ ] No save/UI/startup/reclamation/logistics scope is added.

## Quality gates

Use `C:/Users/aaron/.cargo/bin/cargo.exe` when bare `cargo` is unavailable.

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --all-features`
- [ ] `git diff --check`
- [ ] Search executable tests and CI for `is_solvent`, `is_playable`, exact
  target-count assertions, seed pass-rate thresholds, and generated-world
  quality gates.
- [ ] Confirm no ignored test was added.
- [ ] Confirm the working tree contains no generated build output or machine-
  local configuration.

## Commit and delivery plan

Use incremental conventional commits, for example:

1. `refactor(core): establish stage 4b world model`
2. `feat(content): add frontier generation profiles`
3. `feat(engine): add habitat population runtime`
4. `feat(engine): add geometric scouting knowledge`
5. `feat(engine): add shipyard expansion loop`
6. `test(stage-4b): cover global tick reconciliation`
7. `docs(stage-4b): record implementation evidence`

- [ ] Update this plan's checkboxes in the corresponding incremental commit.
- [ ] Include AI attribution in the final commit and PR body.
- [ ] Push the implementation branch.
- [ ] Create a PR using the project template.
- [ ] Report screenshots as not applicable because the stage is headless and
  non-visual.

## Implementation readiness

- [x] All design links resolve.
- [x] No open gameplay or tuning decision remains in this plan.
- [x] Module/file ownership permits the Phase 2 parallel slices after Phase 1
  freezes aggregate state and hook contracts.
- [x] The foundation phase precedes every dependent worktree.
- [x] Test expectations protect mechanics and invariants rather than one
  generated universe.
- [ ] Human designer approves this implementation plan.
