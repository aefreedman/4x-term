---
title: Authored Market-World Migration Audit
type: audit
date: 2026-07-20
status: recorded
source_direction: docs/2026-07-20-testing-stance-correction.md
---
# Authored Market-World Migration Audit

## Purpose

This is the Stage 1 decision record for moving from the runnable authored
20-system market-trading prototype to the governance-and-expansion game in the
[Governance Sandbox](2026-07-20-design-direction-governance-sandbox.md). It
classifies responsibilities, not whole files, as **keep**, **reshape**,
**replace**, or **remove**. A file appears in more than one row when it contains
both durable contracts and obsolete product assumptions.

The current executable remains coherent and unchanged. These decisions guide
Stages 2–7 of the
[Testing Stance and Constructive Worldgen transition](2026-07-20-testing-stance-correction.md);
they do not claim that the destination model already exists.

## Classification rules

- **Keep**: preserve the responsibility or contract. Its present API may still
  change.
- **Reshape**: retain the gameplay responsibility but re-derive it for origin
  and daughter communities, dead geography, or player-owned logistics.
- **Replace**: introduce a different responsibility or representation; no
  compatibility layer is required.
- **Remove**: delete when its retained mechanism coverage has moved or its
  obsolete feature is retired. No replacement is required unless the new game
  needs the responsibility.

Future owners are the numbered transition stages. Stage 2 establishes test
boundaries, Stage 3 separates geography from living actors, Stage 4 owns
constructive generation, Stage 5 cuts over startup/player identity, Stage 6
owns generated-world invariant/replay tooling, and Stage 7 completes
retirement.

## Migration inventory

### Authored content

| Responsibility and evidence | Coupling today | Decision and rationale | Owner | Test disposition / bounded follow-up |
| --- | --- | --- | --- | --- |
| Authored system list and coordinates — `content/systems.ron`; `SystemSource` in `crates/game-content/src/lib.rs:83-93` | The repository universe is a fixed list of 20 named systems. | **Replace.** G17–G18 require generated dead geography with one constructed living origin, not a canonical populated universe. | 3–5 | Stage 2 may keep only deliberately small coordinates as fixtures. Stage 4 must define generation identity and map-scale constraints before replacement. |
| Per-system economy records — `content/economy.ron`; `EconomySource` in `crates/game-content/src/lib.rs:140-155` | Every authored system receives inventory, targets, production, population, policy, and optional governance. | **Replace.** Empty locations must not silently become live economies; location data and community data need separate composition. | 3–5 | Preserve no repository-world numeric oracle. Which minimum community configuration serves G18 belongs to Stage 4. |
| Resource-pressure tuning — brownout, population, production, Energy logistics, and investment fields in `content/economy_config.ron`; `EconomyConfigSource` in `crates/game-content/src/lib.rs:157-264` | Mechanism parameters are tuned against every authored market and share configuration with obsolete ecology policy. | **Reshape.** G19 and G22 preserve only re-derived community pressure, life-support, production, physical movement, and expansion-facing investment responsibilities. | 2–4 | Extract small mechanism fixtures before schema changes. Stage 3 decides configuration boundaries; Stage 4 owns generator parameters rather than current balance values. |
| Market and fleet tuning — market policy, adaptive NPC response, retirement, and repositioning fields in `content/economy_config.ron` | These values exist to stabilize or animate autonomous market/trader ecology. | **Remove.** G19 and G21 reject that ecology and its quality targets. | 2, 7 | Keep no current threshold oracle. Delete fields with their consuming features after any independent arithmetic coverage is extracted. |
| Player-trader identity configuration — the player branch of `content/traders.ron`; `TraderConfigSource` in `crates/game-content/src/lib.rs:318-350` | Normal play starts as one specially flagged trader. | **Replace.** G17 starts the player as an origin community/governor rather than an independent trader. | 5 | Stage 5 supplies origin-first startup coverage before retiring the current player definition. |
| Independent NPC fleet configuration — NPC/archetype/fleet branches of `content/traders.ron` | Nine initial NPCs and adaptive archetypes populate the authored market network. | **Remove.** G19 strikes independent NPC fleet ecology; future delegated logistics is newly specified player-owned behavior. | 7 | Keep no fleet-count, profitability, spawn, or retirement oracle. |
| Movement capacity, time, and Energy-cost parameters in `content/traders.ron` | Durable transport responsibilities are encoded inside obsolete trader definitions. | **Reshape.** G22.4 preserves non-teleporting physical movement, not the current actor model. | 2, 3, 5 | Retain exact route/capacity/Energy arithmetic fixtures where applicable; Stage 3–5 decide the destination owner. |
| Wallet, ordinary cargo-for-profit, and commercial policy in `content/traders.ron` | These fields support Energy-denominated market trading and commercial contracts. | **Remove.** G22 does not preserve Energy as universal money, wallets, or commercial delivery policy. | 7 | Preserve only generic checked-transfer tests that another accepted responsibility consumes. |
| Goods and recipes — `content/goods.ron`, `content/recipes.ron` | Prices and Energy-denominated production recipes serve trading among autonomous markets. | **Reshape.** G20–G22 retain physical goods, Energy/input costs, and a resource chain, but require new expansion-facing content and do not retain prices as a design contract. | 3–4, 7 | Preserve checked recipe arithmetic fixtures. Future content work must decide goods and recipes independently of the migration audit. |
| Player-facing manual — `content/encyclopedia.ron` | Articles explain the current market, trade, Energy-contract, fleet, and governance prototype. | **Replace.** Player-facing truth must follow the origin-and-expansion game after its mechanics exist. | 5, 7 | Do not rewrite speculative articles in Stage 1. Stage 5 identifies the minimum truthful startup/manual content. |

