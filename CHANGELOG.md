# Changelog

## Unreleased

### Added

- Stage 4's authored headless origin resource engine: deterministic monthly
  ticks, ten-phase Collector output, life support, Batteries, Extractors,
  Refineries, generic body slots, and FIFO construction.
- Exact system accounting for construction commitments, production cycles,
  mutable deposit depletion, Energy retention/overflow, cancellation refunds,
  shortages, and capacity-aware receipts.
- Strict designer-authored Stage 4 RON definitions plus the exact 20-tick
  zero-population Collector → Refinery → Battery → Extractor fixture.
- Stage 3's headless origin-and-frontier substrate: resources, neutral
  locations, exactly one living origin community with physical stocks, resource
  deposits, reclaimable sites, and explicit topology.
- Deterministic `WorldState` snapshots, normalized topology and input ordering,
  and checked physical-resource transfer/reconciliation evidence.
- One-source strict RON world compilation with deterministic, source-aware
  aggregated diagnostics and focused Tier 1 fixtures.
- A reviewed engine-invariant registry with exact oracles, applicability rules,
  non-vacuity witnesses, and focused test evidence.
- Retained frontend architecture, terminal UX, testing, and removed-dependency
  lessons for the future Stage 5 playable-surface rebuild.

### Changed

- Physical stocks, bodies, developments, queues, and accounting now belong to
  persistent systems; communities contain population only and may start at
  zero.
- The workspace contains only `game-core` and `game-content`; the retained
  acceptance surface is headless buildability and 40 focused deterministic
  tests.
- Topology is explicit and may be empty or disconnected; locations do not gain
  living state from deposits, sites, or topology.
- Content compilation validates one schema-specific source instead of a fixed
  repository bundle.
- Migration CI gates formatting, compilation, linting, and retained workspace
  tests instead of legacy product acceptance.

### Removed

- The playable app, CLI, and terminal UI boundaries, along with their terminal
  and async dependency chain.
- Production authored market content and its fixed repository loader.
- Markets, pricing, wallets, commercial reservations and Energy contracts,
  player/NPC traders, fleets, market-per-location state, and related gameplay
  acceptance and diagnostics.
- Authored-world cardinality, ecology-role, bootstrap-solvency,
  liquidation-adequacy, fleet-route, and repository-activity quality gates.

## 0.7.1 - 2026-07-20

### Changed

- Logistics defaults to every posted Energy request and can switch to player-serviceable contracts, making unserviceable demand and its blocker visible.
- Added an implementation-ready plan for replacing authored Energy participation switches with need-derived requests and safe-surplus offers.

## 0.7.0 - 2026-07-16

### Added

- Physical Energy delivery contracts with owned and locked bulk, exact storage transfers, deterministic settlement, timeout recovery, and player/NPC support.
- A dedicated Logistics activity for delivery opportunities, active contracts, and storage transfers.

### Changed

- Energy views distinguish market-owned bulk, contract-locked bulk, reserve claims, protected budgets, player-owned bulk, tank level, route runway, and market health.
- The terminal interface provides six activities: Systems, Trade, Logistics, Governance, Intelligence, and Encyclopedia.
- Every Trade purchase and sale uses a fresh exact-amount dialog with maximum quantity, cost, and limiting-reason feedback.
- NPC trader selection and spawning account for logistics archetypes; Energy is handled through contracts and storage rather than ordinary cargo trading.
- Economy diagnostics include owned bulk and Energy-contract flows.

## 0.6.0 - 2026-07-14

### Added

- Five activity-based terminal views for Systems, Trade, Governance, Intelligence, and Encyclopedia, with compact (`80x30`) and regular (`160x45`) layouts.
- Progression-ready player trade-network access, including core-level rejection of offline reservation contracts and visible capability state.
- A scrollable, content-defined Encyclopedia covering systems, Energy, brownouts, population, goods, markets, recipes, governance, investments, traders, reservations, travel, and trade-network access.
- Destination-market comparisons with route time and Energy requirements for the selected good.
- Route previews that preserve stable destination selection until travel is committed.
- A Trade command that starts or continues a journey, advances through arrival, and pauses automatically.
- Governor-authorized per-good market targets with projected demand and rejection feedback.
- System production views that distinguish raw output per tick from recipe output per run.

### Changed

