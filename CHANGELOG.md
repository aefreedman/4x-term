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

### Changed

- Route previews, active travel, direct connections, and player location now use readable system names with jump, distance, and timing summaries instead of exposing internal content IDs.
- Event log entries now resolve system, trader, good, and production-process IDs to readable display names.
- Player cargo now displays readable good names instead of internal content IDs.
- NPC trader setup now uses nine evenly distributed traders with a shared speed and designer-editable parameters in `content/traders.ron`.
- Markets now express role-specific demand, use lower raw-source rates and production buffers, and preserve stronger value growth through secondary goods.
- Automated traders now reposition to supply markets after unloading at demand-only destinations.
- Global market spreads, untargeted demand, raw-source output, and idle trader repositioning are now designer-configurable in `content/economy_config.ron`.
- Replaced generic currency reporting with physical energy-flow, production, reserve, funded-demand, realized processor cost/revenue/margin, storage, trader-tank, and separate physical-transfer diagnostics in the long-run `--economy-diagnostics` report.
- Cost-aware asks now compound sustainable cost-basis margin with bounded scarcity, while processor input bids use deterministic non-recursive solvency ceilings.