### Content pipeline

| Responsibility and evidence | Coupling today | Decision and rationale | Owner | Test disposition / bounded follow-up |
| --- | --- | --- | --- | --- |
| RON loading, typed conversion, stable IDs, aggregated source-aware errors — `load`, `compile`, and diagnostics in `crates/game-content/src/lib.rs:412-498` | Reusable validation is implemented alongside market-specific compilation. | **Keep.** Stable IDs, typed values, and useful provenance remain durable architecture contracts. | 3–4 | Stage 2 keeps invalid-value, duplicate-ID, unresolved-reference, and aggregated-error tests. Stage 3 identifies a reusable validator boundary without adding a new crate absent concrete need. |
| Exactly 20 systems — `crates/game-content/src/lib.rs:499-514` | Compilation rejects any other universe size. | **Remove.** Neither Tier 1 fixtures nor generated worlds have this cardinality contract. | 3–4 | Stage 2 classifies exact-count tests obsolete; Stage 3 removes the rule when empty geography is representable. |
| One market/economy record per system — `MarketSource` and compile checks in `crates/game-content/src/lib.rs:266-316,881-930` | Geography cannot exist without a live market and its economic state. | **Replace.** G17 requires dead locations and a separately instantiated origin community. | 3 | Add focused content tests for locations without communities only when Stage 3 defines that schema. |
| Graph construction, finite coordinates, connectivity and route energy — `SystemGraph` use and graph checks in `crates/game-content/src/lib.rs`; `crates/game-core/src/lib.rs` | Topology is compiled from systems but consumed primarily for trader routes and market connectivity. | **Reshape.** Spatial topology remains useful, while universal connectivity and trader-route assumptions require re-derivation against G10 and exploration. | 3–4 | Preserve finite-coordinate and arithmetic tests where intentional. Must generated geography be connected, partially connected, or navigable through another rule? Stage 4 must answer. |
| Bootstrap solvency/runway and protected-liquidation checks — `crates/game-content/src/lib.rs:1760-1925,2710-2790` | Validation guarantees market import/runway and trader liquidation behavior in the authored ecology. | **Remove.** Current market solvency is not G18 origin solvency, and trader liquidation is not retained. | 2–4 | Keep generic checked inequalities only if Stage 4's constructive origin-solvency oracle uses them; otherwise delete after migration coverage is secured. |
| Trader/fleet compilation and archetype distribution — `crates/game-content/src/lib.rs:931-1210` | Successful content always instantiates player/NPC traders and fleet policy. | **Remove.** Independent NPC ecology and player-trader identity are obsolete. | 5, 7 | Stage 2 marks repository fleet-shape assertions obsolete. Player-owned automation gets new fixtures only after a gameplay contract exists. |

### Runtime startup and CLI modes

