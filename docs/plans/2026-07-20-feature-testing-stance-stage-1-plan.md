---
title: Testing Stance Correction — Stage 1 Governance and Migration Audit
type: feature
date: 2026-07-20
---
# Testing Stance Correction — Stage 1 Governance and Migration Audit

> **Completion context:** This plan remains part of the active staged-migration
> record. References below to the prototype archive and completed prototype todos
> describe the working tree when Stage 1 was completed; Stage 2 later removed
> those artifacts in favor of Git history. They are not current retention
> requirements.

## Overview

Record the corrected two-tier testing stance in the repository’s contributor and architecture guidance, clearly label the authored market-trading game as a migration input rather than the product target, and create an evidence-backed migration audit. The audit will classify affected content, startup, runtime/data-model, frontend, tests, diagnostics, and CI responsibilities as **keep**, **reshape**, **replace**, or **remove**.

This stage is documentation and governance only. It must leave Rust code, authored content, executable behavior, tests, diagnostics, and CI commands unchanged. Later stages own test extraction, model redesign, world generation, startup cutover, and deletion.

## Problem Statement / Motivation

The repository still presents the authored 20-system market network as current gameplay and retains acceptance paths that reject extinction, aggregate collapse, population ratchets, or missing late market activity. Those expectations conflict with the governance-sandbox direction, where local collapse is intended texture and only named engine invariants or constructive G18 guarantees are failures.

The contradiction is visible across the current surfaces:

- Runtime content compilation requires exactly 20 systems and one market per system. `crates/game-content/src/lib.rs:499-514,881-930`
- Startup always loads the authored content bundle before selecting validate, diagnostic, headless, or TUI behavior. `crates/game-cli/src/main.rs:15-35`
- The long diagnostic validator rejects any extinct system, large population decline, and loss of market activity. `crates/game-cli/src/main.rs:1122-1180`
- Default integration coverage requires a connected multi-hop player trade with positive sales revenue. `crates/game-cli/tests/boundaries.rs:19-89`
- CI directly gates repository content validation and authored-world headless startup. `.github/workflows/ci.yml:21-26`
- `README.md` still describes the authored governorship, adaptive NPC fleet, and 10,000-tick metastability run as active product/acceptance behavior. `README.md:39-69`

Without one authoritative audit, later work risks preserving obsolete behavior accidentally, deleting durable mechanism coverage, or tuning the authored universe as if it were the destination.

## Proposed Solution

### 1. Establish contributor-facing governance

Update `AGENTS.md` with an explicit “Testing and world generation” section adapted from the seven norms in the direction document:

- Individual generated-seed outcomes fail only for a named invariant or G18 guarantee.
- Local collapse is expected texture.
- Authored-universe acceptance is limited to small, hand-computable Tier 1 fixtures.
- Gameplay-facing behavior requires short deterministic fixture coverage; soak-only validation identifies simulation behavior.
- Viability is constructed, never screened or rerolled.
- Generator range changes require design review and reproducibility consideration.
- Generated-world failure classes should become retained Tier 1 fixtures where possible.

Also state the compatibility stance directly: the current trader-first market network is a **full replacement target**, with historical reference retained only in `archive/`; no compatibility layer is implied.

### 2. Align architecture and project-facing documentation

Update `docs/architecture.md` so its testing section defines:

1. Tier 1 authored micro-fixtures for exact mechanism outcomes.
2. Tier 2 generated worlds for exact named invariants and constructive guarantees only.
3. Descriptive diagnostics as non-gating output.

Preserve the existing durable architecture contracts—headless simulation, explicit ordering, checked physical-resource arithmetic, stable identifiers, validate-before-mutate, source-aware content validation, and frontend boundaries. Mark markets, independent traders, trader wallets, universal market population, pricing, commercial contracts, and metastability as prototype responsibilities rather than architecture.

Add a short transitional notice to `README.md`: the runnable build remains the legacy authored market prototype while migration is underway; current product direction is defined by the governance sandbox and testing stance. Relabel the metastability and player-impact commands as legacy diagnostics that must not establish current acceptance policy. Do not delete operational instructions in Stage 1.

Repair the stale internal links introduced by the latest document moves in:

- `docs/plans/2026-07-20-testing-stance-correction.md`
- `docs/design/direction/README.md#legacy-g-label-mapping`
- `archive/README.md`
- `todos/007-complete-p1-slice-2-world-dynamics-population-and-player-progression.md`

### 3. Create the migration-surface audit

Create `docs/plans/2026-07-20-authored-market-world-migration-audit.md` as the Stage 1 decision record. Give every row:

