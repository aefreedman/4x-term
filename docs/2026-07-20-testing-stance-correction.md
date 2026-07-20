# Direction Shift: Testing Stance and Constructive Worldgen

## Status and use

This is a high-level transition direction, not a single implementation
plan. It corrects how the simulation is tested and describes the staged
move from the current authored market-trading prototype toward the strategy
and map-expansion game defined by the
[Governance Sandbox design direction](2026-07-20-design-direction-governance-sandbox.md),
especially G2 and G16–G22.

The stages below must be carved into focused plans or todos before
implementation. Those plans should investigate the repository as it exists
at that time and supply their own exact contracts, migration inventories,
and acceptance commands rather than treating this document as an
executable specification.

## The problem being corrected

The current test universe (the authored ~20-system world) serves as both
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
irrelevant everywhere else** (G18). The world starts dead except for the
player's origin community (G17); there is no NPC market network whose
health needs guaranteeing.

## Relationship to the current implementation

The existing game and codebase are inputs to a migration, not constraints
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

- Small hand-written worlds: roughly 3–6 systems, minimal ships.
- Deterministic, with hand-computable expected outcomes.
- Test *mechanisms*, one or few at a time: ladder-stage transitions at
  known ticks, seasonal oscillation arithmetic, population hysteresis
  stepping, reserve/funded-quantity recomputation per stage, and (as they
  land) expedition resolution, survey layers, training recipes.
- Exact assertions are correct here. Agents may and should "solve for the
  fixture" — that is what fixtures are for.
- Fast: runnable in seconds, suitable for inner-loop iteration and smoke
  testing.

### Tier 2 — Generated worlds (test invariants and guarantees only)

Generated-world tests assert exactly two kinds of things, both
deterministic and exact — **no statistical acceptance criteria, no
N-of-M seed thresholds, no viability screening or reject/reroll**:

1. **Engine invariants**, on a handful of arbitrary seeds, over soak-length
   runs: integer determinism (identical seed → identical run),
   conservation, anti-strand for automated logistics, no deadlock,
   validate-before-mutate. An invariant violation on any seed is always a
   bug. (Extend this list from the invariants actually named in the repo;
   do not invent new ones.)
2. **The constructive guarantees of G18**, as exact per-seed assertions on
   generator output:
   - **Origin solvency with surplus margin**: the origin system alone
     covers its life-support burn through the worst seasonal phase, with
     authored margin above subsistence. This is a direct inequality check
     against the generated configuration using the solvency math established
     by [Slice 1][slice-1] — no simulation required.
   - **Neighborhood affordance**: within starting expedition/scouting
     range, extractable resources meet an authored floor and at least one
     reclaimable site exists.

Everything else observed in a generated world — regions dying, odd
resource distributions, economic weirdness at the frontier — is
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
5. **Viability is constructed, not screened.** The generator builds a
   valid origin and neighborhood by construction. Do not add
   reject/reroll loops, post-hoc viability filters, or statistical
   world-quality gates.
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
that must share one branch or plan. Each stage should leave the repository
coherent and should produce the evidence needed to scope the next stage.

### Stage 1 — Record the direction and audit the migration surface

**Status:** recorded on 2026-07-20 in the
[authored market-world migration audit](2026-07-20-authored-market-world-migration-audit.md).
This records migration decisions only; Stages 2–8 remain future work.

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

### Stage 2 — Establish the two-tier test boundary

- Extract hand-computable mechanism coverage into Tier 1 micro-fixtures.
- Define a named invariant registry with an exact oracle and applicability
  rule for each invariant. Do not count vacuous checks as coverage.
- Separate descriptive world/economy reports from pass/fail validation.
- Remove statistical, metastability, and authored-map quality assertions.
  Preserve their mechanism or invariant coverage only when the underlying
  behavior remains part of the new game; otherwise delete that coverage
  with the obsolete feature.

This stage protects refactoring work without requiring the destination
world model to exist yet.

### Stage 3 — Separate map geography from living economic actors

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

### Stage 4 — Specify and implement constructive world generation

- Define the exact units, values, and inequalities for origin surplus and
  neighborhood affordance before implementing the generator.
