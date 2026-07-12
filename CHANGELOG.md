# Changelog

## Unreleased

### Added

- Initial Rust terminal prototype with a headless ECS simulation.
- Data-defined 20-system frontier economy, production recipes, markets, and traders.
- Player trading, multi-hop travel, economic status, asynchronous simulation controls, and Ratatui interface.
- Headless content validation and simulation commands.

### Changed

- Route previews, active travel, direct connections, and player location now use readable system names with jump, distance, and timing summaries instead of exposing internal content IDs.
- Event log entries now resolve system, trader, good, and production-process IDs to readable display names.
- Player cargo now displays readable good names instead of internal content IDs.
- NPC trader setup now uses nine evenly distributed traders with a shared speed and designer-editable parameters in `content/traders.ron`.
- Markets now express role-specific demand, use lower raw-source rates and production buffers, and preserve stronger value growth through secondary goods.
- Automated traders now reposition to supply markets after unloading at demand-only destinations.
- Global market spreads, untargeted demand, raw-source output, and idle trader repositioning are now designer-configurable in `content/economy_config.ron`.
