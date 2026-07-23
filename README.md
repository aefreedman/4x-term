# 4x-term

A Rust 4X project with a constructive procedural frontier, bounded headless expansion simulation, and a keyboard-first terminal interface.

> **Current status:** Stage 5 is playable. The terminal startup flow generates
> and previews an explicit profile/seed world, then supports construction,
> Habitat bootstrap, scouting, expeditions, founding, and manual atomic ticks.
> Persistence and agent-facing automation remain future work.

`game-core` owns fixed-point systems and body resources, global phase-major ticks, Habitat-backed population tokens, geometric routing, delayed origin knowledge, Shipyards, probes, expeditions, transit, founding, and typed loss. Its public `PlayerWorldView` redacts unknown systems, intermediate route stops, remote runtime state, and unreceived mission outcomes. Privileged complete snapshots are available only through the `test-support` feature.

`game-content` compiles strict authored-world fixtures and editable RON profiles, produces canonical SHA-256 profile fingerprints, and deterministically generates `core:frontier_world@1` artifacts from complete version/seed/profile identity. [`content/profiles/starter.ron`](content/profiles/starter.ron) is the editable profile offered by the human-play executable. It is not a canonical universe. Only the origin scaffold is a generated-world structural guarantee; frontier count and qualitative world outcomes are not acceptance oracles.

See [docs/architecture.md](docs/architecture.md) for current ownership and API boundaries and [Game Design](docs/design/README.md) for current mechanics, long-term direction, lore, and explicitly non-authoritative ideas. The [Testing Stance](docs/plans/2026-07-20-testing-stance-correction.md) and [Engine Invariant Registry](docs/2026-07-20-engine-invariant-registry.md) define the evidence policy.

## Development setup

On macOS, run:

```bash
./setup/bootstrap-macos.sh
./setup/doctor.sh
```

See [setup/README.md](setup/README.md) for prerequisites and details.

## Play

From the repository root, use a terminal of at least `160x45` cells:

```bash
cargo run -p game-play
```

The startup screen defaults to `content/profiles/starter.ron` and seed `0`. Edit either field before generation if desired, preview the origin scaffold, and explicitly confirm Start. Press `?` for contextual help. Arrow keys always navigate; Settings switches between QWERTY (`hjkl`) and Colemak-DH (`unei`). Sessions are not saved.

## Validation

The acceptance surface is buildability plus focused deterministic core, content, application, TUI-state, renderer, and terminal-lifecycle tests.

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Do not add generated-world quality gates or compatibility adapters for the retired prototype.
