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

At the time of this Stage 1 record, the executable remained unchanged. The
current implementation status is maintained in the
[Testing Stance and Constructive Worldgen transition](2026-07-20-testing-stance-correction.md).
Stage 3 subsequently completed on 2026-07-20; the original decisions below are
preserved as historical authority rather than rewritten as if their destination
already existed during Stage 1.

All file/line citations and pre-cutover type or test names in the inventory are
historical evidence from the Stage 1 repository snapshot. They are not current
implementation pointers after the Stage 3 replacement.

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
constructive generation, Stage 5 restores startup/player identity, Stage 6
owns generated-world invariant/replay tooling, and Stage 7 verifies that
Stages 2–3 completed retirement.

## Implementation status after Stage 3

Stage 3 completed the destructive substrate cutover on 2026-07-20. The current
workspace contains only `game-core` and `game-content`; its 15 focused tests
(nine core and six content) have no ignored tests. The runtime now represents
stable resources and locations, exactly one living origin community, physical
stocks and deposits, reclaimable sites, and explicit topology without markets.
The app, TUI, CLI, production authored content, trader/fleet ecology, pricing,
wallets, commercial contracts, and legacy acceptance surface are absent.
Stages 4–8 remain future work, so this boundary is intentionally headless and
non-playable.

## Migration inventory

### Authored content

| Responsibility and evidence | Coupling today | Decision and rationale | Owner | Test disposition / bounded follow-up |
| --- | --- | --- | --- | --- |
| Authored system list and coordinates — `content/systems.ron`; `SystemSource` in `crates/game-content/src/lib.rs:83-93` | The repository universe is a fixed list of 20 named systems. | **Replace.** G17–G18 require generated dead geography with one constructed living origin, not a canonical populated universe. | 3–5 | Stage 2 may keep only deliberately small coordinates as fixtures. Stage 4 must define generation identity and map-scale constraints before replacement. |
| Per-system economy records — `content/economy.ron`; `EconomySource` in `crates/game-content/src/lib.rs:140-155` | Every authored system receives inventory, targets, production, population, policy, and optional governance. | **Replace.** Empty locations must not silently become live economies; location data and community data need separate composition. | 3–5 | Preserve no repository-world numeric oracle. Which minimum community configuration serves G18 belongs to Stage 4. |
| Resource-pressure tuning — brownout, population, production, Energy logistics, and investment fields in `content/economy_config.ron`; `EconomyConfigSource` in `crates/game-content/src/lib.rs:157-264` | Mechanism parameters are tuned against every authored market and share configuration with obsolete ecology policy. | **Reshape.** G19 and G22 preserve only re-derived community pressure, life-support, production, physical movement, and expansion-facing investment responsibilities. | 2–4b | Extract small mechanism fixtures before schema changes. Stage 3 decides configuration boundaries; Stage 4 owns authored origin-engine tuning and Stage 4b owns generator parameters. |
| Market and fleet tuning — market policy, adaptive NPC response, retirement, and repositioning fields in `content/economy_config.ron` | These values exist to stabilize or animate autonomous market/trader ecology. | **Remove.** G19 and G21 reject that ecology and its quality targets. | 2–3 | Keep no current threshold oracle. Delete fields with their consumers after retaining only cheap independent arithmetic coverage. |
| Player-trader identity configuration — the player branch of `content/traders.ron`; `TraderConfigSource` in `crates/game-content/src/lib.rs:318-350` | Normal play starts as one specially flagged trader. | **Remove, then replace later.** G17 starts the player as an origin community/governor rather than an independent trader. | 2–3, then 5 | Delete the trader identity once retained movement/accounting is isolated. Stage 5 introduces origin-first startup independently; it is not a prerequisite for deletion. |
| Independent NPC fleet configuration — NPC/archetype/fleet branches of `content/traders.ron` | Nine initial NPCs and adaptive archetypes populate the authored market network. | **Remove.** G19 strikes independent NPC fleet ecology; future delegated logistics is newly specified player-owned behavior. | 2–3 | Keep no fleet-count, profitability, spawn, or retirement oracle; delete configuration with its consumers. |
| Movement capacity, time, and Energy-cost parameters in `content/traders.ron` | Durable transport responsibilities are encoded inside obsolete trader definitions. | **Reshape.** G22.4 preserves non-teleporting physical movement, not the current actor model. | 2, 3, 5 | Retain exact route/capacity/Energy arithmetic fixtures where applicable; Stage 3–5 decide the destination owner. |
| Wallet, ordinary cargo-for-profit, and commercial policy in `content/traders.ron` | These fields support Energy-denominated market trading and commercial contracts. | **Remove.** G22 does not preserve Energy as universal money, wallets, or commercial delivery policy. | 2–3 | Delete fields and consumers after retaining only directly useful checked-transfer arithmetic. |
| Goods and recipes — `content/goods.ron`, `content/recipes.ron` | Prices and Energy-denominated production recipes serve trading among autonomous markets. | **Reshape.** G20–G22 retain physical goods, Energy/input costs, and a resource chain, but require new expansion-facing content and do not retain prices as a design contract. | 3–4, 7 | Preserve checked recipe arithmetic fixtures. Future content work must decide goods and recipes independently of the migration audit. |
| Player-facing manual — `content/encyclopedia.ron` | Articles explain the current market, trade, Energy-contract, fleet, and governance prototype. | **Remove, then replace later.** Player-facing truth must follow the origin-and-expansion game after its mechanics exist. | 2–3, then 5 | Delete obsolete articles with their mechanics. Stage 5 writes a new minimum truthful manual. |