- responsibility/mechanism;
- current files or symbols;
- current coupling to the authored market world;
- classification (**keep**, **reshape**, **replace**, or **remove**);
- rationale tied to G17–G22 or a durable architecture contract;
- owning future stage (2–7);
- test disposition and exact follow-up question where unresolved.

Organize the inventory by surface:

| Surface | Required inventory coverage |
| --- | --- |
| Authored content | `systems.ron`, `economy.ron`, `economy_config.ron`, `traders.ron`, and player-facing legacy descriptions in `encyclopedia.ron` |
| Content pipeline | Exact-20 validation, market-per-system compilation, graph/connectivity rules, bootstrap solvency warnings, trader/fleet compilation, reusable schema/source diagnostics |
| Runtime startup | Shared authored-directory load, TUI default, headless acceptance, content validation, economy diagnostics, pricing comparison, and player-impact modes |
| Core data model | `GameDefinition`, `SystemDefinition`, `Market`, `Trader`, fleet dynamics, population/brownout state, reservations/contracts, snapshots, graph/topology, checked Energy ledgers |
| App/TUI | Player-trader location/cargo, local trade, all-system market views, read-only autonomous markets, governance, Energy logistics, event/view boundaries |
| Tests | Micro-mechanism tests, atomic rejection/conservation tests, repository-content freeze tests, authored-world activity tests, trade acceptance, diagnostic-validator tests, ignored 1,000-tick acceptance |
| Diagnostics | Reconciliation/invariant evidence, texture summaries, player-impact gating, metastability gating, market/fleet activity thresholds |
| CI/docs | Workspace tests, content validation, headless acceptance, README commands, archived evidence, contributor guidance |

Use the following initial decision baseline; deviations require an explicit rationale in the audit:

- **Keep:** deterministic scheduling and replayable fixed inputs; checked Energy/resource accounting; validate-before-mutate; stable IDs; source-aware validation; headless core and app/frontend boundaries; exact non-vacuous mechanism tests. The existing atomicity solution remains institutional guidance. `docs/solutions/rust-ecs-validate-before-mutate.md:13-33`
- **Reshape:** physical resource logistics, life-support pressure, brownout/population/season arithmetic, investments, map topology, governance, and anti-strand responsibilities so they serve origin/daughter communities and player-owned logistics rather than autonomous market ecology.
- **Replace:** exact-20 authored startup, market-per-location world representation, trader-first player identity, generated-world selection/replay identity, repository-world acceptance, and pass/fail diagnostic boundaries.
- **Remove:** independent NPC fleet ecology as a product requirement; metastability, universal survival, global-collapse, population-ratchet, ongoing-trade, and player-impact quality gates; market/pricing/wallet/reservation behavior that no accepted future responsibility needs.

Where a broad subsystem contains mixed responsibilities, split it into multiple rows rather than assigning one classification to an entire file. For example, classify exact Energy reconciliation separately from metastability checks inside the same diagnostic harness.

### 4. Close Stage 1 without pre-implementing later stages

Link the completed audit from the direction document and mark Stage 1’s evidence as recorded without marking Stages 2–7 complete. Add a “Stage 1 boundary” section to the audit listing prohibited work:

- no Rust or RON edits;
- no test deletion or fixture extraction;
- no CI gate changes;
- no world-model or generator design;
- no startup cutover;
- no archival/deletion of the runnable prototype.

## Technical Considerations

### Architecture Impacts

Stage 1 changes architecture **documentation**, not architecture implementation. It resolves which current contracts are durable and which are temporary, while preserving the headless/core/frontend boundaries already documented. `docs/architecture.md:7,185-193,296,308-325`

The audit should distinguish geography from living economic actors even though Stage 3 owns that code change. Current definitions embed inventories, population, policy, governance, and logistics in every `SystemDefinition`, while snapshots expose markets and traders as the world’s primary shape. `crates/game-core/src/lib.rs:663-790,1624-1682,1838-1860,2498-2514`

### Performance Implications

None. No runtime or CI behavior changes in Stage 1.

### Security Considerations

None beyond normal repository hygiene. Do not include machine-local `.pi/`, `.obsidian/`, logs, generated indexes, or build output in the audit.

### Compatibility Stance

- **Current gameplay:** full replacement over later stages.
- **Current save/data compatibility:** not promised; Stage 1 records the issue but does not design a migration.
- **Archived documents:** historical reference only.
- **Runnable prototype during Stage 1:** preserved unchanged so the repository remains coherent.

## SpecFlow Analysis

### Flow Overview

