---
title: "Direction Shift: Testing Stance and Constructive Worldgen"
type: plan
status: completed
date: 2026-07-20
completed: 2026-07-21
---
# Direction Shift: Testing Stance and Constructive Worldgen

## Status and use

This is the completed high-level transition plan that corrected how the
simulation is tested and guided the move from the former authored
market-trading prototype to the strategy and map-expansion game defined by the
[Governance Sandbox design direction](../2026-07-20-design-direction-governance-sandbox.md),
especially G2 and G16–G22.

The stages below must be carved into focused plans or todos before
implementation. Those plans should investigate the repository as it exists
at that time and supply their own exact contracts, migration inventories,
and acceptance commands rather than treating this document as an
executable specification.

**Current migration boundary (2026-07-21):** The testing-boundary, destructive
cutover, origin simulation, constructive frontier, and playable-startup bundles
are complete. The workspace now includes the headless `game-core` simulation,
the `game-content` schema/profile/generation adapter, the application-owned
session boundary, the terminal UI, and the human-play `4x-term` executable.
The all-feature workspace suite has 110 focused tests and no ignored tests.
Persistence and an agent-facing command protocol remain possible future product
work. Broad invariant sweeps, runtime replay, and texture diagnostics have no
scheduled milestone and should be proposed only for a concrete need; none is a
prerequisite for truthful human play.

## The problem being corrected

The pre-cutover test universe (the authored ~20-system world) served as both
*fixture* and *benchmark*. This invites a failure mode: treating every
local wobble in that specific universe as a defect, and tuning simulation
constants until that one map behaves placidly.

Under the project's design direction, **local collapse is permitted and
expected** — a struggling or dead region is world texture (and future
reclamation content), not a bug. The world must be *possible*, not
provably stable. Solving for one authored map optimizes for exactly the
wrong target.

The deeper correction: world viability is not a statistical property to
verify after generation. It is **constructed where it matters and
irrelevant everywhere else** (G18). Revision 1 constructs the origin command
seat at population zero with the approved physical scaffold; population is
bootstrapped through a Habitat. The generated frontier starts neutral, with no
NPC market network whose health needs guaranteeing.

## Relationship to the pre-cutover implementation

The pre-cutover game and codebase were inputs to a migration, not constraints
on the destination. The authored 20-system market network, independent NPC
traders, trader-first player flow, market-per-system model, diagnostics,
and acceptance gates may be incompatible with the governance sandbox. A
stage may preserve a useful mechanism, reshape it for player-owned
logistics, replace its data model, or remove it entirely.

Compatibility with obsolete gameplay is not a goal. The durable gameplay
foundation is the physical Energy pressure and a resource economy that
forces tension between sustaining current communities and funding
expansion. Markets, pricing, independent traders, NPC fleet behavior,
authored routes, and their UI or diagnostics have no presumption of
survival.

Preserve lower-level contracts such as deterministic scheduling, checked
resource accounting, validate-before-mutate, stable identifiers, headless
simulation, and frontend boundaries only where they continue to serve the
new game. Remove conflicting features outright when that is simpler or
truer than adapting them; this project is prototyping, not maintaining
backward product compatibility.

## The corrected stance

### Tier 1 — Authored micro-fixtures (test mechanisms)

- Small hand-written worlds: roughly 1–6 systems with only the bodies, slots,
  developments, populations, and ships needed by the scenario.
- Deterministic, with hand-computable expected outcomes.
- Test *mechanisms*, one or few at a time: fixed-point distance boundaries,
  seasonal apportionment, body-resource contention, Habitat ready/finalize
  timing, fact merge and delay, Shipyard queue progression, expedition
  settlement/loss, and complete-state rollback.
- Exact assertions are correct here. Agents may and should "solve for the
  fixture" — that is what fixtures are for.
- Fast: runnable in seconds, suitable for inner-loop iteration and smoke
  testing.

### Tier 2 — Generated worlds (test invariants and guarantees only)

Generated-world tests assert exactly two kinds of things, both
deterministic and exact — **no statistical acceptance criteria, no
N-of-M seed thresholds, no viability screening or reject/reroll**:

1. **Engine invariants**, on a handful of arbitrary seeds and only where their
   applicability setup is present: deterministic identity/output, checked
   arithmetic, sole ownership, population reconciliation, atomic mutation, and
   other active entries in the invariant registry. An applicable invariant
   violation on any seed is always a bug. Do not invent soak criteria for
   reserved systems such as automated logistics or replay.
