# 4x-term

A data-driven 4X terminal game written in Rust.

The planned architecture keeps the headless ECS simulation independent from terminal rendering so other frontends remain possible. See [docs/architecture.md](docs/architecture.md) for the current design.

> **Transition status:** The runnable build described below is the legacy authored
> market-trading prototype. It remains operational while the project migrates,
> but its trader-first flow, independent NPC ecology, and metastability targets
> are not current product or compatibility requirements. Current direction is
> defined by the [Governance Sandbox](docs/2026-07-20-design-direction-governance-sandbox.md)
> and [Testing Stance and Constructive Worldgen](docs/2026-07-20-testing-stance-correction.md).

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

The interface is organized into six top-level activities:

- `F1` **Systems**: `↑`/`↓` or `j`/`k` wraps through systems; `Enter` opens overview detail, including the goods that the system can produce from sources and recipes; `m` opens the selected system's read-only market; `o` changes the sort column; `d` reverses sort direction. Press `F2` to carry the selected destination into a route proposal.
- `F2` **Trade**: `Tab`/`Shift-Tab` switches between ordinary goods and read-only destination comparisons. `↑`/`↓` or `j`/`k` moves within the active region. `b` or `s` opens a fresh exact-amount buy or sell dialog; there is no reusable preset quantity or immediate trade shortcut. The dialog shows the current maximum and limiting reason; `m` fills that maximum, `Enter` confirms, and `Esc` cancels. `t` or `Enter` commits the displayed travel proposal; `g` runs the simulation until arrival and pauses.
- `F3` **Logistics**: `Tab`/`Shift-Tab` moves between Energy Requests, Active Contract, and Storage. In Energy Requests, `←`/`→` switches between every posted request and only contracts currently serviceable by the player; the all-requests view shows stock, target, inbound Energy, runway, stage, and why an unserviceable request has no current contract. The focused panel explains what `Enter` will do. Serviceable contracts show the complete physical payload, route burns, delivery, fee, and runway change; active contracts explain automatic progress and cancellation limits; storage transfers open their own exact-amount dialog. Energy logistics never reuse ordinary Trade selection or quantity state.
- `F4` **Governance**: `↑`/`↓` or `j`/`k` selects a row; `Tab`/`Shift-Tab` jumps between policy, market-target, import-priority, and investment sections; `←`/`→` edits an available governed row, including per-good target amounts; `i` inspects the stable Systems selection; `Esc` returns to the governed market. Autonomous markets are explicitly read-only.
- `F5` **Intelligence**: `↑`/`↓` or `j`/`k` scrolls the bounded event history.
- `F6` **Encyclopedia**: `Tab`/`Shift-Tab` switches factual manual sections; `↑`/`↓` or `j`/`k` selects an article; `PageUp`/`PageDown` scrolls the selected article. Encyclopedia articles describe game mechanics and catalog reference material separately from controls-only contextual help.

The footer is reserved for global controls and command feedback: `Space` pauses/resumes, `.` single-steps while paused, `r` changes tick rate, `?` opens contextual help, and `q` quits. Activity-specific actions are labelled inside their focused panels, and unavailable actions show a reason there.

Terminal dimensions are measured in cells. `80x30` is the minimum supported compact layout and `160x45` enables the regular side-by-side layout; smaller terminals show only resize and quit guidance. The game remains menu- and table-oriented and deliberately does not render a spatial ASCII map. It exposes current market information for all systems. Frontier System 01 is the player's authored starting governorship; every other market remains read-only. Governance edits policy through typed application requests, while investments execute autonomously each tick rather than through upkeep clicks.

## Designer configuration

Runtime content is stored under `content/`.

- `economy_config.ron` controls global market policy, brownouts, population, all four diminishing-cost investment shapes, default AI allocations, raw-source output, and idle NPC repositioning.
- `encyclopedia.ron` contains the player-facing manual sections, articles, and paragraphs. Its prose is loaded as content and projected unchanged through the application layer.
- `economy.ron` controls per-system inventories, demand targets, recipes, raw sources, deterministic seasonal generation, optional investment-allocation overrides, and the optional starting governor.
- `goods.ron` controls individual base prices.
- `traders.ron` controls fixed/dynamic fleet mode, archetype initial/max counts, response/retirement windows, speed, physical starting tank and bulk capacity, ordinary cargo capacity, refuel policy, player trade-network access, naming, and distribution. Energy fee/recovery policy lives in economy configuration.

The dynamic production fleet begins with nine evenly spaced NPC traders and adapts slowly to persistent normalized unserved profitable opportunity.

## Legacy economy diagnostics

These commands inspect the runnable market prototype. Their world-health, fleet-activity, player-impact, and metastability thresholds are historical diagnostics, not current product acceptance policy; exact reconciliation remains useful evidence independently of those thresholds.

Run `cargo run -p game-cli -- --economy-diagnostics 500` to inspect 50-tick and final per-system net flow, storage, brownout history, seasonal phase/output, network stage percentages, cycle amplitudes, fleet size/backlog/persistence/spawn/retirement state, physical-energy reconciliation, NPC cargo/travel/profitability state, Energy offers/requests, contract outcomes, timeout/recovery, starvation causes, and per-archetype activity.

An identical-session `--player-impact` probe remains available as a controlled legacy diagnostic when a specifically tuned bounded external-delivery intervention is needed; it is not a routine player logistics command or a current quality gate. The probe requires a stage or population divergence within its horizon and reconciles intervention inflow separately. The fast controlled unit fixture exercises the mechanism; repository-scale probe parameters must be chosen from current diagnostics rather than copied from historical tuning.

The 10,000-tick population/metastability harness is an explicit legacy diagnostic run, not a routine test/CI path or current acceptance gate:

```bash
cargo run -p game-cli --release -- --economy-diagnostics 10000
```

The 1,000- and 10,000-tick commands are explicit long legacy runs, not routine/default tests; their presence here does not claim they were run or make their authored-world thresholds current requirements. The 1,000-tick content acceptance is ignored by default. Run it explicitly with:

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