1. A contributor reads `AGENTS.md` before changing simulation, content, tests, or generation and receives the corrected failure policy.
2. The contributor follows links to architecture and direction documents with no stale paths.
3. A Stage 2–7 planner opens the audit, finds the relevant responsibility, evidence, classification, test disposition, and owning stage.
4. If a generated-world failure is reported, the guidance first asks whether it violates a named invariant/G18 guarantee, then recommends a retained Tier 1 reproduction where possible.
5. A user can still run the authored prototype, but README wording no longer presents its metastability behavior as current product acceptance.

### Important Variations and Edge Cases

- A single file may contain both durable and obsolete behavior; classifications apply to responsibilities, not filenames.
- “Keep” preserves a contract or responsibility, not necessarily the present type/API unchanged.
- “Remove” does not require a replacement unless the future game needs the responsibility.
- Unknown destination details remain explicit follow-up questions assigned to a later stage; Stage 1 must not invent generator or world-model contracts.
- Archived documents may retain obsolete language, but every entry point must label them historical and link to current direction.

### Provisional Assumptions

- `docs/plans/2026-07-20-testing-stance-correction.md` remains the transition source of truth.
- The audit is a dated top-level design document, while this implementation plan lives under `docs/plans/`.
- No issue tracker integration is configured in `AGENTS.md`, so this plan creates no external issue.

## Acceptance Criteria

- [x] `AGENTS.md` records all seven testing/worldgen norms in concise contributor-facing language and states the no-compatibility stance for obsolete gameplay.
- [x] `docs/architecture.md` documents Tier 1, Tier 2, and descriptive diagnostics, while retaining the named durable architecture contracts.
- [x] `README.md` clearly distinguishes the runnable legacy prototype from current product direction and no longer presents metastability/player-impact behavior as current acceptance policy.
- [x] `docs/plans/2026-07-20-authored-market-world-migration-audit.md` inventories every required surface and gives each responsibility evidence, one classification, rationale, future-stage owner, and test disposition.
- [x] Mixed diagnostic and test responsibilities are split so invariant coverage is not accidentally classified with authored-world quality gates.
- [x] All links to the renamed governance-sandbox and testing-stance documents resolve.
- [x] The direction document links to the completed Stage 1 audit without implying later-stage completion.
- [x] No `.rs`, `.ron`, workflow, Cargo, archived historical-content, generated, or machine-local file is modified.
- [x] Existing test and CI commands remain unchanged and continue to pass.

## Success Metrics

- **Coverage:** every content/startup/runtime/frontend/test/diagnostic/CI category named by Stage 1 has at least one evidence-backed audit row and no unclassified known dependency.
- **Decision clarity:** every row has exactly one primary classification and a future-stage owner; unresolved design details are phrased as bounded questions, not hidden compatibility assumptions.
- **Policy consistency:** searches of current, non-archived guidance find no unqualified claim that local collapse, universal survival, or metastability is a current quality gate.
- **Repository coherence:** all current-direction Markdown links resolve and the runnable prototype remains unchanged.

## Dependencies & Risks

### Dependencies

- Governance Sandbox G17–G22 and its testing implications. `docs/design/direction/README.md#legacy-g-label-mapping`
- Testing stance, norms, and Stage 1 contract. `docs/plans/2026-07-20-testing-stance-correction.md:46-56,62-107,110-152`
- Existing architecture boundaries and testing strategy. `docs/architecture.md:7,185-193,296-325`

### Risks

- **Over-broad classification:** marking a whole subsystem “remove” could discard durable arithmetic or boundary contracts. Mitigation: classify responsibilities at row level and split mixed files.
- **Premature destination design:** the audit could accidentally specify Stage 3–6 implementations. Mitigation: record owners and open questions; do not define new schemas, generators, or replay contracts.
- **Governance/runtime mismatch:** readers may interpret new guidance as describing implemented behavior. Mitigation: README and audit must explicitly distinguish current executable state from target direction.
- **Stale inventory:** code may change before a later stage consumes the audit. Mitigation: record line-cited evidence and require each later plan to re-verify its rows.
- **False test coverage:** an existing repository-world check may look like an invariant while depending on authored activity. Mitigation: require exact oracle, applicability, and non-vacuity notes for every test disposition.

## Implementation Notes

### Files to Create

- `docs/plans/2026-07-20-authored-market-world-migration-audit.md` — authoritative Stage 1 inventory and classification record.

### Files to Modify