- Generate an origin, starting neighborhood, and unconstrained frontier;
  "unconstrained" means unconstrained by viability, not unconstrained by
  the map-scale direction in G10.
- Validate authored generator parameters and generated output through the
  reusable content pipeline with meaningful provenance.
- Define replay identity and compatibility in the stage plan. At minimum it
  must account for generator version, seed, and validated configuration,
  rather than assuming a seed alone is sufficient.
- Assert both G18 guarantees exactly for every successful generated output;
  seed corpora are regression coverage, not statistical viability proof.

Later world axes should have explicit extension and versioning rules rather
than an unsupported promise that all additions preserve old seed output.

### Stage 5 — Cut runtime startup over to the origin-and-frontier paradigm

- Decide how a generated world is selected and composed for normal play,
  tests, diagnostics, and replay.
- Start the player as the origin community/governor, not as an independent
  trader in a populated market network.
- Ensure non-origin locations do not silently instantiate living markets or
  independent NPC communities.
- Update application and CLI startup boundaries while keeping the
  simulation headless and frontend-independent.

This is the product-paradigm cutover. It should not be hidden inside a test
or generator implementation task.

### Stage 6 — Add generated-world invariant soaks and replay tooling

- Run only applicable named invariants against generated worlds.
- Keep validate-before-mutate and other rejection behavior in focused
  fixtures unless a generated harness deliberately exercises those paths.
- Require non-vacuous setup for automated-logistics liveness checks; until
  player-owned delegated logistics exists, such checks may remain focused
  mechanism coverage rather than generated-world acceptance.
- Add single-world replay with full event logging and complete generation
  identity for failure forensics.
- Keep distribution and frontier-texture summaries descriptive and outside
  CI pass/fail semantics.

### Stage 7 — Complete retirement of the obsolete market-network surface

Conflicting behavior may and should be removed in any earlier stage when it
blocks the migration. This stage is the final cleanup and proof that no
obsolete product assumptions remain.

- Delete or quarantine the legacy 20-system authored world once retained
  mechanism coverage has moved to appropriate fixtures.
- Remove independent NPC trader ecology, obsolete startup paths, and tests
  whose premise conflicts with G17/G19.
- Remove or rewrite CI and diagnostic gates that require global stability,
  trade churn, universal market health, or other world-quality outcomes.
- Reassess market/economy code by gameplay responsibility: retain what the
  origin and future daughter communities need, reshape trade into
  player-owned logistics where appropriate, and remove the rest.

This stage is complete only when obsolete gameplay is no longer the hidden
compatibility target for new work.

### Stage 8 — Hand off to expansion-gameplay slices

After the world and testing foundations are truthful, carve separate plans
for scouting, expeditions, resource ruins, site reclamation, daughter
communities, delegated logistics, specialists, and delayed information.
Those plans derive from G7–G16 and the open questions in the governance
sandbox; they are not bundled into this testing/worldgen transition.

## Cross-stage constraints

- Preserve deterministic, checked simulation arithmetic and explicit
  ordering. Any stronger cross-platform or spatial determinism contract
  must be specified by the stage that owns it.
- Preserve validate-before-mutate, source-aware diagnostics, stable domain
  identifiers, and the headless simulation boundary where those contracts
  remain applicable.
- Do not add reject/reroll generation, seed screening, or statistical world
  quality gates in any intermediate stage.
- Markets, traders, pricing, and NPC behavior have no compatibility
  guarantee. Keep them only when a stage demonstrates their role in the
  new gameplay pressure; do not preserve them solely to minimize code
  change.
- The unrelated untracked `.obsidian/` directory must remain untouched.

## Direction-level completion

This transition is complete when:

- normal startup represents one living origin in a generated dead frontier;
- the two G18 guarantees are constructive and exactly validated;
- mechanism tests use small authored fixtures;
- generated-world gates assert only named, applicable invariants;
- texture diagnostics have no CI pass/fail semantics;
- the legacy authored market network and independent NPC trader ecology no
  longer define product behavior or acceptance; and
- subsequent gameplay work can build scouting, reclamation, expansion, and
  delegation without pretending that empty locations are live markets.

[slice-1]:
  ../todos/006-complete-p1-slice-1-energy-denominated-economy-foundation.md
