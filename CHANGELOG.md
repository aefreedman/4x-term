# Changelog

## Unreleased

### Added

- Initial Rust terminal prototype with a headless ECS simulation.
- Data-defined 20-system frontier economy, production recipes, markets, and traders.
- Player trading, multi-hop travel, economic status, asynchronous simulation controls, and Ratatui interface.
- Headless content validation and simulation commands.
- Physical energy-economy views for market stock, reserve claims, protected budgets, player tank and cargo-bay energy, route runway, and market health.
- Energy-flow reconciliation and per-market solvency diagnostics, with pricing-mode override and identical-state scarcity/cost-aware A/B runs.
- Deterministic funded partial-arrival recovery, processor structural-solvency reporting, authored physical refuel policies, and explicit normal/full/low/deficit energy displays.
- A deterministic four-stage brownout ladder with stage transitions, runway, stage-aware throughput, demand, pricing, protection, immutable app views, and textual TUI visibility.
- Validated world-dynamics content scaffolding for seasons, static/dynamic population configuration, fixed/dynamic fleets, all four investment kinds, governance, and aggregate history.
- Deterministic seasonal generation on three prototype systems, with base/effective output, phase, trend, and next-turning-point visibility in immutable app views and the TUI.
- A bounded identical-session player-impact probe with one typed, recorded external delivery and explicit reconciliation of intervention inflow.
- A deterministic endogenous NPC fleet with normalized persistent opportunity, market-funded slow spawning, bounded profitability, conservation-safe deferred retirement, typed lifecycle events, and fleet diagnostics.
- Dynamic population hysteresis with a validated 10,000-sample maximum for efficient bounded energy/goods sufficiency history, fast starvation decline, slower gated logistic recovery, atomically denominator-paired carry, population-scaled life support/labor/tertiary demand, tier milestones, and an explicit 10,000-tick metastability acceptance harness.
- A common autonomous investment executor for collectors, storage, population support, and canonically funded route subsidies, with diminishing checked costs, stable allocation ties, cooldowns, maximum levels, protected-surplus spending, typed status, and Emergency/Starvation suppression with automatic subsidy recovery.
- One authored starting player governorship plus default AI allocations, typed authorized policy/allocation requests and rejection feedback, immutable governor views, and TUI controls for reserve horizon, margin, import priorities, and autonomous investment allocations.
- Authored, progression-ready player trade-network access with offline reservation-contract rejection in the headless core, immutable application projection, and visible TUI capability state.
- A responsive, explicitly scrollable F5 Encyclopedia backed by frontend-independent factual sections and articles for systems, energy, brownouts, population, goods, markets, recipes, governance, investments, traders, reservations, travel, and trade-network access.
- Read-only selected-good destination market comparisons with player-relative route time and energy facts, plus stable-ID Trade region selection that previews routes without committing travel.

### Changed

- Replaced the all-at-once terminal dashboard with F1–F5 Systems, Trade, Governance, Intelligence, and Encyclopedia activities, contextual controls, and cell-based compact (`80x30`) and regular (`160x45`) layouts.
- Systems, Trade, and Governance tables now use deterministic selected-row viewports with position/more indicators; compact Trade prioritizes exact selected-action cargo and tank consequences, route workflow, and unavailable reason.
- Encyclopedia prose now loads from validated `content/encyclopedia.ron`, introduces mechanics in plain language before detailed terms, and avoids runtime-settings narration.
- Trade gives surplus vertical space to its scrollable local-market and destination lists while keeping action, route, and player summaries compact.
- Systems navigation wraps, selected remote markets have an explicit read-only view, Governance can jump between sections, warning markers reflect actual severity, and shortcut accents are consistent across primary surfaces.
- Trade preserves mnemonic `(S)ell`; paused single-step moved to `.` so function keys remain reserved for top-level activities.
- Route previews, active travel, direct connections, and player location now use readable system names with jump, distance, timing, and route-specific required-energy summaries instead of exposing internal content IDs.
- Event log entries now resolve system, trader, good, and production-process IDs to readable display names.
- Player cargo now displays readable good names instead of internal content IDs.
- NPC trader setup now begins with nine evenly distributed traders and uses Dynamic production mode with designer-editable archetype, response, cooldown, retirement, and maximum-count parameters in `content/traders.ron`.
- Markets now express role-specific demand, use lower raw-source rates and production buffers, and preserve stronger value growth through secondary goods.
- Automated traders now reposition to supply markets after unloading at demand-only destinations.
- Global market spreads, untargeted demand, raw-source output, and idle trader repositioning are now designer-configurable in `content/economy_config.ron`.
- Replaced generic currency reporting with physical energy-flow, production, reserve, funded-demand, realized processor cost/revenue/margin, storage, trader-tank, and separate physical-transfer diagnostics in the long-run `--economy-diagnostics` report.
- Expanded interval and final economy diagnostics with per-system net flow/storage/stage history, network stage percentages, seasonal state, and cycle-amplitude summaries; diagnostics and player-impact probes now fail on reconciliation calculation errors or mismatches.
- Nonzero seasonal amplitudes now require even periods so triangle waves reach exact sampled extrema, with source-aware content errors for invalid definitions.
- Conflicting CLI execution modes are rejected instead of being resolved by argument-order precedence.
- Cost-aware asks now compound sustainable cost-basis margin with bounded scarcity, while processor input bids use deterministic non-recursive solvency ceilings.
- Repository markets now use dynamic population, with Sable Junction tuned as a recoverable demographic stress case; app, TUI, and CLI views expose population trend, carrying cap, tier, sufficiency trajectory, and aggregate stage/population history.
- The long 1,000-tick repository content acceptance is ignored by default with an explicit command; routine tests retain a 50-tick deterministic/activity smoke and short system-only/trader-only insertion-order checks.
- Repository markets now enable all four tuned investment shapes with a shared default AI allocation; investment energy is reported as an explicit physical sink in reconciliation and market-flow views.

### Fixed

- Canonical typed key routing now governs live input precedence; obsolete punctuation and case-sensitive governance aliases are inert, and no-action keys leave UI and application state unchanged.
- Route proposals survive unrelated inspection changes and rejected commits, cannot be replaced during travel, and display energy only from the route view matching the proposed destination.
- Textual active, selection, warning, severity, read-only, empty, disabled, and shortcut cues remain meaningful without color; extreme displayed allocation and usage values are formatted without overflow.
- Long-run dynamic-fleet diagnostics now skip brownout-suppressed zero-bid opportunities, and route subsidies retain cost-aware processor solvency ceilings.
- Logistic population growth now preserves compatible fractional carries exactly and uses unbiased round-half-to-even conversion for incompatible carrying-capacity changes, preventing both tiny-population stalls and premature growth.
