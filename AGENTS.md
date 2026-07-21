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

## Testing and world generation

- An individual generated-seed outcome is a bug only when it violates a named engine invariant or a G18 constructive guarantee. Do not tune constants to repair one seed's local behavior.
- Local collapse is expected world texture and future reclamation content, not a failure by itself.
- Do not write acceptance criteria against a specific authored universe except for small Tier 1 fixtures whose outcomes are hand-computable.
- Gameplay-facing behavior needs short, deterministic Tier 1 scenario coverage. A behavior observable only through a soak is a simulation behavior, not a gameplay acceptance test.
- Construct the approved origin scaffold directly. Do not require neighborhood viability unless a later approved design adds a concrete structural witness; never add post-hoc gameplay screening or statistical world-quality gates.
- Treat generator parameter ranges as reviewed design decisions. Flag range changes for design review and version the generator when reproducibility requires it.
- When a generated-world failure occurs, reproduce and retain the failure class as a Tier 1 fixture where possible before fixing it.

The current trader-first authored market network is a full replacement target, not a compatibility contract. During the migration, the workspace must remain buildable around retained contracts but need not remain playable. Delete obsolete code, content, tests, diagnostics, and docs instead of preserving compatibility or copying them into an archive; Git history is the recovery path.