2. **The approved structural guarantee of G18**, implemented as exact per-seed
   assertions on revision-1 generator output. It asserts the
   mandatory origin records, references, placement, resources, starting stocks,
   and development scaffold. It does not assert economic solvency, seasonal
   surplus, affordability or quantity floors, tick-zero action availability,
   long-run survival, nearby witnesses, favorable distributions, or a
   reclaimable site.

Everything else observed in a generated world — regions dying, odd resource
quantities or distributions, economic weirdness at the frontier — is
presumptively texture. It may inform worldgen tuning; it is never a test
failure.

### Descriptive diagnostics (not tests)

Distribution shape, frontier characteristics, and emergent texture are
reported by diagnostics tooling for human review when tuning worldgen
feel. These reports have no pass/fail semantics and must not be wired
into CI as gates.

## Norms to record in AGENTS.md (verbatim or adapted)

1. **Individual seed outcomes are not bugs unless they violate a named
   engine invariant or a G18 constructive guarantee.** Do not tune
   constants to fix one seed's local behavior.
2. **Local collapse is expected.** Regions struggling or dead in a
   generated world are design-intended texture and future reclamation
   content. Only invariant violations and guarantee failures are bugs.
3. **Never write acceptance criteria against a specific authored
   universe** except in Tier 1 micro-fixtures, which must be small enough
   that expected outcomes are hand-computable.
4. **A feature that can only be validated by a soak run is a simulation
   feature.** Gameplay-facing features must be verifiable in Tier 1
   scenario fixtures (small world, tens of ticks, deterministic expected
   outcome).
5. **The approved scaffold is constructed, not screened.** The generator builds
   the approved origin scaffold directly. The current generator revision has no
   neighborhood viability guarantee. Do not add post-hoc gameplay filters or
   statistical world-quality gates.
6. **Generator parameter ranges are design decisions, not fixes.** Do not
   adjust generation ranges to make a failing check pass without flagging
   the change for design review. Generator changes invalidate seed
   reproducibility and are loud, reviewed events — version the generator
   if necessary.
7. **When a generated-world failure occurs**, reproduce the failure class
   as a Tier 1 fixture where possible before fixing, and keep the
   fixture — the fixture suite grows from real failure classes.

## Transition stages

These are dependency-ordered direction stages, not implementation phases
that must share one branch or plan. Each stage should leave retained code
buildable, retained contracts tested, and current documentation truthful. The
legacy game need not remain playable between stages. Obsolete implementation,
content, tests, diagnostics, and UI should be deleted rather than bridged to a
replacement or copied into a working-tree archive; Git history preserves them.

### Stage 1 — Record the direction and audit the migration surface

**Status:** recorded on 2026-07-20 in the
[authored market-world migration audit](2026-07-20-authored-market-world-migration-audit.md).
This records migration decisions only. The implementation bundles through
playable startup are complete below; deeper gameplay remains future work.
Replay and diagnostic tooling have no scheduled transition owner.

- Add the testing stance and norms to `AGENTS.md` and architecture notes.
- Mark obsolete product assumptions clearly, including trader-first play,
  independent NPC market ecology, and metastability as a quality bar.
- Inventory current content, runtime startup, tests, diagnostics, CI gates,
  and data-model assumptions that privilege the authored 20-system world.
- Classify each affected mechanism as **keep**, **reshape**, **replace**, or
  **remove**. Classification is a design decision, not a compatibility
  exercise; obsolete behavior needs no replacement unless the new game
  needs the responsibility it served.

This stage changes governance and documents the migration; it should avoid
prematurely redesigning the world model.

### Stage 2 — Establish the two-tier test boundary and remove obsolete gates

**Status:** completed on 2026-07-20. The implementation established the
registry and direct three-location source fixtures, removed authored-world
quality predicates and repository acceptance, deleted legacy diagnostic and
acceptance CLI modes, reduced CI to retained workspace gates, and removed
working-tree prototype archives and completed prototype todos. At the Stage 2
boundary, the workspace passed formatting, check, Clippy with warnings denied,
and all 201 then-retained tests with no ignored tests; playability was
intentionally not acceptance.

The reviewed [Engine Invariant Registry](../2026-07-20-engine-invariant-registry.md)
records each active contract's exact oracle, applicability rule, non-vacuity
witness, failure evidence, and focused tests.

- Extract hand-computable mechanism coverage into Tier 1 micro-fixtures only
  for responsibilities that are cheaper to retain than re-engineer.
- Define a named invariant registry with an exact oracle and applicability
  rule for each invariant. Do not count vacuous checks as coverage.