### Content pipeline

| Responsibility and evidence | Coupling today | Decision and rationale | Owner | Test disposition / bounded follow-up |
| --- | --- | --- | --- | --- |
| RON loading, typed conversion, stable IDs, aggregated source-aware errors — `load`, `compile`, and diagnostics in `crates/game-content/src/lib.rs:412-498` | Reusable validation is implemented alongside market-specific compilation. | **Keep.** Stable IDs, typed values, and useful provenance remain durable architecture contracts. | 3–4 | Stage 2 keeps invalid-value, duplicate-ID, unresolved-reference, and aggregated-error tests. Stage 3 identifies a reusable validator boundary without adding a new crate absent concrete need. |
| Exactly 20 systems — `crates/game-content/src/lib.rs:499-514` | Compilation rejects any other universe size. | **Remove.** Neither Tier 1 fixtures nor generated worlds have this cardinality contract. | 2 | Remove the rule and cardinality-gated graph path so Stage 2 can compile small fixtures; no geography redesign is required. |
| One market/economy record per system — `MarketSource` and compile checks in `crates/game-content/src/lib.rs:266-316,881-930` | Geography cannot exist without a live market and its economic state. | **Replace.** G17 requires dead locations and a separately instantiated origin community. | 3 | Add focused content tests for locations without communities only when Stage 3 defines that schema. |
| Graph construction, finite coordinates, connectivity and route energy — `SystemGraph` use and graph checks in `crates/game-content/src/lib.rs`; `crates/game-core/src/lib.rs` | Topology is compiled from systems but consumed primarily for trader routes and market connectivity. | **Reshape.** Spatial topology remains useful, while universal connectivity and trader-route assumptions require re-derivation against G10 and exploration. | 3–4b | Preserve finite-coordinate and arithmetic tests where intentional. Stage 4b must decide whether generated geography is connected, partially connected, or navigable through another rule. |
| Bootstrap solvency/runway and protected-liquidation checks — `crates/game-content/src/lib.rs:1760-1925,2710-2790` | Validation guarantees market import/runway and trader liquidation behavior in the authored ecology. | **Remove.** Economic solvency is not a generated-world oracle, and trader liquidation is not retained. | 2 | Delete the legacy checks now. Stage 4 provides authored mechanism fixtures; Stage 4b derives structural placement guarantees without economic inequalities. |
| Trader/fleet compilation and archetype distribution — `crates/game-content/src/lib.rs:931-1210` | Successful content always instantiates player/NPC traders and fleet policy. | **Remove.** Independent NPC ecology and player-trader identity are obsolete. | 2–3 | Delete compilation with the legacy schema; player-owned automation gets new fixtures only after a gameplay contract exists. |

