# 4x-term

A Rust 4X project in an origin-and-frontier migration.

> **Current status:** Stage 3 is complete. The workspace contains only the
> headless `game-core` and source-compilation `game-content` crates. It has no
> playable application, CLI, TUI, or production content bundle. Stage 5 will
> own a new truthful startup and terminal boundary; it will not restore the
> retired trader-market prototype.

`game-core` owns a format-independent `WorldState` substrate: resources,
neutral locations, exactly one living origin community and its physical stocks,
resource deposits, reclaimable sites, and explicit topology. Topology may be
empty or disconnected. `game-content` compiles one RON world source and returns
deterministically ordered, source-aware aggregated diagnostics before any world
is instantiated.

See [docs/architecture.md](docs/architecture.md) for the current boundaries.
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

The current acceptance surface is buildability plus 15 focused deterministic
tests. They cover stable IDs, checked Energy arithmetic, normalized world
instantiation and permutations, neutral frontier locations, valid empty or
disconnected topology, invalid definition/topology rejection, exact resource
reconciliation and atomic transfer rejection, and strict source-aware content
compilation diagnostics.

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Do not reintroduce gameplay commands, a frontend shell, authored-world
acceptance, or production content merely to make the former prototype runnable.