- Delete legacy economy diagnostics, pricing/player-impact probes,
  metastability validation, authored-world acceptance, and obsolete tests;
  do not preserve descriptive tooling for a world model being removed.
- Remove arbitrary authored-world content predicates that obstruct small
  fixtures, including exact cardinality and global ecology-quality rules.
- Remove content/headless CI acceptance when it tests the legacy product.
  Keep formatting, compilation, linting, and focused retained-contract tests.
- Do not require current content, normal startup, headless play, the TUI, or
  the full legacy workspace test inventory to remain operational.

This stage protects only retained foundations. It deliberately permits a
buildable but non-playable repository while the destination model is absent.

### Stage 3 — Remove the obsolete product surface and introduce the substrate

**Status:** completed on 2026-07-20. Stage 3 replaced the trader/market model
with a deterministic headless substrate for stable resources and locations, one
living origin community, deposits, reclaimable sites, and explicit topology.
The retained `game-core` and `game-content` workspace has nine core tests and
six content tests (15 total), with no ignored tests. Production authored
content and the app, TUI, and CLI crates were removed rather than preserved as
compatibility shells. Current contract evidence is recorded in the
[Engine Invariant Registry](../2026-07-20-engine-invariant-registry.md).

- Delete trader-first identity, independent NPC fleet ecology, pricing,
  wallets, commercial market behavior, authored market startup/content, and
  obsolete application/TUI surfaces as their retained low-level mechanisms
  are isolated. Do not add adapters or placeholders that mimic old gameplay.
- Introduce a world model capable of representing locations without live
  markets or populations.
- Represent one living origin separately from empty geography, extractable
  resources, and minimally typed reclaimable sites.
- Decouple graph/topology construction from market instantiation.
- Split reusable content validation from validation rules specific to the
  legacy authored market network.

The exact bodies, slots, ruin internals, surveys, and information model are
out of scope. This stage creates only the minimum truthful substrate needed
by G17 and G18.

### Stage 4 — Implement the authored origin resource/infrastructure engine

**Status:** completed on 2026-07-20 with system-owned state, strict Stage 4
content, exact mechanism coverage, and the approved 20-tick zero-population
bootstrap.

- Add deterministic ticks, seasonal Energy, retention/overflow, life support,
  and exact shortage evidence in the headless core.
- Implement G13 systems → bodies → generic slots → developments.
- Make systems persistently own stocks, deposits, developments, queues, and
  accounting while community state represents population.
- Implement the designer-authored Collector, Battery, Extractor, Refinery, and
  FIFO construction contracts in short exact Tier 1 fixtures.
- Support the population-zero origin and its one free construction work without
  adding population arrival, loss, growth, or mutation behavior.
- Leave `ReclaimableSiteDefinition` unchanged.
- Do not implement map generation, G18 output guarantees, scouting/outward
  commands, generator identity, or playable startup.

### Stage 4b — Implement constructive frontier and bounded expansion