### Runtime startup and CLI modes

| Responsibility and evidence | Coupling today | Decision and rationale | Owner | Test disposition / bounded follow-up |
| --- | --- | --- | --- | --- |
| Repository-directory load before mode selection — `main` in `crates/game-cli/src/main.rs:15-35` and `content_path` at `:210-214` | TUI, validation, headless, diagnostics, and probes all begin from the same authored definition. | **Remove, then replace later.** Normal play and generated-world tooling need new composition rather than a hidden canonical universe. | 2–3, then 5 | Delete the legacy load/startup path without a bridge. Stage 5 independently defines new selection and replay contracts. |
| TUI default startup — `ExecutionMode::Tui` in `crates/game-cli/src/main.rs:104-111` | Default play creates a player trader in populated markets. | **Remove, then replace later.** G17 starts the player as the origin community/governor. | 2–3, then 5 | Interim playability is not required. Stage 5 adds origin-first startup as a new surface. |
| `--validate-content` — `ExecutionMode::ValidateContent` in `crates/game-cli/src/main.rs:37` | Validates the authored market bundle and its market/fleet-specific rules. | **Remove, then replace later.** Reusable source-aware validation survives in focused tests, not this command. | 2, then 4–5 | Delete the legacy command/CI step. Add a new generator/configuration validator only when its inputs exist. |
| `--headless` authored acceptance — `ExecutionMode::Headless` in `crates/game-cli/src/main.rs:73-103`; `.github/workflows/ci.yml:25-26` | A successful headless run proves current repository startup and activity assumptions. | **Remove, then replace later.** Frontend-independent simulation is durable; this authored startup is not. | 2, then 5–6 | Delete the command/CI gate without replacement. Stage 5 introduces a new headless boundary from the new startup contract. |
| Exact reconciliation in headless and diagnostic modes — `reconcile_energy` call sites in `crates/game-cli/src/main.rs:101-102,488-492,1353-1355,1664-1666` | Durable accounting evidence is embedded in modes that also inspect authored-world quality. | **Keep.** G22.5 requires exact physical-resource reconciliation independent of current markets or probes. | 2 onward | Exact oracle: expected equals actual with every physical transfer channel accounted for. Applicability: every run that mutates physical resources. |
| Descriptive economy/world texture reporting — `ExecutionMode::EconomyDiagnostics` in `crates/game-cli/src/main.rs:55-72` | Current reports summarize authored markets, fleet activity, population, and Energy flow. | **Remove, then replace later if useful.** The current report describes a deleted model. | 2, then 6 | Delete it rather than adapting it. Stage 6 may create new non-gating summaries from generated-world needs. |
| Pricing comparison, player-impact divergence, and metastability/market-activity gates — `crates/game-cli/src/main.rs:38-54,65,1122-1180` | These modes and validators judge obsolete pricing, intervention response, survival, and market ecology. | **Remove.** None is a named engine invariant or constructive G18 guarantee. | 2 | Preserve cheap exact inflow/reconciliation fixtures separately; delete the modes and gates without replacement. |

### Core data model and simulation responsibilities

