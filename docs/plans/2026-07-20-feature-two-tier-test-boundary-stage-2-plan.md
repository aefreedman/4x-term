---
title: "Stage 2: Establish the Two-Tier Test Boundary"
type: feature
date: 2026-07-20
---
# Stage 2: Establish the Two-Tier Test Boundary

## Executive Summary

Stage 2 will make the repository's executable test boundary match the testing
stance recorded in Stage 1. It will keep focused, exact evidence for durable
simulation contracts; remove authored-world activity, metastability, and
trader-impact quality gates; and delete legacy economy diagnostics after
extracting only cheap focused evidence for durable contracts. New observational
diagnostics should be added only for a concrete generated-world tuning need.

The implementation will add an authoritative Markdown invariant registry,
reuse or extract small deterministic Tier 1 fixtures, and delete obsolete tests
and product gates after identifying any cheap-to-retain durable responsibility.
It will not introduce generated worlds, define G18 values, or design the
replacement RON model. It may remove legacy content, diagnostics, CLI modes,
startup paths, CI acceptance, and tests without keeping the old game playable.
Repository coherence means retained code builds and retained contracts pass—not
that a user can complete a game loop during migration.

This is a **full replacement** of obsolete test and diagnostic premises. No
compatibility aliases, deprecated test gates, or hidden long-soak acceptance
paths should preserve metastability, universal survival, trade churn, NPC fleet
health, or required player-impact divergence.

## Problem Statement

The current suite contains strong focused tests for checked arithmetic,
reconciliation, insertion-order independence, atomic rejection, and exact
Energy-logistics calculations. It also contains repository-scale tests and CLI
validators that treat the authored 20-system market network as a benchmark.
Those two kinds of evidence are still mixed:

- `game-content` freezes authored counts, identities, fleet composition, and
  activity in short and ignored long runs, while production compilation also
  rejects small fixtures through arbitrary cardinality, distance-shape,
  market-role, bootstrap, numeraire, and NPC-shape predicates.
- `game-cli` makes a 10,000-tick descriptive command fail on extinction,
  population shape, trade/production activity, contract churn, and fleet
  liveness.
- CLI reconciliation tests and deterministic-order tests load the repository
  universe even when their useful oracle is independent of that universe.
- the player-impact probe requires a tuned stage or population divergence even
  though its only durable responsibility—external-inflow accounting—already
  has focused core coverage.
- there is no single named registry that tells contributors an invariant's
  exact oracle, applicability rule, non-vacuity witness, or current evidence.

This blocks truthful refactoring: deleting broad tests risks losing real
contracts, while keeping them makes obsolete gameplay an implicit compatibility
target.

## Proposed Solution

1. Create `docs/2026-07-20-engine-invariant-registry.md` as the reviewed source
   of truth for named invariants and their evidence. The registry is
   documentation, not a runtime registration framework.
2. Classify current test families as Tier 1 mechanism evidence, active named
   invariant evidence, descriptive-only tooling, obsolete premise, or deferred
   constructive/generated coverage.
3. Reuse existing focused core tests where they already provide a non-vacuous
   exact oracle. Do not duplicate a repository-scale assertion merely to claim
   extraction.
4. Replace repository-loaded CLI invariant tests with a small local diagnostic
   fixture and pure formatter inputs.
5. Remove repository-world content activity/structure tests, metastability
   validation, repository insertion-permutation acceptance, multi-hop trader
   acceptance, legacy economy diagnostics, and pricing/player-impact probes
   after cheaply retained responsibilities are accounted for. Do not replace
   obsolete coverage one-for-one.
6. Remove arbitrary authored-world validation gates that obstruct small
   fixtures—especially exact cardinality and global market-ecology quality
   rules—while preserving reusable structural validation. Current authored
   content is evidence, not a compatibility target.
7. Remove legacy content-validation and headless-play CI steps. Keep format,
   compilation, lint, and focused retained-contract tests green.
8. Permit normal startup, authored content loading, the TUI, and old app flows to
   stop working or be deleted before Stage 5 restores a truthful executable.

## SpecFlow Analysis

### Developer flow: add or change a deterministic mechanism

