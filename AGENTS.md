# Agent guidance

## Project

4x-term is a data-driven 4X terminal game planned in Rust. Read `docs/architecture.md` before making architectural changes.

## Working conventions

- Keep the simulation headless and independent from terminal rendering.
- Prefer small, focused changes.
- Do not add dependencies or create new crate boundaries without a concrete need.
- Add tests with implementation changes once code scaffolding exists.
- Update `CHANGELOG.md` under `Unreleased` for user-visible changes.
- Never commit credentials, machine-local Pi configuration, or generated build output.
