# 4x-term

A data-driven 4X terminal game written in Rust.

The planned architecture keeps the headless ECS simulation independent from
terminal rendering so other frontends remain possible. See
[docs/architecture.md](docs/architecture.md) for the current design.

> **Transition status:** The authored market-trading prototype is being removed
> before its origin-and-frontier replacement exists. During Stages 2–4 the
> workspace is required to compile and preserve focused engine contracts, but no
> playable CLI/TUI, repository-content acceptance, or legacy command is
> promised. Git history—not compatibility code or a working-tree archive—keeps
> the former prototype available for archaeology.

Current direction is defined by the
[Governance Sandbox](docs/2026-07-20-design-direction-governance-sandbox.md),
the [Testing Stance and Constructive Worldgen transition](docs/2026-07-20-testing-stance-correction.md),
and the [Engine Invariant Registry](docs/2026-07-20-engine-invariant-registry.md).

## Development setup

On macOS, run:

```bash
./setup/bootstrap-macos.sh
./setup/doctor.sh
```

See [setup/README.md](setup/README.md) for prerequisites and details.

## Validation during migration

The supported repository gates cover retained code and contracts only:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Do not restore deleted gameplay commands, authored-world acceptance, diagnostics,
or broad tests merely to make the legacy prototype runnable.
