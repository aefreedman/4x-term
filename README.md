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

## Run the game

From the repository root:

```bash
cargo run -p game-cli
```

Energy remains the only unit of account for ordinary goods, but Energy itself moves physically only through delivery contracts and exact storage transfers.

The interface is organized into five top-level activities:

- `F1` **Systems**: `↑`/`↓` or `j`/`k` wraps through systems; `Enter` opens overview detail, including the goods that the system can produce from sources and recipes; `m` opens the selected system's read-only market; `o` changes the sort column; `d` reverses sort direction. Press `F2` to carry the selected destination into a route proposal.
- `F2` **Trade**: `Tab`/`Shift-Tab` switches between ordinary goods, Energy logistics, and read-only destination comparisons. `↑`/`↓` or `j`/`k` moves only the active region. `n` changes the exact reusable quantity; `b`/`s` buy or sell ordinary goods only. `e` submits the selected Energy delivery opportunity at its exact displayed payload for next-step resolution; `x` cancels the player's remote pre-load contract; `f` transfers owned bulk Energy to the tank; `p` deposits owned bulk Energy into the current market. Logistics panels show offers, requests, payload split, locked bulk, contract state, blockers, and recovery outcomes. `Shift-B`/`Shift-S` opens a focused ordinary-goods order with live cost, capacity, maximum-quantity, and limiting-reason feedback; `m` uses the current maximum, `Enter` confirms, and `Esc` cancels. `t` or `Enter` commits only the displayed travel proposal outside an order; `g` starts or continues the journey, runs the simulation until arrival, and then pauses.
- `F3` **Governance**: `↑`/`↓` or `j`/`k` selects a row; `Tab`/`Shift-Tab` jumps between policy, market-target, import-priority, and investment sections; `←`/`→` edits an available governed row, including per-good target amounts; `i` inspects the stable Systems selection; `Esc` returns to the governed market. Autonomous markets are explicitly read-only.
- `F4` **Intelligence**: `↑`/`↓` or `j`/`k` scrolls the bounded event history.
- `F5` **Encyclopedia**: `Tab`/`Shift-Tab` switches factual manual sections; `↑`/`↓` or `j`/`k` selects an article; `PageUp`/`PageDown` scrolls the selected article. Encyclopedia articles describe game mechanics and catalog reference material separately from controls-only contextual help.

Global controls are `Space` to pause/resume, `.` to single-step while paused, `r` to change tick rate, `?` for contextual help, and `q` to quit. Unavailable actions are shown as disabled with a reason.

Terminal dimensions are measured in cells. `80x30` is the minimum supported compact layout and `160x45` enables the regular side-by-side layout; smaller terminals show only resize and quit guidance. The game remains menu- and table-oriented and deliberately does not render a spatial ASCII map. It exposes current market information for all systems. Frontier System 01 is the player's authored starting governorship; every other market remains read-only. Governance edits policy through typed application requests, while investments execute autonomously each tick rather than through upkeep clicks.

## Designer configuration

Runtime content is stored under `content/`.

- `economy_config.ron` controls global market policy, brownouts, population, all four diminishing-cost investment shapes, default AI allocations, raw-source output, and idle NPC repositioning.
- `encyclopedia.ron` contains the player-facing manual sections, articles, and paragraphs. Its prose is loaded as content and projected unchanged through the application layer.
- `economy.ron` controls per-system inventories, demand targets, recipes, raw sources, deterministic seasonal generation, optional investment-allocation overrides, and the optional starting governor.
- `goods.ron` controls individual base prices.
- `traders.ron` controls fixed/dynamic fleet mode, archetype initial/max counts, response/retirement windows, speed, physical starting tank and bulk capacity, ordinary cargo capacity, refuel policy, player trade-network access, naming, and distribution. Energy fee/recovery policy lives in economy configuration.

The dynamic production fleet begins with nine evenly spaced NPC traders and adapts slowly to persistent normalized unserved profitable opportunity.

## Economy diagnostics

Run `cargo run -p game-cli -- --economy-diagnostics 500` to inspect 50-tick and final per-system net flow, storage, brownout history, seasonal phase/output, network stage percentages, cycle amplitudes, fleet size/backlog/persistence/spawn/retirement state, physical-energy reconciliation, NPC cargo/travel/profitability state, Energy offers/requests, contract outcomes, timeout/recovery, starvation causes, and per-archetype activity.

An identical-session `--player-impact` probe remains available as a controlled diagnostic when a specifically tuned bounded external-delivery intervention is needed; it is not a routine player logistics command. The probe requires a stage or population divergence within its horizon and reconciles intervention inflow separately. The fast controlled unit fixture is the routine gate; repository-scale probe parameters must be chosen from the current diagnostics rather than copied from historical tuning.

The 10,000-tick population/metastability harness is an explicit acceptance run, not a routine test/CI path:

```bash
cargo run -p game-cli --release -- --economy-diagnostics 10000
```

The 1,000- and 10,000-tick commands are explicit long acceptance runs, not routine/default tests; their presence here does not claim they were run. The 1,000-tick content acceptance is ignored by default. Run it explicitly with:

```bash
cargo test -p game-content tests::repository_energy_economy_remains_active_and_deterministic_for_1000_ticks -- --ignored --exact
```

A 50-tick deterministic/activity smoke and short system-only/trader-only insertion permutations remain in the default suite; the fast controlled unit test is the routine gate.

## Validation

```bash
cargo run -p game-cli -- --validate-content
cargo run -p game-cli -- --headless
cargo run -p game-cli -- --economy-diagnostics 500
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```
