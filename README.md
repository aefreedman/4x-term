# 4x-term

A data-driven 4X terminal game written in Rust.

The planned architecture keeps the headless ECS simulation independent from terminal rendering so other frontends remain possible. See [docs/architecture.md](docs/architecture.md) for the current design.

## Development setup

On macOS, run:

```bash
./setup/bootstrap-macos.sh
./setup/doctor.sh
```

See [setup/README.md](setup/README.md) for prerequisites and details.

## Run the prototype

From the repository root:

```bash
cargo run -p game-cli
```

Controls:

- `Tab`: move focus
- Arrow keys or `j`/`k`: move selection
- `Space`: pause or resume
- `s`: advance one tick while paused
- `r`: change tick rate
- `n`: enter the quantity used by buy and sell commands
- `b` / `x`: buy or sell the selected quantity of the selected good
- `Enter`: travel to the selected system
- `?`: show or hide help
- `q`: quit

The terminal should be at least 70 columns by 24 rows. The prototype exposes current market information for all systems.

## Designer configuration

Runtime content is stored under `content/`.

- `economy_config.ron` controls global market buy/sell percentages, the untargeted-good discount, overall raw-source output, and idle NPC repositioning.
- `economy.ron` controls per-system inventories, demand targets, recipes, and raw sources.
- `goods.ron` controls individual base prices.
- `traders.ron` controls trader count, speed, starting funds, cargo capacity, naming, and distribution.

The initial nine NPC traders use `EvenlySpaced` distribution across the 20 systems.

## Validation

```bash
cargo run -p game-cli -- --validate-content
cargo run -p game-cli -- --headless
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```