| Responsibility and evidence | Coupling today | Decision and rationale | Owner | Test disposition / bounded follow-up |
| --- | --- | --- | --- | --- |
| Repository-directory load before mode selection — `main` in `crates/game-cli/src/main.rs:15-35` and `content_path` at `:210-214` | TUI, validation, headless, diagnostics, and probes all begin from the same authored definition. | **Replace.** Normal play and generated-world tooling need explicit generation/configuration/replay composition rather than a hidden canonical universe. | 4–5 | Stage 5 defines selection and error contracts for normal play, tests, diagnostics, and replay. Do not hide cutover inside test utilities. |
| TUI default startup — `ExecutionMode::Tui` in `crates/game-cli/src/main.rs:104-111` | Default play creates a player trader in populated markets. | **Replace.** G17 starts the player as the origin community/governor. | 5 | Stage 5 adds startup acceptance for one living origin and dead non-origin locations. Save compatibility is not promised and must be decided explicitly if persistence exists then. |
| `--validate-content` — `ExecutionMode::ValidateContent` in `crates/game-cli/src/main.rs:37` | Validates the authored market bundle and its market/fleet-specific rules. | **Reshape.** A validation command remains useful, but its inputs and semantic rules must follow generated configuration and reusable content. | 3–5 | Preserve source-aware failure behavior. Stage 5 decides whether the command validates authored parameters, generated output, or both through explicit modes. |
| `--headless` authored acceptance — `ExecutionMode::Headless` in `crates/game-cli/src/main.rs:73-103`; `.github/workflows/ci.yml:25-26` | A successful headless run proves current repository startup and activity assumptions. | **Replace.** Headless execution is durable; authored-market acceptance is not. | 2, 5–6 | Keep the headless boundary, then give it an explicit micro-fixture or generated-world contract. Stage 5 defines the new CLI oracle before changing CI. |
| Exact reconciliation in headless and diagnostic modes — `reconcile_energy` call sites in `crates/game-cli/src/main.rs:101-102,488-492,1353-1355,1664-1666` | Durable accounting evidence is embedded in modes that also inspect authored-world quality. | **Keep.** G22.5 requires exact physical-resource reconciliation independent of current markets or probes. | 2 onward | Exact oracle: expected equals actual with every physical transfer channel accounted for. Applicability: every run that mutates physical resources. |
| Descriptive economy/world texture reporting — `ExecutionMode::EconomyDiagnostics` in `crates/game-cli/src/main.rs:55-72` | Current reports summarize authored markets, fleet activity, population, and Energy flow. | **Reshape.** Diagnostics remain useful for human worldgen tuning, but their future fields follow generated frontier texture and carry no quality verdict. | 2, 6 | Stage 2 separates reporting from validation. Stage 6 specifies generation/replay identity and useful non-gating summaries. |
| Pricing comparison, player-impact divergence, and metastability/market-activity gates — `crates/game-cli/src/main.rs:38-54,65,1122-1180` | These modes and validators judge obsolete pricing, intervention response, survival, and market ecology. | **Remove.** None is a named engine invariant or constructive G18 guarantee. | 2, 7 | Preserve exact inflow/reconciliation fixtures separately; the pricing/probe/gating behaviors need no replacement. |

### Core data model and simulation responsibilities

