# 4x-term

A Rust 4X project in an origin-and-frontier migration.

> **Current status:** Stage 4 is complete. The workspace contains only the
> headless `game-core` and source-compilation `game-content` crates. It has no
> playable application, CLI, TUI, generated frontier, outward action, or
> production content bundle. Stage 4b owns constructive generation and bounded
> expansion; Stage 5 owns a truthful startup and terminal boundary.

`game-core` owns a format-independent `WorldState`: neutral locations, one
population-only origin community, persistent system-owned stocks, mutable
deposits, unchanged reclaimable sites, explicit topology, and the optional
Stage 4 resource engine. The engine provides deterministic ticks, seasonal
Collectors, life support, Batteries, Extractors, Refineries, generic body
slots, and FIFO construction with exact accounting. `game-content` compiles
strict Stage 3 or Stage 4 RON sources into normalized definitions with
source-aware aggregated diagnostics.

See [docs/architecture.md](docs/architecture.md) for the current boundaries and
the [Game Design Wiki](docs/design/README.md) for durable gameplay contracts.
The [Governance Sandbox](docs/2026-07-20-design-direction-governance-sandbox.md),
[Testing Stance](docs/2026-07-20-testing-stance-correction.md), and
[Engine Invariant Registry](docs/2026-07-20-engine-invariant-registry.md) define
the broader direction and active evidence policy. Git history, not compatibility
code or an archive, retains the removed prototype.

## Development setup

On macOS, run:

```bash
./setup/bootstrap-macos.sh
./setup/doctor.sh
```

See [setup/README.md](setup/README.md) for prerequisites and details.

## Validation

The current acceptance surface is buildability plus 40 focused deterministic
tests. They cover the retained substrate, strict content diagnostics, stable
IDs, checked resource arithmetic, atomic command rejection, system ownership,
construction reservations/cancellation, development ordering and conditions,
life-support shortages, storage overflow, production cycles, and the exact
20-tick zero-population bootstrap.

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Do not add a frontend shell, generated-world quality gate, outward action, or
production content merely to make the former prototype runnable.