1. The developer identifies whether the behavior is a Tier 1 mechanism, an
   active named invariant, or obsolete prototype behavior.
2. For Tier 1, the developer constructs the smallest explicit initial state,
   command/tick sequence, and exact expected state/events.
3. For an invariant, the developer also checks the registry's applicability
   rule and creates a non-vacuity witness that actually enters that condition.
4. A rejected atomic operation compares state, ledgers, events, reservations,
   and relevant ID allocators before and after rejection.
5. The focused test runs in the default workspace suite; no repository-world
   activity threshold is added.

### Developer flow: diagnose a generated or long-run failure

Stage 2 has no generated-world harness. If a long or repository run reveals a
failure, the developer first determines whether it violates a registered
invariant. When possible, the failure class becomes a retained Tier 1 fixture.
Local collapse, unusual population behavior, missing trade, and quiet frontier
activity remain observations and do not become gates.

### Tool flow: legacy commands disappear

1. Remove economy diagnostics, pricing comparison, player-impact,
   metastability, and authored headless/content acceptance instead of adapting
   them to describe a model scheduled for deletion.
2. Preserve exact reconciliation through focused core/CLI-independent tests,
   not through a requirement that a legacy command remain runnable.
3. Operational README instructions disappear with their commands. Git history
   remains the reference for the old prototype.

### CI flow

1. Formatting, compilation, clippy, and retained-contract tests remain.
2. Delete obsolete tests when their gameplay premise disappears; do not make
   them pass through compatibility fixtures or replacement assertions.
3. Remove `--validate-content` and `--headless` legacy acceptance steps. New
   startup/generated-world gates arrive only with their owning Stages 5–6.
4. A non-playable CLI/TUI during Stages 2–4 is an accepted migration state.

### Important variations and edge cases

- A deterministic test is vacuous if both runs do nothing; its fixture must
  produce a state transition, contention, rejection, or resource flow relevant
  to the claimed oracle.
- Automated-logistics anti-strand evidence applies only when a carrier is
  accepted/loaded or a claim/locked lot exists and the configured recovery path
  is exercised. An empty fleet or zero work is not evidence.
- A diagnostic may print `insolvent`, `deficit`, extinction, or inactivity as an
  observation. These values must not affect its exit status unless a separately
  registered engine invariant is violated.
- G18 origin-solvency and neighborhood-affordance entries remain explicitly
  reserved. Stage 2 must not invent margins, ranges, floors, site types,
  generation identity, or seed corpora.
- Content compilation currently couples graph construction and later validation
  to exactly 20 systems. Removing only the error is insufficient: graph
  construction must run for any nonempty finite fixture, or later structural
  checks and derived values would be silently skipped.
- Small fixtures must not be forced to contain exporter/importer/knife-edge
  roles, anti-correlated source placement, nonuniform pairwise distances, an NPC
  archetype, or a bulk-Energy NPC. Those are authored prototype qualities, not
  reusable validation contracts.
- The current session still requires exactly one player trader and one market
  record per system. Stage 2 may use an inert player and one market per fixture
  location; Stage 3/5 own removing those data-model/startup constraints rather
  than hiding them in a fixture framework.

## Technical Approach

### Architecture

The registry should contain one row or subsection per contract with these
required fields:

- canonical ID and name;
- status: active, conditional, or reserved/deferred;
- exact oracle;
- applicability rule;
- non-vacuity witness;
- current Tier 1 evidence by exact test name;
- failure evidence required in assertion output;
- future generated-world applicability, if any;
- owning stage for unresolved fields.

The initial registry should cover only contracts already named by project
guidance and supported by evidence:

- deterministic scheduling and documented insertion-order independence;
- checked physical-resource conservation and exact reconciliation;
- validate-before-mutate atomic rejection;
- checked arithmetic and overflow rejection;
- stable/monotonic domain identifier allocation where dynamic creation applies;
- source-aware deterministic content validation for retained schemas;
- conditional automated-logistics anti-strand/bounded recovery.