| Responsibility and evidence | Coupling today | Decision and rationale | Owner | Test disposition / bounded follow-up |
| --- | --- | --- | --- | --- |
| Stable IDs, deterministic schedule/order, checked integer Energy and validate-before-mutate — `crates/game-core/src/lib.rs`; [atomicity guidance](solutions/rust-ecs-validate-before-mutate.md) | These contracts currently operate through market, recipe, trader, and logistics systems. | **Keep.** They are explicit cross-stage architecture contracts and implement G22.5 independent of current gameplay nouns. | 2–7 | Preserve exact overflow, rejection atomicity, conservation, deterministic-order, and reconciliation fixtures; rewrite fixtures only when obsolete types are removed. |
| `SystemDefinition`, `Market`, and `CoreSnapshot.markets` — `crates/game-core/src/lib.rs:663-724,1624-1682,2498-2514` | A “system” embeds live economy inputs and the snapshot presents markets as the world. | **Replace.** G17 needs locations without living economic actors and separately composed communities. | 3 | Stage 3 must define only the minimum truthful substrate: dead locations, one living origin, extractable resources, and minimally typed reclaimable sites. Bodies, slots, ruin internals, surveys, and full information design remain out of scope. |
| Player-trader identity and special player flag — `TraderDefinition` and `Trader` in `crates/game-core/src/lib.rs:725-759,1838-1860` | Player startup is represented as one trader among autonomous market actors. | **Remove, then replace later.** G17 requires origin-community/governor identity. | 2–3, then 5 | Delete the old identity once retained movement/accounting is isolated. Stage 5 adds origin identity without sequencing deletion on replacement coverage. |
| Independent NPC trader entities and ecology — the same trader types plus `FleetMode` at `crates/game-core/src/lib.rs:525-582` | Autonomous merchants own wallet, cargo, travel, and profitability state. | **Remove.** G19 and G21 reject independent NPC market ecology. | 2–3 | Keep no entity-lifecycle or profitability contract solely for compatibility. |
| Trader movement capacity, time, travel Energy, and physical carriage | Durable movement responsibilities live inside an obsolete actor type. | **Reshape.** G22.4 keeps capacity-, time-, and Energy-costed physical movement. | 2–5 | Preserve exact route, capacity, loading, and burn arithmetic only where applicable to accepted logistics. |
| Trader wallets, commercial cargo/profit, and reservation state | These fields implement Energy-denominated market exchange. | **Remove.** G22 explicitly does not preserve universal money, wallets, or commercial reservation policy. | 2–3 | Delete with market exchange; retain generic transaction arithmetic only where another current responsibility consumes it. |
| Exact physical-resource accounting and ledgers | Reconciliation is implemented through current market, trader, and contract channels. | **Keep.** Checked deterministic arithmetic and exact reconciliation are G22.5 contracts regardless of destination ownership. | 2 onward | Tier 1 exact oracle: all sources, sinks, stores, and in-flight amounts reconcile, including rejected mutations. |
| Energy generation, storage, life-support burn, production, transport, sinks, and player-facing resource pressure | Current behavior is instantiated on every market and uses commercial contracts for movement. | **Reshape.** G22 preserves these responsibilities for living communities and expansion, not as universal money or market ecology. | 2–5 | Stage 3 decides which state belongs to communities; later gameplay slices define expansion spending and player-owned logistics. |
| Brownout ladder, population hysteresis, seasonal arithmetic, investments | These mechanisms are tuned to all authored markets and metastability. | **Reshape.** G19 says they may return only when re-derived for origin/daughter communities and player-facing pressure. | 2–3, 7 | Stage 2 can retain hand-computable mechanism fixtures without retaining current thresholds as product balance. Which mechanisms survive is a later design decision. |
| Market prices, bids/asks, wallets, reservations, ordinary trade and commercial Energy contracts | They coordinate autonomous market exchange and trader profit. | **Remove.** G21–G22 explicitly deny these a presumption of survival; physical transport does not require commercial-market semantics. | 2–3 | Preserve only generic checked transfer/transaction contracts already consumed by another retained responsibility. Otherwise delete code and tests together. |
| Fleet spawn/retire ecology and profitability/opportunity response — `FleetMode` at `crates/game-core/src/lib.rs:525-582` | Autonomous traders adapt to unmet profitable market opportunity. | **Remove.** G19 strikes the NPC ecology. Future delegated fleets are player-owned and require a new contract. | 2–3 | Delete thresholds and behavior; future player-owned automation starts from a new contract. |
| Graph/topology, route time, capacity, and travel Energy | Implemented for traders across authored markets. | **Reshape.** G22.4 retains non-teleporting movement with capacity, time, and Energy cost; geography and actors must be decoupled. | 3–5 | Keep exact route arithmetic fixtures. Anti-strand applies only to actual automated logistics and requires a non-vacuous setup. |