- `AGENTS.md` — contributor testing/worldgen norms and compatibility stance.
- `docs/architecture.md` — two-tier testing architecture and explicit prototype boundaries.
- `README.md` — transitional legacy-prototype notice and non-authoritative diagnostic labels.
- `docs/plans/2026-07-20-testing-stance-correction.md` — repaired links and Stage 1 audit reference/status.
- `docs/design/direction/README.md#legacy-g-label-mapping` — repaired testing-direction link.
- `archive/README.md` — repaired current-direction links.
- `todos/007-complete-p1-slice-2-world-dynamics-population-and-player-progression.md` — repaired supersession links only; preserve its obsolete historical content.

### Files Explicitly Not Modified

- `crates/**`
- `content/**`
- `.github/workflows/**`
- `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`
- `archive/market-trading-prototype/**`
- `.obsidian/**`, `.pi/**`, `.compound-game-dev/**`, `target/**`

## Testing Strategy

### Test-Development Recommendations

Stage 1 adds no Rust tests because it changes no executable behavior. The audit must nonetheless create a concrete Stage 2 test-development backlog:

- Map every existing test family to **Tier 1 mechanism**, **candidate named invariant**, **constructive guarantee**, **descriptive-only**, or **obsolete premise**.
- For each candidate invariant, record its exact oracle, applicability rule, and required non-vacuous setup before treating it as generated-world coverage.
- Preserve atomic rejection and checked-accounting regression tests as durable mechanism/invariant evidence.
- Convert real generated-world failures into small retained fixtures where a hand-computable reproduction is possible.
- Avoid mutable repository-content magic numbers except where a Stage 2 migration freeze test is explicitly temporary.
- Keep long-run seed corpora as deterministic regression coverage only; never introduce pass percentages, reject/reroll screening, or world-quality thresholds.

### Automated Validation

Run after documentation edits:

```bash
git diff --check
cargo fmt --all -- --check
cargo test --workspace --all-features
cargo run -p game-cli -- --validate-content
cargo run -p game-cli -- --headless
```

Also run a local relative-Markdown-link check over tracked non-archived current guidance, and verify required policy language with focused `git grep` searches. Since the repository has no dedicated Markdown test framework, use a short standard-library-only Python check or equivalent existing local tool; do not add a dependency solely for Stage 1.

### Manual Review

- [x] Follow every current-direction link from `README.md`, `AGENTS.md`, architecture, direction documents, archive entry point, and obsolete todo.
- [x] Review each audit row against its cited source and confirm the classification applies to the responsibility rather than the whole file.
- [x] Confirm no current guidance implies that an arbitrary generated region must survive or that one seed should drive tuning.
- [x] Review `git status --short`; confirm the planned Markdown files are the only task changes and the unrelated untracked `notes/2026-07-16.md` remains unmodified and unstaged, with no machine-local paths included.

## References & Research

References are project-root relative.

### Evidence Index

- **E1 — Direction and scope:** `docs/plans/2026-07-20-testing-stance-correction.md:46-56,62-107,110-152`
- **E2 — Architecture contracts:** `docs/architecture.md:7,185-193,296-325`
- **E3 — Content/startup coupling:** `crates/game-cli/src/main.rs:15-35,52-72`; `crates/game-content/src/lib.rs:412-428,499-514,881-930`
- **E4 — Core model coupling:** `crates/game-core/src/lib.rs:525-582,663-790,1624-1682,1838-1860,2498-2514`
- **E5 — Frontend coupling:** `crates/game-app/src/lib.rs:77-129,360-398,531-555,666-727`
- **E6 — Authored-world tests/gates:** `crates/game-cli/tests/boundaries.rs:19-89`; `crates/game-content/src/lib.rs:1935-2015,2461-2560,2686-2710`; `.github/workflows/ci.yml:21-26`
- **E7 — Metastability diagnostics:** `crates/game-cli/src/main.rs:52-72,570-630,1122-1180`
- **E8 — Institutional atomicity guidance:** `docs/solutions/rust-ecs-validate-before-mutate.md:13-33`

### Institutional Knowledge

- `docs/solutions/rust-ecs-validate-before-mutate.md` — retain validate-then-apply atomicity independently of whether markets, traders, or their current commands survive.
- No other relevant solution document was found in the scoped `${DOCS_ROOT}/solutions/` search.

### Research Scope and Decisions

- VCS: Git; `.gitignore` honored. Rust workspace; no Unity project detected.
- Broad supplemental research: skipped because Stage 1 is a project-local governance and inventory task with strong authoritative local direction.
- Authoritative external docs cross-check: skipped because the plan makes no engine/framework/package API claims.
- `cg_search_repo` was unavailable because `rg` is not installed; bounded `git grep`, `find`, direct reads, and three parallel repository-research spikes supplied the evidence.
- No issue was created because `AGENTS.md` has no configured `issue_tracker` value.