| Responsibility and evidence | Coupling today | Decision and rationale | Owner | Test disposition / bounded follow-up |
| --- | --- | --- | --- | --- |
| Stable IDs, deterministic schedule/order, checked integer Energy and validate-before-mutate — `crates/game-core/src/lib.rs`; [atomicity guidance](solutions/rust-ecs-validate-before-mutate.md) | These contracts currently operate through market, recipe, trader, and logistics systems. | **Keep.** They are explicit cross-stage architecture contracts and implement G22.5 independent of current gameplay nouns. | 2–7 | Preserve exact overflow, rejection atomicity, conservation, deterministic-order, and reconciliation fixtures; rewrite fixtures only when obsolete types are removed. |
| `SystemDefinition`, `Market`, and `CoreSnapshot.markets` — `crates/game-core/src/lib.rs:663-724,1624-1682,2498-2514` | A “system” embeds live economy inputs and the snapshot presents markets as the world. | **Replace.** G17 needs locations without living economic actors and separately composed communities. | 3 | Stage 3 must define only the minimum truthful substrate: dead locations, one living origin, extractable resources, and minimally typed reclaimable sites. Bodies, slots, ruin internals, surveys, and full information design remain out of scope. |
| Player-trader identity and special player flag — `TraderDefinition` and `Trader` in `crates/game-core/src/lib.rs:725-759,1838-1860` | Player startup is represented as one trader among autonomous market actors. | **Replace.** G17 requires origin-community/governor identity. | 5 | Stage 5 provides origin-first startup and command coverage before removing the old identity. |
| Independent NPC trader entities and ecology — the same trader types plus `FleetMode` at `crates/game-core/src/lib.rs:525-582` | Autonomous merchants own wallet, cargo, travel, and profitability state. | **Remove.** G19 and G21 reject independent NPC market ecology. | 7 | Keep no entity-lifecycle or profitability contract solely for compatibility. |
| Trader movement capacity, time, travel Energy, and physical carriage | Durable movement responsibilities live inside an obsolete actor type. | **Reshape.** G22.4 keeps capacity-, time-, and Energy-costed physical movement. | 2–5 | Preserve exact route, capacity, loading, and burn arithmetic only where applicable to accepted logistics. |
| Trader wallets, commercial cargo/profit, and reservation state | These fields implement Energy-denominated market exchange. | **Remove.** G22 explicitly does not preserve universal money, wallets, or commercial reservation policy. | 7 | Keep generic transaction atomicity only where another retained resource transfer consumes it. |
| Exact physical-resource accounting and ledgers | Reconciliation is implemented through current market, trader, and contract channels. | **Keep.** Checked deterministic arithmetic and exact reconciliation are G22.5 contracts regardless of destination ownership. | 2 onward | Tier 1 exact oracle: all sources, sinks, stores, and in-flight amounts reconcile, including rejected mutations. |
| Energy generation, storage, life-support burn, production, transport, sinks, and player-facing resource pressure | Current behavior is instantiated on every market and uses commercial contracts for movement. | **Reshape.** G22 preserves these responsibilities for living communities and expansion, not as universal money or market ecology. | 2–5 | Stage 3 decides which state belongs to communities; later gameplay slices define expansion spending and player-owned logistics. |
| Brownout ladder, population hysteresis, seasonal arithmetic, investments | These mechanisms are tuned to all authored markets and metastability. | **Reshape.** G19 says they may return only when re-derived for origin/daughter communities and player-facing pressure. | 2–3, 7 | Stage 2 can retain hand-computable mechanism fixtures without retaining current thresholds as product balance. Which mechanisms survive is a later design decision. |
| Market prices, bids/asks, wallets, reservations, ordinary trade and commercial Energy contracts | They coordinate autonomous market exchange and trader profit. | **Remove.** G21–G22 explicitly deny these a presumption of survival; physical transport does not require commercial-market semantics. | 7 | Preserve only generic checked transfer/transaction contracts where another accepted responsibility uses them. Otherwise delete tests with the feature. |
| Fleet spawn/retire ecology and profitability/opportunity response — `FleetMode` at `crates/game-core/src/lib.rs:525-582` | Autonomous traders adapt to unmet profitable market opportunity. | **Remove.** G19 strikes the NPC ecology. Future delegated fleets are player-owned and require a new contract. | 7 | Do not reuse spawn/retire thresholds as automation acceptance. A future fleet-management slice may borrow ideas but starts from player-owned responsibilities. |
| Graph/topology, route time, capacity, and travel Energy | Implemented for traders across authored markets. | **Reshape.** G22.4 retains non-teleporting movement with capacity, time, and Energy cost; geography and actors must be decoupled. | 3–5 | Keep exact route arithmetic fixtures. Anti-strand applies only to actual automated logistics and requires a non-vacuous setup. |

### Application and TUI surfaces