### Application and TUI surfaces

| Responsibility and evidence | Coupling today | Decision and rationale | Owner | Test disposition / bounded follow-up |
| --- | --- | --- | --- | --- |
| Typed request boundary, asynchronous owner, immutable frontend view snapshots | The boundary currently exposes market/trader nouns but keeps rules out of the TUI. | **Keep only the pattern.** Frontend independence remains an architecture contract, but current request/view types and composition tests need not survive. | 2–5 | Retain code/tests only when they compile without compatibility work; Stage 5 may rebuild the boundary around new commands. |
| Player-trader location/cargo, `CommitTrade`, local trade and destination comparison — `crates/game-app/src/lib.rs:77-129,370-398,666-727` | Primary player flow is trading and traveling among markets. | **Remove, then replace later.** G17 and G21 require origin-community governance and expansion rather than an independent trader role. | 2–3, then 5 | Delete flow and acceptance without waiting for origin startup. Do not require UI compatibility. |
| All-system market inspection and read-only autonomous markets — `SystemInspectionView` at `crates/game-app/src/lib.rs:360-369` | Every location is presented as a market with population, prices, and activity. | **Replace.** Dead geography and layered discovery cannot be represented truthfully as read-only markets. | 3, 5 | Stage 5 tests that non-origin locations do not instantiate hidden living markets; detailed information-layer UX belongs to later gameplay slices. |
| Governance, Energy logistics, population and event views — `crates/game-app/src/lib.rs:347-369,448-555` | Useful responsibilities are entangled with market policy and commercial contracts. | **Remove current forms; reuse patterns later.** Governance and physical resource pressure are central, while these market rows are prototype forms. | 2–3, then 5 | Delete current views when entanglement makes retention costly. New exact views follow accepted origin/community commands. |
| Six-activity TUI navigation and market-focused screens — `crates/game-tui/src/`; current `README.md` controls | Systems, Trade, Logistics, Governance, Intelligence, and Encyclopedia assume trader/market gameplay. | **Remove, then replace later.** The future activity model follows scouting, reclamation, community management, expansion, and information limits. | 2–3, then 5 | Delete current screens/tests without a compatibility shell. Stage 5 builds only the new cutover-critical UI. |

### Tests and diagnostics

