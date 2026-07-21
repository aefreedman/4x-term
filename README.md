# 4x-term

A Rust 4X project with a constructive procedural frontier and bounded headless
expansion simulation.

> **Current status:** Stage 4b is implemented in the headless `game-core` and
> RON/profile/generation `game-content` crates. The workspace has no playable
> application, startup session, CLI, TUI, save system, or production world
> bundle. Stage 5 owns the first truthful startup and terminal boundary.

`game-core` owns fixed-point systems and body resources, global phase-major
ticks, Habitat-backed population tokens, geometric routing, delayed origin
knowledge, Shipyards, probes, expeditions, transit, founding, and typed loss.
Its public `PlayerWorldView` redacts unknown systems, intermediate route stops,
remote runtime state, and unreceived mission outcomes. Privileged complete
snapshots are available only through the `test-support` feature.

`game-content` compiles strict authored-world fixtures and editable RON profiles,
produces canonical SHA-256 profile fingerprints, and deterministically generates
`core:frontier_world@1` artifacts from complete version/seed/profile identity.
[`content/profiles/starter.ron`](content/profiles/starter.ron) is an editable
baseline, not a canonical universe or playable startup path. Only the origin
scaffold is a generated-world structural guarantee; frontier count and
qualitative world outcomes are not acceptance oracles.

See [docs/architecture.md](docs/architecture.md) for current ownership and API
boundaries and the [Game Design Wiki](docs/design/README.md) for durable gameplay
contracts. The [Testing Stance](docs/2026-07-20-testing-stance-correction.md) and
[Engine Invariant Registry](docs/2026-07-20-engine-invariant-registry.md) define
the evidence policy.

## Development setup

On macOS, run:

```bash
./setup/bootstrap-macos.sh
./setup/doctor.sh
```

See [setup/README.md](setup/README.md) for prerequisites and details.

## Validation

The current acceptance surface is buildability plus 56 focused deterministic
tests: 28 in `game-core` and 28 in `game-content`.

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Do not add a frontend shell, generated-world quality gate, or compatibility
adapter merely to make the retired prototype runnable.