| Responsibility and evidence | Coupling today | Decision and rationale | Owner | Test disposition / bounded follow-up |
| --- | --- | --- | --- | --- |
| Typed request boundary, asynchronous owner, immutable frontend view snapshots | The boundary currently exposes market/trader nouns but keeps rules out of the TUI. | **Keep.** Headless simulation and frontend independence remain architecture contracts. | 3–7 | Preserve boundary tests that prove crate composition and no TUI/ECS leakage; view/request types may change with gameplay. |
| Player-trader location/cargo, `CommitTrade`, local trade and destination comparison — `crates/game-app/src/lib.rs:77-129,370-398,666-727` | Primary player flow is trading and traveling among markets. | **Replace.** G17 and G21 require origin-community governance and expansion rather than an independent trader role. | 5, 7 | Retire multi-hop profit acceptance after Stage 5 supplies origin startup/command coverage. Do not require UI compatibility. |
| All-system market inspection and read-only autonomous markets — `SystemInspectionView` at `crates/game-app/src/lib.rs:360-369` | Every location is presented as a market with population, prices, and activity. | **Replace.** Dead geography and layered discovery cannot be represented truthfully as read-only markets. | 3, 5 | Stage 5 tests that non-origin locations do not instantiate hidden living markets; detailed information-layer UX belongs to later gameplay slices. |
| Governance, Energy logistics, population and event views — `crates/game-app/src/lib.rs:347-369,448-555` | Useful responsibilities are entangled with market policy and commercial contracts. | **Reshape.** Governance and physical resource pressure are central, while market targets/contracts and universal population are prototype forms. | 3, 5, 7 | Keep typed-boundary/view-projection patterns. New exact view tests follow accepted origin/community commands; do not freeze current rows. |
| Six-activity TUI navigation and market-focused screens — `crates/game-tui/src/`; current `README.md` controls | Systems, Trade, Logistics, Governance, Intelligence, and Encyclopedia assume trader/market gameplay. | **Replace.** The future activity model must follow scouting, reclamation, community management, expansion, and information limits. | 5, 7 | Preserve generic input-to-intent and `TestBackend` patterns, not current tabs or labels. Stage 5 defines only cutover-critical UI. |

### Tests and diagnostics