G18 constructive guarantees should remain reserved for their owning generation
work. Runtime replay is outside this boundary and has no reserved invariant. A
general liveness or no-deadlock entry must not be marked active unless its exact
finite bound and applicable setup are
derived from an existing mechanism contract; current bounded
claim/lot/recovery tests may be cited under anti-strand instead of inventing an
unbounded global oracle.

Do not add a Rust registry, procedural macro, custom test runner, or CI parser.
The Markdown registry plus resolvable test names is sufficient for this stage
and avoids coupling documentation policy to test infrastructure.

### Test fixture structure

- Keep focused `game-core` vector tests and small `GameDefinition` fixtures that
  already produce exact outcomes.
- Add narrowly named local fixture constructors only where an extracted CLI or
  content test cannot reuse current setup. Avoid a universal builder with many
  defaults that can make applicability accidental.
- For each conditional fixture, include an explicit setup assertion before the
  tested action—for example, a contested intent exists, a contract is active, a
  locked lot is nonzero, or an external inflow was recorded.
- Prefer structural equality of snapshots/events over selected aggregate fields
  for deterministic-order tests when the entire retained outcome is part of the
  contract.
- Keep CLI formatter tests pure where possible: construct a
  `ReconciliationReport` or diagnostic row directly rather than running the
  authored world merely to obtain text.

### Data / Content Impact

No RON files or serialized source types change, but production validation must
stop treating arbitrary authored-world tuning as a content contract.

Within `game-content` and the matching core definition validation:

- remove the exact-20 error and replace the `compiled_systems.len() == 20` graph
  gate with structural preconditions that work for any nonempty finite fixture;
- remove the nonuniform-distance requirement; equal spacing is valid fixture and
  world input unless a later generator contract says otherwise;
- remove `validate_roles_and_anticorrelation` and its exporter/importer/
  knife-edge and source/solar anti-correlation requirements;
- separate required derivation from quality validation: retain computations
  needed by the runnable prototype, such as protected-budget derivation, but
  remove bootstrap-solvency/runway and liquidation adequacy as content
  acceptance gates;
- allow an empty NPC archetype set for a fixed zero-NPC fleet; retain dynamic
  fleet consistency checks only when dynamic fleet behavior is actually
  configured, and remove `validate_archetype_route_capacity` as an authored
  fleet/topology quality gate rather than requiring every fixture archetype to
  cross a nearest-three graph edge;
- preserve exactly one canonical `core:energy` good with the Energy category,
  but stop requiring its `bootstrap_cost` to equal 1 in content and
  `GameSession::new`; that value is an obsolete numeraire convention, not a
  physical-resource invariant;
- create 3–6-location source fixtures directly and do not add temporary
  20-record scaffolding or duplicate the authored repository in test code;
- remove assertions about repository counts, named frontier systems, NPC fleet
  composition, authored balance, trade activity, and population/world health;
- retain exact source/ID/field diagnostics and aggregation of independent
  validation failures;
- reduce `non_default_source_scaling_matches_runtime_role_and_reserve_math` to
  focused fixed-point/config arithmetic if needed, relying on existing core
  runtime tests for full burn/reserve behavior;
- remove `protected_budget_uses_the_runtime_liquidation_contract_adversarially`
  unless a separately accepted non-commercial responsibility is identified;
  it currently protects obsolete liquidation/trader economics.

Repository content does not need a replacement integration gate in this stage.
If it no longer validates after obsolete predicates or supporting features are
removed, delete or defer it rather than repairing it for interim playability.

### Runtime / Platform Impact

Stage 2 may leave `game-cli`, `game-app`, or `game-tui` without a playable flow.
Retained crates should compile where practical, but obsolete modes and surfaces
should be deleted rather than adapted.

`game-cli` cleanup includes:

- remove player-impact, pricing-comparison, economy-diagnostic, metastability,
  and authored headless/content-validation modes when they exist only for the
  legacy product;
- remove `SoakSummary`, `SoakTracker`, compact repository soaks, diagnostic
  formatters, processor-solvency/texture reporting, and associated tests unless
  a focused retained invariant directly consumes a helper;
- move or retain exact reconciliation only in the smallest non-legacy boundary
  that owns physical-resource accounting; a legacy CLI output format is not a
  contract;