- Systems, Trade, and Governance tables use bounded selected-row viewports with position and overflow indicators.
- Trade supports reusable-quantity shortcuts and focused single-transaction orders with quantity, cost, tank, cargo, and limiting-reason feedback.
- Systems navigation wraps, remote markets are explicitly read-only, and Governance supports section-to-section navigation.
- Route and travel displays use readable system names with jump, distance, timing, and required-Energy summaries.
- Encyclopedia prose is loaded from validated `content/encyclopedia.ron` and introduces mechanics in plain language before detailed terminology.
- Paused single-step uses `.`, preserving function keys for top-level activities and `(S)ell` for selling.

### Fixed

- Input routing has deterministic precedence, and keys without an action leave application and interface state unchanged.
- Route proposals survive unrelated inspection changes and rejected commits, cannot be replaced during travel, and display Energy only for the proposed destination.
- Textual selection, warning, severity, read-only, disabled, and shortcut cues remain meaningful without color.
- Extreme allocation and usage values render without overflowing their fields.

## 0.5.0 - 2026-07-13

### Added

- A deterministic four-stage brownout model with stage-aware throughput, demand, pricing, protection, runway, and terminal visibility.
- Content definitions for seasons, static and dynamic populations, fixed and dynamic fleets, investments, governance, and aggregate history.
- Deterministic seasonal production on three prototype systems, including phase, trend, and next-turning-point views.
- A bounded player-impact diagnostic that compares identical sessions and reconciles a recorded external delivery.
- An endogenous NPC fleet with persistent opportunity tracking, market-funded spawning, bounded profitability, and conservation-safe retirement.
- Dynamic population decline and recovery driven by Energy and goods sufficiency, with carrying capacity and population tiers.
- Autonomous investment in collectors, storage, population support, and route subsidies, subject to allocation, cost, cooldown, funding, and brownout constraints.
- Player governorship with authorized reserve, margin, import-priority, and investment-allocation controls.

### Changed

- NPC traders use dynamic production and configurable archetype, response, cooldown, retirement, and fleet-size parameters.
- Economy diagnostics include per-system flow, storage, brownout-stage, seasonal, and population history.
- Repository markets use dynamic populations, with Sable Junction configured as a recoverable demographic stress case.
- Repository markets enable all four investment types with default AI allocations, and reconciliation reports investment Energy as a physical sink.
- Seasonal definitions with nonzero amplitude require even periods so sampled triangle waves reach both extrema.
- Conflicting CLI execution modes are rejected.
- Long-running repository acceptance tests are opt-in, while routine tests cover deterministic short-run activity and insertion order.

### Fixed

- Fleet diagnostics exclude opportunities suppressed by brownout zero bids, and route subsidies retain processor solvency limits.
- Population growth preserves fractional progress across compatible capacity changes and uses unbiased rounding when capacity changes are incompatible.

## 0.4.0 - 2026-07-13

### Added

- Physical Energy accounting for market stock, reserves, protected budgets, player tank and cargo Energy, route runway, and market health.
- Energy-flow reconciliation and per-market solvency diagnostics.
- Scarcity-aware and cost-aware pricing comparison from identical simulation states.
- Deterministic partial-arrival recovery, processor structural-solvency reporting, authored refueling policies, and explicit Energy-level displays.

### Changed

- Economy diagnostics report physical Energy flow, production, reserves, funded demand, processor costs and revenue, storage, trader tanks, and transfers instead of generic currency flow.
- Ask prices combine sustainable cost-basis margins with bounded scarcity; processor input bids use deterministic solvency ceilings.

## 0.3.0 - 2026-07-11

### Added

- Cumulative market cash-flow and production diagnostics.
- A long-running `--economy-diagnostics` report for liquidity and trader-stall analysis.

### Changed

- Markets use role-specific demand, lower raw-source rates, production buffers, and stronger value growth through secondary goods.
- Traders reposition to supply markets after unloading at demand-only destinations.
- Market spreads, untargeted demand, raw-source output, and idle-trader repositioning are configurable in `content/economy_config.ron`.

## 0.2.0 - 2026-07-11

### Changed

- NPC trader configuration defines nine evenly distributed traders, shared travel speed, and designer-editable parameters in `content/traders.ron`.

## 0.1.0 - 2026-07-11

### Added

- Initial Rust terminal prototype with a headless ECS simulation.
- A data-defined 20-system frontier economy with production recipes, markets, and traders.
- Player trading, multi-hop travel, economic status, asynchronous simulation controls, and a Ratatui interface.
- Headless content validation and simulation commands.

### Changed

- Route previews, active travel, direct connections, and player location use readable system names with jump, distance, and timing summaries.
- Event log entries display readable system, trader, good, and production-process names.
- Player cargo displays readable good names.
