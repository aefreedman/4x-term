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
- `economy.ron` controls per-system inventories, demand targets, recipes, raw sources, and deterministic seasonal generation.
- `goods.ron` controls individual base prices.
- `traders.ron` controls fixed/dynamic fleet mode, initial and maximum count, response/retirement windows, speed, physical starting tank, cargo capacity, naming, and distribution.

The dynamic production fleet begins with nine evenly spaced NPC traders and adapts slowly to persistent normalized unserved profitable opportunity.

## Economy diagnostics

Run `cargo run -p game-cli -- --economy-diagnostics 500` to inspect 50-tick and final per-system net flow, storage, brownout history, seasonal phase/output, network stage percentages, cycle amplitudes, fleet size/backlog/persistence/spawn/retirement state, physical-energy reconciliation, and NPC cargo/travel/profitability state.

Run an identical-session player-impact probe with one explicitly recorded external delivery:

```bash
cargo run -p game-cli -- --player-impact \
  --impact-target frontier:system_04 --impact-tick 300 \
  --impact-good core:energy --impact-quantity 500 --impact-horizon 500
```

The probe requires a stage or population divergence within the bounded horizon and reconciles the intervention inflow separately.

The 10,000-tick population/metastability harness is an explicit acceptance run, not a routine test/CI path:

```bash
cargo run -p game-cli --release -- --economy-diagnostics 10000
```

The repository's older 1,000-tick content acceptance is ignored by default because it is intentionally long. Run it explicitly with:

```bash
cargo test -p game-content tests::repository_energy_economy_remains_active_and_deterministic_for_1000_ticks -- --ignored --exact
```

A 50-tick deterministic/activity smoke and short system-only/trader-only insertion permutations remain in the default suite.

## Validation

```bash
cargo run -p game-cli -- --validate-content
cargo run -p game-cli -- --headless
cargo run -p game-cli -- --economy-diagnostics 500
cargo run -p game-cli -- --player-impact --impact-target frontier:system_04 --impact-tick 300 --impact-good core:energy --impact-quantity 500 --impact-horizon 500
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```
