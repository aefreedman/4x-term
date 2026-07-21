# Changelog

## Unreleased

### Added

- Stage 4b's deterministic `core:frontier_world@1` generator, editable strict
  RON profiles, canonical SHA-256 profile fingerprints, logical provenance, and
  complete version/seed/profile generation identity.
- Fixed-point geometric routing, initial origin knowledge, keyed observations,
  delayed fact transmissions, monotonic deterministic merge, and redacted
  player route/system views.
- Habitat-backed population tokens, automatic generation, stable population
  identity, derived occupancy/support/work, and explicit transition accounting.
- Shipyards with independent FIFO project queues, completed probes and
  expeditions, world-owned transit, typed slot reservations, settlement,
  insufficient-slot loss, delayed outcome reports, and remote command unlock.
- Whole-world ten-phase atomic ticking with exact cross-system resource and
  population reconciliation evidence.
- Strict authored-world support for coherent initial resident population tokens,
  including initialized-population accounting and never-reused sequence setup.
- An editable `content/profiles/starter.ron` baseline and 56 focused
  deterministic tests across `game-core` and `game-content`.

### Changed

- Every location now has persistent system runtime. Immutable map facts own
  initial body resources while runtime bodies solely own remaining quantities;
  systems own stocks, infrastructure, queues, assets, and accounting.
- Population tokens are the sole mutable population authority; community
  population and Habitat occupancy are derived views.
- Simulation time, transit, and origin knowledge are world-owned, and each tick
  executes phase-major across all systems in stable order before committing.
- The public runtime boundary is a knowledge-filtered `PlayerWorldView`;
  privileged complete snapshots are gated behind the `test-support` feature.
- Generated-world artifacts now expose identity, provenance, and validated world
  definitions through read-only accessors rather than public fields.
- The retained Stage 4 Collector, Battery, Extractor, Refinery, construction,
  life-support, overflow, and accounting mechanics now run under the Stage 4b
  map/runtime schema.
- The workspace remains headless and non-playable; no application, startup
  session, CLI, TUI, save format, or production world bundle is present.

### Removed

- Floating-point map positions, explicit route topology, standalone deposits and
  deposit reservations, origin-only ticking, and writable population totals.
- Compatibility fields, fixtures, tests, and current-state documentation for
  the replaced Stage 3/4 world schemas.
- Generated-world target-count, connectivity, reachability, solvency, survival,
  and qualitative-quality acceptance oracles; only the constructive origin
  scaffold is guaranteed.

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