- allow the default CLI/TUI startup path to be removed or return an explicit
  migration-unavailable result rather than constructing compatibility content.

No save compatibility is promised. No dependency, performance, or platform
expansion is expected; frontend rendering may shrink substantially as obsolete
screens are deleted in Stage 3.

## Implementation Phases

### Phase 1: Establish the registry and classification boundary

- [x] Create `docs/2026-07-20-engine-invariant-registry.md` with the required
      fields, canonical initial entries, active/conditional/reserved statuses,
      and exact existing test evidence.
- [x] Add a migration matrix for every current test/diagnostic family identified
      by the Stage 1 audit: keep as Tier 1, register as invariant evidence,
      descriptive-only, remove as obsolete, or defer to a named later stage.
- [x] Record that registry updates require a reviewed oracle, applicability rule,
      and non-vacuity witness; a test name containing “invariant” is not enough.
- [x] Link the registry from `docs/architecture.md` and the Stage 2 section of
      `docs/plans/2026-07-20-testing-stance-correction.md`.
- [x] Reserve, but do not specify, G18 and generated-world entries for Stages 4
      and 6.

Validation:
- [x] Manually trace every active registry entry to at least one existing or
      planned exact test and verify conditional entries have a non-vacuity
      witness.
- [x] Confirm no active entry uses survival, activity, profitability, population
      shape, or one authored universe as its oracle.

### Phase 2: Move durable evidence to focused Tier 1 coverage

- [x] In `crates/game-core/src/tests.rs` and
      `crates/game-core/src/energy_logistics/tests.rs`, inventory and retain the
      existing focused tests for reconciliation, deterministic ordering,
      atomicity, checked arithmetic, identifier allocation, D10 attribution,
      and bounded recovery; add tests only for a registry field with no current
      exact evidence.
- [x] Add explicit setup assertions to conditional invariant tests where a test
      could otherwise pass without contention, an active contract, a locked
      resource, or an actual flow.
- [x] In `crates/game-content/src/lib.rs`, remove the exact-20 error and
      cardinality-gated graph construction, then add direct 3–6-location source
      fixtures for reusable schema/provenance tests.
- [x] Remove authored-world quality predicates that block those fixtures:
      nonuniform distances, exporter/importer/knife-edge role composition,
      source/solar anti-correlation, bootstrap solvency/runway, and commercial
      liquidation adequacy. Preserve generic arithmetic helpers only where a
      retained exact test consumes them.
- [x] Permit a fixed zero-NPC content definition without archetypes or a bulk
      hauler; keep dynamic fleet checks conditional on dynamic fleet
      applicability, and remove `validate_archetype_route_capacity` without
      redesigning topology.
- [x] Decouple the canonical Energy identity/category rule from the obsolete
      `bootstrap_cost == 1` numeraire rule in both content compilation and core
      session validation.
- [x] Remove `repository_content_loads_with_structural_roles`,
      `repository_economy_short_smoke_is_deterministic_and_active`, and the
      ignored 1,000-tick repository acceptance after mapping their durable
      assertions to the registry's focused evidence.
- [x] Add a small content fixture whose optional archetype cannot cross its
      shortest adjacent edge and prove compilation is not rejected solely for
      fleet-route ecology; route Energy arithmetic remains covered separately.
- [x] Remove or narrow repository-backed balance tests whose premises are
      commercial liquidation, universal market activity, or mutable authored
      tuning; preserve only exact reusable arithmetic/source-validation
      coverage.
- [x] Remove repository-loaded CLI reconciliation tests and rely on the focused
      core external-inflow/reconciliation fixtures; do not retain a diagnostic
      formatter solely to host the contract.
- [x] Remove the repository-bound insertion-permutation test because focused
      non-vacuous core insertion-order tests already own that contract.
- [x] Remove `player_completes_a_multi_hop_headless_trade`. Keep
      `public_crate_boundaries_compose` only if it still exercises retained APIs
      without compatibility work; otherwise delete it and let Stage 5 add the
      distinct origin-first boundary.

Validation:
- [x] Run newly focused tests by exact name during development, then rely on the
      final workspace suite rather than duplicating every per-crate run.