| Test or diagnostic family and evidence | Coupling today | Decision and rationale | Owner | Test disposition / exact oracle and applicability follow-up |
| --- | --- | --- | --- | --- |
| Checked arithmetic, transaction conservation, overflow rejection, validate-before-mutate, stable ordering | Mostly small core fixtures despite market/trader terminology. | **Keep.** These are named durable engine contracts. | 2 onward | Tier 1 exact oracle: all affected values and events are unchanged on rejection; accepted deltas reconcile exactly. Applicability: every advertised atomic mutation. |
| Recipe/source arithmetic, seasonal ticks, brownout transitions, population hysteresis, route cost | Focused mechanisms sometimes load repository content or use broad fixtures. | **Reshape.** Mechanisms are candidates, but current market balance and thresholds are not contracts. | 2–3 | Move surviving behavior to 3–6-location hand-computable fixtures. Exact oracle: expected state/event at named ticks. Open: which mechanisms remain after G19 re-derivation? |
| Source-aware content validation, duplicate IDs/references, invalid values, aggregated failures | Reusable schema behavior coexists with exact-20 and market graph rules. | **Keep.** Provenance and typed validation remain durable. | 2–4 | Tier 1 exact oracle: deterministic diagnostics include source/ID/field and all independent failures. Applicability: every authored input schema retained. Split out obsolete semantic rules. |
| `repository_content_loads_with_structural_roles` — `crates/game-content/src/lib.rs:1935-2015` | Freezes counts, system IDs, fleet sizes, trader policy, and exact authored balances. | **Remove.** It is a mutable repository-world benchmark, not a small mechanism fixture. | 2 | Preserve only cheap, separately useful schema/arithmetic checks. No destination oracle should mention 20 systems, system 14/15, or nine NPCs. |
| `repository_economy_short_smoke_is_deterministic_and_active` — `crates/game-content/src/lib.rs:2686-2710` | Requires bought/sold and produced events in the authored world after 50 ticks. | **Remove.** Trade churn and repository-world activity are obsolete quality signals. | 2 | Delete without replacement; existing focused deterministic tests own any retained ordering contract. |
| Ignored 1,000-tick repository acceptance — `crates/game-content/src/lib.rs:2461-2675` | Requires solvency, contracts, activity after tick 300, fleet motion, population behavior, and exact replay. | **Remove.** Most oracles are authored ecology/metastability gates. | 2 | Retain only cheap independent reconciliation/determinism tests, then delete the soak and all summaries. |
| Multi-hop player trade acceptance — `crates/game-cli/tests/boundaries.rs:19-89` | Requires connected authored markets, player cargo/travel, trades, and positive sales revenue. | **Remove.** It proves obsolete player flow. | 2 | Delete without replacement. Keep the neighboring composition test only if retained APIs still support it without compatibility work; Stage 5 adds a new origin flow. |
| Metastability validator tests — `metastability_rejects_*` and `metastability_accepts_*` in `crates/game-cli/src/main.rs:2257-2299` | Tests encode extinction, monotonic-decline, and final-stability quality bars. | **Remove.** Local collapse and population shape are texture, not named invariants or G18 guarantees. | 2 | Delete with the validator and summary; no formatting replacement or quality threshold is needed. |
| Repository-bound insertion permutation test — `short_system_only_and_trader_only_permutations_match_key_outcomes` in `crates/game-cli/src/main.rs:2300-2332` | Deterministic outcomes are demonstrated through the authored system/trader world. | **Reshape.** Deterministic ordering survives, but the repository fixture and trader ecology do not. | 2 | Move to a small non-vacuous Tier 1 fixture or named generated invariant. Exact oracle: order-equivalent insertion produces identical retained state/events under a defined applicability rule. |
| Reconciliation formatter and rejection tests — `crates/game-cli/src/main.rs:2411-2521` | Focused tests prove exact flow reporting and rejection of mismatched/overflowing totals. | **Keep.** They exercise G22.5 independently of metastability. | 2 onward | Exact oracle: report difference is zero for valid flow; mismatches and total-calculation overflow are rejected. Applicability: every physical-resource run. |
| Player-impact divergence plus reconciliation test — `crates/game-cli/src/main.rs:2522-2570` | One test mixes an obsolete required stage/population divergence with durable baseline/intervention reconciliation. | **Remove probe; retain cheap accounting evidence.** | 2 | Delete divergence and probe flow; use an existing or small focused external-inflow reconciliation fixture without preserving CLI reporting. |
| Metastability, activity, and population gates in `validate_metastability` — `crates/game-cli/src/main.rs:1122-1180` | Validation fails on extinction, decline, missing activity, ratchets, and fleet/contract behavior. | **Remove.** These are obsolete authored-world quality gates. | 2 | Delete validator, summary, and output; future texture diagnostics start from generated-world needs. |
| `SoakSummary` texture fields — `crates/game-cli/src/main.rs:570-630` | One summary describes the authored market model and mixes observations with acceptance inputs. | **Remove; redesign later if needed.** | 2, then 6 | Delete the summary after retaining focused reconciliation. Stage 6 starts new diagnostics from generated-world questions rather than these fields. |
| Player-impact probe and required divergence | Requires a tuned intervention to produce stage/population divergence. | **Remove.** A specific authored-world response is neither a named invariant nor constructive guarantee. | 2 | Delete without replacement; retain external-inflow accounting only through a focused invariant test. |

