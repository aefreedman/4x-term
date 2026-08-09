# Project Conventions

## Authority

- Read `AGENTS.md` before changing the repository.
- Read `docs/architecture.md` before changing architecture.
- Read `docs/design/README.md` before changing game design.
- Treat `docs/PRINCIPLES.md` as a redirect, not a separate authority.
- Use `specs/` for bigpowers planning and execution state.

## Commands

| Gate | Command |
| --- | --- |
| Run | `cargo run -p game-play` |
| Format | `cargo fmt --all -- --check` |
| Check | `cargo check --workspace --all-targets --all-features` |
| Lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| Test | `cargo test --workspace --all-features` |
| Preflight | `cargo fmt --all -- --check && cargo check --workspace --all-targets --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features` |

## Workflow

- Keep `specs/state.yaml` synchronized with the current worktree before starting work.
- Use `specs/execution-status.yaml` as the sole story-status authority.
- Keep release ordering in `specs/release-plan.yaml`.
- Give every implementation task a runnable `verify` command.
- Use the `solo-git` integration workflow in `specs/workflows/solo-git.yaml`.
- Land approved work with `scripts/land-branch.sh` from the primary repository.
- Use Conventional Commits with the form `type(scope): reason`.
- NEVER add co-author trailers to commits.

## Always Green

- Run the smallest relevant test after each behavior change.
- Run Preflight before integration.
- NEVER proceed after a reproducible gate failure.
- Fix a small discovered defect separately.
- Record a non-trivial discovered defect before continuing.
- NEVER dismiss a failure as pre-existing, unrelated, or out of scope.

## Engineering

- Keep the simulation headless and independent from terminal rendering.
- Preserve deterministic ordering, checked arithmetic, and player-safe information boundaries.
- Add no dependency or crate boundary without a concrete need.
- Prefer typed errors and validated commands over panics.
- Use timeouts only at real external or pacing boundaries.
- Preserve graceful terminal cleanup when failures occur.