- [x] Deliberately break and restore only new or materially changed conditional
      tests whose non-vacuity is uncertain; retain representative failure
      evidence rather than mutation-checking every oracle.
- [x] Verify no default or ignored test name/content asserts repository trade,
      production churn, fleet activity, universal survival, population ratchets,
      or exact authored counts as a quality outcome.

### Phase 3: Delete legacy tooling and acceptance, then finish documentation

- [x] Remove player-impact, pricing-comparison, economy-diagnostic,
      metastability, legacy content-validation, and authored headless execution
      modes with their parsers, reports, trackers, formatters, and tests.
- [x] Retain exact external-inflow/reconciliation evidence only in focused
      non-legacy tests; do not preserve a CLI mode or output format to host it.
- [x] Remove the content-validation and headless-acceptance CI steps. Keep
      format, check, clippy, and tests for retained contracts.
- [x] Delete obsolete operational README sections rather than rewriting them as
      interim legacy instructions. State plainly that the game may be
      non-playable until Stage 5.
- [x] Move any still-current durable contract stated only in completed prototype
      todos/plans (for example checked physical Energy accounting) into the
      invariant registry or current direction docs, then delete those completed
      artifacts rather than retaining them as working-tree history.
- [x] Remove `archive/market-trading-prototype/`, `archive/README.md`, and
      obsolete/completed prototype todos after repairing current links; retain
      the Stage 1 plan as part of the active staged-migration record and do not
      copy newly removed implementation history anywhere in the working tree.
- [x] Update `CHANGELOG.md` under `Unreleased` for the removed legacy diagnostic
      mode and corrected test boundary.
- [x] Mark Stage 2 complete in
      `docs/plans/2026-07-20-testing-stance-correction.md` and add completion evidence
      without implying that Stages 3–8 are complete.
- [x] Update the Stage 1 audit's Stage 2 backlog/evidence links if needed; retain
      its original migration decisions.

Validation:
- [x] Search executable Rust, current README guidance, CI, and the working tree
      for active definitions, invocations, or copies of removed diagnostics,
      legacy acceptance, authored-world tests, and archive material. Historical
      references remain available through Git, not current files.
- [x] Confirm CI contains only gates justified by retained code/contracts.
- [x] Confirm failure of normal startup, authored content loading, headless play,
      or legacy TUI flows is not treated as a Stage 2 regression.

## Acceptance Criteria

### Functional Requirements

- [x] The invariant registry records an exact oracle, applicability rule,
      non-vacuity witness, failure evidence, and resolvable test names for every
      active invariant.
- [x] No active registry entry or automated test treats local collapse,
      universal survival, population shape, trade/production churn, fleet
      profitability/activity, or required player-impact divergence as success.
- [x] Existing focused coverage proves deterministic ordering, exact physical
      Energy reconciliation, checked arithmetic, atomic rejection, stable IDs,
      source-aware validation, and applicable bounded logistics recovery.
- [x] Removed repository-scale tests contribute no hidden default or ignored
      acceptance gate.
- [x] Legacy economy diagnostics, player-impact, pricing comparison,
      metastability, content-validation acceptance, and authored headless play
      are removed without aliases or compatibility layers.
- [x] CI does not require current repository content, normal startup, headless
      play, or legacy TUI flows to work.
- [x] The retained workspace is buildable around retained contracts; no
      playable interim executable is required.
- [x] G18 values, generated-world seeds, geography/community schema, and startup
      cutover remain deferred to their owning stages; runtime replay is outside
      the migration boundary.

### Quality Requirements

- [x] `cargo fmt --all -- --check` passes.
- [x] `cargo check --workspace --all-targets --all-features` passes.
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
      passes.
- [x] `cargo test --workspace --all-features` passes for the retained workspace
      with no ignored legacy repository acceptance remaining.
- [x] Test names and failure messages identify the contract and report expected
      versus actual values or the rejected mutation surface.
- [x] No gameplay or visual acceptance is required during the non-playable
      migration interval, even if obsolete TUI surfaces are deleted.
- [x] No save, authored-content, command, or UI compatibility is promised.