| Test or diagnostic family and evidence | Coupling today | Decision and rationale | Owner | Test disposition / exact oracle and applicability follow-up |
| --- | --- | --- | --- | --- |
| Checked arithmetic, transaction conservation, overflow rejection, validate-before-mutate, stable ordering | Mostly small core fixtures despite market/trader terminology. | **Keep.** These are named durable engine contracts. | 2 onward | Tier 1 exact oracle: all affected values and events are unchanged on rejection; accepted deltas reconcile exactly. Applicability: every advertised atomic mutation. |
| Recipe/source arithmetic, seasonal ticks, brownout transitions, population hysteresis, route cost | Focused mechanisms sometimes load repository content or use broad fixtures. | **Reshape.** Mechanisms are candidates, but current market balance and thresholds are not contracts. | 2–3 | Move surviving behavior to 3–6-location hand-computable fixtures. Exact oracle: expected state/event at named ticks. Open: which mechanisms remain after G19 re-derivation? |
| Source-aware content validation, duplicate IDs/references, invalid values, aggregated failures | Reusable schema behavior coexists with exact-20 and market graph rules. | **Keep.** Provenance and typed validation remain durable. | 2–4 | Tier 1 exact oracle: deterministic diagnostics include source/ID/field and all independent failures. Applicability: every authored input schema retained. Split out obsolete semantic rules. |
| `repository_content_loads_with_structural_roles` — `crates/game-content/src/lib.rs:1935-2015` | Freezes counts, system IDs, fleet sizes, trader policy, and exact authored balances. | **Remove.** It is a mutable repository-world benchmark, not a small mechanism fixture. | 2, 7 | Preserve only separately extracted durable schema/arithmetic checks. No destination oracle should mention 20 systems, system 14/15, or nine NPCs. |
| `repository_economy_short_smoke_is_deterministic_and_active` — `crates/game-content/src/lib.rs:2686-2710` | Requires bought/sold and produced events in the authored world after 50 ticks. | **Remove.** Trade churn and repository-world activity are obsolete quality signals. | 2, 7 | Determinism moves to an exact fixture or generated invariant with full generation identity; production mechanism activity must be non-vacuously constructed, not inferred from this world. |
| Ignored 1,000-tick repository acceptance — `crates/game-content/src/lib.rs:2461-2675` | Requires solvency, contracts, activity after tick 300, fleet motion, population behavior, and exact replay. | **Remove.** Most oracles are authored ecology/metastability gates. | 2, 7 | Split exact reconciliation/determinism into named checks; classify event/activity summaries descriptive. Delete remaining quality thresholds with obsolete systems. |
| Multi-hop player trade acceptance — `crates/game-cli/tests/boundaries.rs:19-89` | Requires connected authored markets, player cargo/travel, trades, and positive sales revenue. | **Replace.** It proves obsolete player flow, while the crate-composition boundary in the same file remains useful. | 2, 5 | Keep the boundary test; Stage 5 replaces the flow with origin startup and one accepted governance/community request once those contracts exist. |
| Metastability validator tests — `metastability_rejects_*` and `metastability_accepts_*` in `crates/game-cli/src/main.rs:2257-2299` | Tests encode extinction, monotonic-decline, and final-stability quality bars. | **Remove.** Local collapse and population shape are texture, not named invariants or G18 guarantees. | 2, 7 | Delete with the validator gates after any independent summary formatting coverage is separated. No replacement quality threshold. |
| Repository-bound insertion permutation test — `short_system_only_and_trader_only_permutations_match_key_outcomes` in `crates/game-cli/src/main.rs:2300-2332` | Deterministic outcomes are demonstrated through the authored system/trader world. | **Reshape.** Deterministic ordering survives, but the repository fixture and trader ecology do not. | 2 | Move to a small non-vacuous Tier 1 fixture or named generated invariant. Exact oracle: order-equivalent insertion produces identical retained state/events under a defined applicability rule. |
| Reconciliation formatter and rejection tests — `crates/game-cli/src/main.rs:2411-2521` | Focused tests prove exact flow reporting and rejection of mismatched/overflowing totals. | **Keep.** They exercise G22.5 independently of metastability. | 2 onward | Exact oracle: report difference is zero for valid flow; mismatches and total-calculation overflow are rejected. Applicability: every physical-resource run. |
| Player-impact divergence plus reconciliation test — `crates/game-cli/src/main.rs:2522-2570` | One test mixes an obsolete required stage/population divergence with durable baseline/intervention reconciliation. | **Reshape.** The test must be split by responsibility rather than retained as one authored-world acceptance. | 2, 7 | Remove the divergence oracle and probe flow; extract a focused **Keep** fixture whose exact oracle accounts for external inflow and reconciles both sessions. |
| Metastability, activity, and population gates in `validate_metastability` — `crates/game-cli/src/main.rs:1122-1180` | Validation fails on extinction, decline, missing activity, ratchets, and fleet/contract behavior. | **Remove.** These are obsolete authored-world quality gates. | 2, 7 | Texture values may remain only as descriptive output; none may return a quality failure. |
| `SoakSummary` texture fields — `crates/game-cli/src/main.rs:570-630` | One summary combines useful observations with pass/fail inputs and exact reconciliation. | **Reshape.** Future diagnostics can report generated-world texture without acceptance semantics. | 2, 6 | Split exact reconciliation into the kept invariant path; retain only useful descriptive fields with full generation/replay identity. |
| Player-impact probe and required divergence | Requires a tuned intervention to produce stage/population divergence. | **Remove.** A specific authored-world response is neither a named invariant nor constructive guarantee. | 2, 7 | Its exact external-inflow accounting is retained only through the separate reconciliation fixture above. No repository-scale divergence gate. |

### CI, documentation, and historical evidence

| Responsibility and evidence | Coupling today | Decision and rationale | Owner | Test disposition / bounded follow-up |
| --- | --- | --- | --- | --- |
| Formatting, check, clippy, and workspace tests — `.github/workflows/ci.yml:15-22` | General code-quality gates also run authored-world tests in the workspace suite. | **Keep.** Tooling gates remain valid; individual obsolete tests must be reclassified. | 2 onward | Keep commands. Stage 2 changes test membership only after replacement coverage is explicit. |
| Content validation and headless acceptance CI — `.github/workflows/ci.yml:23-26` | Both commands instantiate and validate the authored market universe. | **Replace.** CI should eventually validate current content/generation and truthful headless startup, not legacy world quality. | 2, 5–6 | Do not change CI before new exact oracles exist. Stage 5 owns command cutover; Stage 6 adds only named generated invariants. |
| README runtime, designer configuration, and diagnostic guidance | Presents trader-first market behavior and metastability commands as active product surface. | **Reshape.** Operational instructions stay truthful while the runnable prototype exists, but are explicitly labeled legacy and non-authoritative. | 1, then 5–7 | Stage 5 rewrites startup/player guidance; Stage 7 removes retired commands. Documentation link checks remain part of every stage. |
| `archive/market-trading-prototype/` and `archive/README.md` | Preserves former plans, specs, evidence, and captures. | **Keep.** Historical evidence supports code archaeology but has no authority or compatibility force. | 1 onward | Keep archive labels and current-direction links accurate. Do not run archived acceptance criteria as current gates. |
| Contributor and architecture guidance — `AGENTS.md`, `docs/architecture.md` | Previously lacked the complete generated-world failure policy. | **Keep.** These are the durable entry points for preventing accidental authored-world tuning. | 1 onward | Review later plans against both documents. Generator range changes and new invariants require explicit, reviewed contracts. |