### CI, documentation, and historical evidence

| Responsibility and evidence | Coupling today | Decision and rationale | Owner | Test disposition / bounded follow-up |
| --- | --- | --- | --- | --- |
| Formatting, check, clippy, and workspace tests — `.github/workflows/ci.yml:15-22` | General code-quality gates also run authored-world tests in the workspace suite. | **Keep for retained code.** Individual obsolete tests are deleted. | 2 onward | Keep commands for the retained workspace; no one-for-one replacement coverage is required. |
| Content validation and headless acceptance CI — `.github/workflows/ci.yml:23-26` | Both commands instantiate and validate the authored market universe. | **Remove, then replace later.** They are legacy product acceptance, not migration safety. | 2, then 5–6 | Stage 2 may delete both CI steps without replacement. Stage 5/6 add truthful startup and generated-invariant gates when those surfaces exist. |
| README runtime, designer configuration, and diagnostic guidance | Presents trader-first market behavior and metastability commands as active product surface. | **Remove with implementation.** Current instructions must disappear when their code/content disappears; interim playability is not promised. | 2–3, then 5 | During the non-playable interval, document migration status rather than preserving operational instructions. Stage 5 writes new startup guidance. |
| `archive/market-trading-prototype/` and `archive/README.md` | Duplicates former plans, specs, evidence, and captures in the working tree. | **Remove.** Git history is sufficient for archaeology; a migration archive creates another surface to curate. | 2–3 | Do not quarantine deleted implementation/content/docs. Remove legacy archive material after current links no longer depend on it. |
| Contributor and architecture guidance — `AGENTS.md`, `docs/architecture.md` | Previously lacked the complete generated-world failure policy. | **Keep.** These are the durable entry points for preventing accidental authored-world tuning. | 1 onward | Review later plans against both documents. Generator range changes and new invariants require explicit, reviewed contracts. |

## Stage 2 test-development backlog (historical)

This backlog records the Stage 1 handoff to Stage 2; it is not a current todo
list after Stages 2–3 completion.

Stage 1 changes no tests. Stage 2 should use the inventory above to identify
focused retained evidence before deleting obsolete modules. This is not a
one-for-one replacement exercise: delete broad tests and their implementation
when no retained responsibility justifies them.

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

## Migration execution correction

Stage owner numbers record dependency context, not a requirement to keep the
legacy game operational until the latest listed stage. Stages 2–3 should perform
most deletion once retained low-level responsibilities are isolated. The
workspace must remain buildable around retained contracts, but normal startup,
headless play, authored content, diagnostics, TUI flows, and legacy acceptance
may be absent until Stage 5 restores a truthful executable. Do not archive
removed source/content/tests/UI; use Git history.

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
- **Stage 3 (closed 2026-07-20):** The minimum substrate is stable resources and
  finite locations; exactly one origin community with nonzero population and
  physical stocks; separately referenced nonzero deposits and minimally typed
  reclaimable sites; and explicit normalized topology that permits empty or
  disconnected graphs. Bodies, slots, site internals, surveys, information
  layers, and generation guarantees remain deferred.
- **Stage 4:** What exact authored origin resource/infrastructure engine makes
  bank/develop pressure hand-computable without population mutation, scouting,
  or generated-world claims?