**Status:** completed on 2026-07-21 by
[PR #15](https://github.com/aefreedman/4x-term/pull/15). The retained two-crate
workspace now implements `core:frontier_world@1`, strict editable profiles and
complete generation identity, fixed-point geometric routing, body-owned
resources, a global ten-phase atomic tick, Habitat-backed population, delayed
origin knowledge, Shipyards, probes, expeditions, founding, explicit loss, and
a knowledge-filtered `PlayerWorldView`. The all-feature workspace suite has 56
focused deterministic tests with no ignored tests. Exact current contracts and
evidence are recorded in the [constructive-frontier plan](2026-07-20-feature-constructive-world-generation-stage-4b-plan.md),
the [architecture](../architecture.md), and the
[Engine Invariant Registry](../2026-07-20-engine-invariant-registry.md).

- Construct the approved origin-only G18 scaffold, then generate unconstrained
  frontier systems from explicit profiles. Target count, connectivity,
  reachability, quantities, and distributions are not generated-world quality
  oracles.
- Validate generator parameters/output with provenance and record generator
  family/revision, seed, normalized profile fingerprint, and complete generated
  definition. Generation identity does not promise runtime event-log replay.
- Replace explicit edges and standalone deposits with fixed-point procedural
  positions, geometric ship routes, and body-owned resource quantities.
- Implement Habitat population generation, Shipyards, probes, delayed origin
  knowledge, one-population expedition ships, and bounded founding with exact
  physical accounting.
- Test short authored mechanics, deterministic generator identity, ranges,
  references, and named invariants. Do not play generated worlds, require the
  target count exactly, or classify seed quality statistically.

Later world axes should have explicit extension and versioning rules rather
than an unsupported promise that all additions preserve old seed output.

### Stage 5 — Restore playable startup with the origin-and-frontier paradigm

**Status:** completed on 2026-07-21 and released in v0.8.0 by
[PR #16](https://github.com/aefreedman/4x-term/pull/16). The workspace now
composes generated content through `game-app`, presents only typed player-safe
views to `game-tui`, and ships the synchronous human-play `4x-term` executable.
Startup supports editable or random seeds, an allowlisted generated preview,
explicit confirmation, and exact preview consumption. Play begins at the sole
living origin; neutral systems remain hidden behind knowledge projections and
do not instantiate markets or NPC communities.

The playable surface covers origin resource management, construction, Habitat
population bootstrap, Shipyards, probe scouting, delayed reports, expeditions,
founding and loss, aliases, semantic keyboard layouts, terminal-size safety,
and paced one-tick advancement. Application tests cover preview identity and
staleness, player-safe projection, typed rejection and draft recovery, complete
Habitat bootstrap, and a committed tick followed by an atomic rejected tick.
TUI tests cover semantic input, stable selection by system identity, typed ship
actions, pacing, rendering, size safety, and staged terminal cleanup. Extensive
manual playtesting covers the composed startup, scouting, and founding journeys.

No compatibility path from the deleted trader game was added. The simulation
remains headless and frontend-independent; `game-app` exclusively owns mutable
`WorldState`, and terminal dependencies remain confined to `game-tui`.

### Stage 7 — Verify retirement of the obsolete market-network surface

**Status:** completed as part of the playable-startup audit. Repository,
dependency, documentation, and acceptance review found no retained trader-first
runtime, market-network compatibility path, or legacy world-quality gate.

Deletion belongs primarily to Stages 2–3. This stage is a final search and
proof that no obsolete product assumptions or accidental compatibility paths
remain; it is not a holding area for deferred demolition.

- Confirm the legacy 20-system content, independent NPC trader ecology,
  obsolete startup paths, diagnostics, tests, and UI are absent from the
  working tree unless a current responsibility explicitly justifies them.
- Confirm no content or implementation copy was quarantined in `archive/`;
  use Git history for archaeology.
- Confirm CI contains no global stability, trade churn, universal market
  health, or other legacy world-quality gate.
- Reassess market/economy code by gameplay responsibility: retain what the
  origin and future daughter communities need, reshape trade into
  player-owned logistics where appropriate, and remove the rest.

This stage is complete only when obsolete gameplay is no longer the hidden
compatibility target for new work.

### Stage 8 — Hand off expansion-gameplay ideas

**Status:** completed. The future directions for deeper scouting and
expeditions, resource ruins, site reclamation, cultural influence and
delegation, specialists, richer delayed information, and expanded production
chains are recorded in [Future Feature Ideas](../ideas.md). Automated
freight/logistics was deliberately not carried forward in this handoff.

These ideas are not implementation contracts. Each requires an approved design
and focused implementation plan before development. Stage 4b remains limited to
its approved Habitat, probe, one-population founding, and origin-recipient
information contracts.

## Cross-stage constraints

- Preserve deterministic, checked simulation arithmetic and explicit
  ordering. Any stronger cross-platform or spatial determinism contract
  must be specified by the stage that owns it.
- Preserve validate-before-mutate, source-aware diagnostics, stable domain
  identifiers, and frontend-independent simulation boundaries where those
  contracts remain applicable. A legacy headless command or playable loop is
  not itself a retained contract.
- Do not add reject/reroll generation, seed screening, or statistical world
  quality gates in any intermediate stage.
- Markets, traders, pricing, and NPC behavior have no compatibility
  guarantee. Delete them as soon as retained responsibilities are isolated;
  do not preserve them to minimize code change, keep tests green, or maintain
  interim playability.
- Do not copy removed source, content, tests, diagnostics, UI, or superseded
  migration docs into a working-tree archive. Git history is sufficient.
- The unrelated untracked `.obsidian/` directory must remain untouched.

## Direction-level completion

This transition completed with v0.8.0 when:

- normal startup represents one persistent origin seat/community in a generated dead frontier and uses the approved Habitat-backed population/founding model;
- approved G18 structural guarantees are constructive and exactly validated;
- mechanism tests use small authored fixtures;
- generated-world gates assert only named, applicable invariants;
- texture diagnostics have no CI pass/fail semantics;
- the legacy authored market network and independent NPC trader ecology no
  longer define product behavior or acceptance; and
- subsequent gameplay work can build scouting, reclamation, expansion, and
  delegation without pretending that empty locations are live markets.