## Validation Plan

### Automated Validation

- [x] `cargo fmt --all -- --check`
- [x] `cargo check --workspace --all-targets --all-features`
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [x] `cargo test --workspace --all-features`
- [x] Run exact affected tests only as targeted diagnosis when developing or
      investigating a workspace failure; do not duplicate all crate suites in
      final evidence.
- [x] `git diff --check`
- [x] Use `grep` or available repository search tooling to verify that
      executable Rust, README command guidance, and CI contain no active
      `validate_metastability`, `metastability_acceptance`, removed player-impact
      or pricing-comparison modes, repository activity smoke, or ignored
      1,000-tick acceptance
      invocation. Current governance plans/audits may name removed behavior to
      record the decision; copied implementation/content and legacy archive
      material are not allowed.

### Manual Validation

- [x] Review each registry row against its cited test and confirm the fixture
      enters the applicability condition.
- [x] Confirm exact reconciliation tests still report expected, actual,
      difference, and named transfer channels without depending on a legacy CLI
      command.
- [x] Confirm the default and ignored test lists contain no authored-world
      quality benchmark.
- [x] Confirm no task or review treats broken/removed startup, authored content,
      headless play, or TUI behavior as a Stage 2 regression.

### Evidence to Capture

- Final workspace test summary, including ignored-test count, plus targeted
  failure evidence only for new or materially changed conditional tests.
- Focused reconciliation test output showing exact expected/actual accounting.
- Registry-to-test review notes for active and conditional entries.
- Search output proving obsolete gates survive only as current governance
  decision text, not executable code, content, CI, UI, or copied archive files.
- `git diff --name-only` showing the intentionally deleted and retained surfaces.

## Dependencies and Risks

### Technical Dependencies

- Stage 1 governance, architecture guidance, and migration audit must remain the
  authority for classification.
- Existing focused `game-core` tests provide cheap retained evidence for core
  contracts; obsolete tests need no replacement.
- Stage 2 removes arbitrary authored-world validation predicates so small
  source fixtures can compile; Stage 3 still owns the one-market-per-system
  data-model split and locations without living communities.
- No playable startup or legacy acceptance dependency remains. Stage 5 creates
  new startup acceptance from the origin-and-frontier contract.

### Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Broad tests are deleted before durable assertions are identified. | Regressions in conservation, ordering, or atomicity lose coverage. | Require registry mapping and exact focused evidence before deletion; reuse existing core tests rather than trusting test counts. |
| A renamed long soak remains a quality gate. | Obsolete authored-world behavior continues to constrain implementation. | Search assertions and command exit paths, not only test names; remove ignored acceptance too. |
| Deleting legacy diagnostics accidentally deletes reconciliation evidence. | Physical Energy corruption becomes harder to detect. | Keep reconciliation only as a registered focused invariant test; do not retain the diagnostic host. |
| Removing the exact-20 error alone leaves graph construction disabled for small fixtures. | Tests could pass through a different validation path or skip derived checks. | Generalize the graph precondition in the same change and prove 3–6-location valid/error fixtures traverse it. |
| Legacy role, distance, bootstrap, numeraire, NPC-shape, or archetype route-capacity predicates are mistaken for invariants. | Arbitrary prototype tuning continues to constrain fixtures and later worldgen. | Remove the named predicates explicitly; keep route-cost arithmetic and structural/reference validation in focused tests. |
| Conditional liveness tests pass with no work. | Registry claims coverage without exercising the invariant. | Require explicit setup assertions and a concrete claim/lot/carrier transition before the oracle. |
| Stage 2 drifts into worldgen or replacement startup design. | Scope expands and later contracts are guessed prematurely. | Keep G18 entries reserved, leave runtime replay out of scope, delete legacy gates without replacing them, and hand replacement behavior to Stages 3–5. |
| Reviewers interpret a non-playable build as a regression. | Time is spent rebuilding deleted compatibility surfaces. | Put the buildable-but-non-playable policy in AGENTS, architecture, README, direction, and acceptance criteria. |
| Deleted material is copied into `archive/` “just in case.” | Working-tree bloat and stale guidance survive the migration. | Use Git history exclusively and search for copied legacy trees before completion. |