- **Stage 4b:** Which structural origin/neighborhood records, topology rules,
  generation identity, and bounded outward action make G18 constructive without
  economic inequalities, quantity floors, or assumed reclaimable-site needs?
- **Stage 5:** How does complete generation identity select normal play, and
  what is the smallest truthful origin-first app/TUI startup flow?
- **Stage 6:** Which retained automated logistics exists to make anti-strand or
  liveness checks applicable and non-vacuous, and which texture summaries are
  useful without becoming gates?
- **Stage 7:** Does a final working-tree and CI search prove that Stages 2–3
  deleted every unjustified market/economy/trader surface and compatibility
  copy?

## Stage 1 completion evidence

- Contributor policy: `AGENTS.md`
- Architecture testing boundary: `docs/architecture.md`
- Executable/target distinction: `README.md`
- Transition source of truth:
  `docs/2026-07-20-testing-stance-correction.md`

At Stage 1 completion, implementation and CI behavior were intentionally
unchanged. Stage 2 subsequently moved current durable contracts into the
[Engine Invariant Registry](2026-07-20-engine-invariant-registry.md) and deleted
superseded working-tree history. Later stages must re-read the cited code before
acting because line numbers and coupling can change after this audit date.

## Stage 2 completion evidence

This section is historical Stage 2 evidence. Paths and test names that Stage 3
later deleted are not current registry evidence.

- Active and reserved contracts:
  [Engine Invariant Registry](2026-07-20-engine-invariant-registry.md)
- Direct Tier 1 content fixtures and retained provenance checks:
  `crates/game-content/src/lib.rs`
- Focused reconciliation, ordering, atomicity, and identifier evidence:
  `crates/game-core/src/tests.rs` and
  `crates/game-core/src/energy_logistics/tests.rs`
- Retained CI boundary: `.github/workflows/ci.yml`
- Removed surfaces: repository acceptance and ignored soaks, legacy CLI
  diagnostics/acceptance, authored-world quality predicates, prototype archive,
  and completed prototype todos. The Stage 1 plan remains part of the active
  staged-migration record.
- Validation: formatting, workspace check, Clippy with warnings denied, and 201
  retained tests passed with zero ignored tests.

## Stage 3 completion evidence

- Accepted implementation plan:
  `docs/plans/2026-07-20-feature-origin-frontier-substrate-stage-3-plan.md`.
- Retained workspace: `game-core` and `game-content` only. `game-app`,
  `game-tui`, `game-cli`, and production `content/` are absent; no compatibility
  shells or translated authored market universe remain.
- Core substrate: `crates/game-core/src/lib.rs` defines stable resources and
  locations, one origin community, physical stores/deposits, reclaimable sites,
  explicit topology, normalized snapshots, checked Energy arithmetic, and
  atomic resource transfer accounting.
- Content substrate: `crates/game-content/src/lib.rs` compiles one source-aware
  Stage 3 RON world and aggregates deterministic diagnostics before returning a
  definition. Its fixtures are test-only Tier 1 worlds.
- Current exact registry evidence includes
  `input_permutations_produce_equal_snapshots`, `normalizes_permuted_input`,
  `energy_transfer_reconciles_exactly`,
  `resource_transfer_rejections_are_atomic_on_every_path`,
  `energy_arithmetic_is_checked`,
  `content_id_validation_and_display_are_stable`,
  `compiles_a_dead_isolated_location_and_instantiates_world_state`,
  `aggregates_exact_source_aware_diagnostics`,
  `parse_errors_include_document_provenance`,
  `unknown_fields_are_rejected_in_top_level_and_nested_sources`, and
  `location_diagnostics_are_complete_and_permutation_independent`. No legacy
  market/trader test is current evidence, and automated logistics is reserved
  because no such domain exists.
- Validation on 2026-07-20: `cargo test --workspace --all-features -- --list`
  resolved nine `game-core` and six `game-content` tests (15 total), zero doc
  tests, and no ignored test attributes in the retained crates.