## Stage 2 test-development backlog

Stage 1 changes no tests. Stage 2 should use the inventory above to produce an
explicit registry rather than deleting broad test modules at once.

1. Label every current family **Tier 1 mechanism**, **candidate named
   invariant**, **constructive guarantee**, **descriptive-only**, or
   **obsolete premise**.
2. For each candidate invariant, record:
   - exact oracle;
   - applicability rule;
   - fixture or generated setup proving the assertion is non-vacuous;
   - failure output and replay identity.
3. Preserve checked arithmetic, deterministic ordering, atomic rejection,
   exact reconciliation, stable IDs, and source-aware validation in focused
   fixtures even when their current market/trader wrappers are removed.
4. Extract surviving seasonal, population, brownout, production, route, and
   logistics mechanisms only into small fixtures with known outcomes; their
   current balance constants are not presumed durable.
5. Treat repository-content counts, ongoing trade, NPC profitability, universal
   survival, aggregate metastability, population ratchets, and required
   player-impact divergence as obsolete premises.
6. Keep deterministic seed corpora as regression inputs only. Never introduce
   pass percentages, reject/reroll screening, or statistical world-quality
   thresholds.
7. Turn real generated failure classes into retained Tier 1 reproductions when
   the mechanism can be isolated and hand-computed.

G18 constructive guarantee tests cannot be implemented until Stage 4 defines
exact units, values, inequalities, generation configuration, and replay
identity. Stage 2 may reserve registry entries but must not invent those
contracts.

## Stage 1 boundary

This audit deliberately performs no destination implementation:

- no Rust or RON edits;
- no test deletion, fixture extraction, or changed assertions;
- no CI gate or command changes;
- no world-model or generator schema design;
- no generated-world selection or startup cutover;
- no archival or deletion of the runnable prototype;
- no compatibility layer for obsolete gameplay.

## Open questions assigned to later stages

- **Stage 2:** Which current focused tests are genuinely non-vacuous named
  invariants, and what canonical names, exact oracles, and applicability rules
  belong in the invariant registry?
- **Stage 3:** What is the minimum geography/community substrate that represents
  dead locations, one living origin, extractable resources, and minimally typed
  reclaimable sites without prematurely designing bodies, slots, ruin internals,
  surveys, or information layers?
- **Stage 4:** What map connectivity rule, origin surplus margin, starting
  range, extractable-resource floor, and reclaimable-site placement make both
  G18 guarantees exact and constructive?
- **Stage 5:** What complete generation identity selects normal play and replay,
  and what is the smallest truthful origin-first app/TUI startup flow?
- **Stage 6:** Which retained automated logistics exists to make anti-strand or
  liveness checks applicable and non-vacuous, and which texture summaries are
  useful without becoming gates?
- **Stage 7:** Which market/economy types still serve a demonstrated community
  or player-owned-logistics responsibility after cutover, and which can be
  deleted outright?

## Stage 1 completion evidence

- Contributor policy: `AGENTS.md`
- Architecture testing boundary: `docs/architecture.md`
- Executable/target distinction: `README.md`
- Transition source of truth:
  `docs/2026-07-20-testing-stance-correction.md`
- Historical entry point: `archive/README.md`
- Obsolete todo link correction:
  `todos/007-complete-p1-slice-2-world-dynamics-population-and-player-progression.md`

Implementation and CI behavior are intentionally unchanged. Each later stage
must re-read the cited code before acting because line numbers and coupling can
change after this audit date.