## Documentation and Follow-up

### Documentation to Update

- [x] Add `docs/2026-07-20-engine-invariant-registry.md`.
- [x] Update `docs/architecture.md` with the registry link and contribution rule.
- [x] Update `docs/plans/2026-07-20-testing-stance-correction.md` with Stage 2 status and
      evidence.
- [x] Update `docs/plans/2026-07-20-authored-market-world-migration-audit.md` and the
      Stage 2/3 wording in `docs/plans/2026-07-20-testing-stance-correction.md` to record
      the planning correction: arbitrary authored-world validation predicates
      are removed in Stage 2 when they obstruct truthful micro-fixtures; Stage 3
      still owns the geography/community schema split.
- [x] Replace legacy operational README guidance with migration status; do not
      document commands that are intentionally removed or broken.
- [x] Update `CHANGELOG.md` under `Unreleased`.

### Intentional Follow-up

- **Stage 3:** finish all planned demolition, including dynamic-fleet rules,
      trader identity, market/trader helpers, and obsolete app/TUI surfaces;
      separate geography from living actors, remove the one-market-per-system
      requirement, and re-derive population, brownout, and investment validation
      so current tuning ratios or “all four shapes” rules do not become
      accidental destination contracts.
- **Stage 4:** implement the authored origin resource/infrastructure engine in
      exact Tier 1 fixtures.
- **Stage 4b:** define structural G18 placement contracts, parameter provenance,
      generator version, and constructive per-output tests; explicitly decide
      connectivity/topology rather than inheriting the current nearest-three,
      universally connected graph rule.
- **Stage 5:** introduce origin-first composition, headless execution, and
      startup acceptance from the new model without a trader compatibility path.
- **Stage 7:** run final code/content/docs/CI searches and prove that no
      unjustified legacy surface or compatibility copy remains. Discovery of
      one fails the Stage 2–3 prerequisite; it is not scheduled demolition.

## References & Research

Reference roots:

- Runtime/test paths are relative to `crates/` unless noted.
- Governance artifacts are relative to `docs/`.

### Evidence Index

- **E1 — controlling direction:**
  `plans/2026-07-20-testing-stance-correction.md:62-105,163-177,225-235`
- **E2 — migration decisions/backlog:**
  `plans/2026-07-20-authored-market-world-migration-audit.md:112-166`
- **E3 — architecture test boundary:** `architecture.md:306-347`
- **E4 — focused core fixtures and invariants:**
  `game-core/src/tests.rs:5-182,1704-1970,3633,4555,5411`;
  `game-core/src/energy_logistics/tests.rs:3-108,135-318`
- **E5 — authored-world compiler gates and repository-coupled tests:**
  `game-content/src/lib.rs:509-558,680-814,881-1008,1420-1730,1930-2117,2460-2714,2762-2870`;
  `game-core/src/lib.rs:2760-2780`
- **E6 — mixed CLI validation/diagnostics:**
  `game-cli/src/main.rs:35-72,244-504,570-630,1122-1280,1341-1355,1523-1841,2256-2570`
- **E7 — integration/CI boundaries:**
  `game-cli/tests/boundaries.rs:1-89`; `.github/workflows/ci.yml:15-26`
- **E8 — current user-facing diagnostic guidance:** `../README.md:58-97`

### Internal References

- `AGENTS.md:16-27` — contributor rules for generated failures, Tier 1 behavior,
  constructive generation, and compatibility.
- `docs/2026-07-20-design-direction-governance-sandbox.md:214-226` — product-level
  two-tier testing direction.
- `.github/workflows/ci.yml:15-26` — commands that Stage 2 must preserve.

### External References

No external research or authoritative API cross-check was needed. Stage 2 uses
project-local Rust tests and documented project contracts and introduces no
engine, framework, package, platform, or third-party API behavior.

### Institutional Knowledge

- `docs/solutions/rust-ecs-validate-before-mutate.md` — every atomic operation
  must calculate and validate all results before mutation, apply validated
  values together, and emit events only after success; checked arithmetic alone
  is not sufficient evidence of atomicity.
