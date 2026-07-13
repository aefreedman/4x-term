# World Dynamics Acceptance Evidence

Date: 2026-07-13
Branch: `feat/world-dynamics-progression`

## Automated quality checks

The final implementation passed:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo run -p game-cli -- --validate-content
```

Routine workspace result: 117 tests passed, one intentionally ignored long-running legacy acceptance, and no failures. The ignored test remains available through its documented explicit command. Content validation loaded 20 systems, 11 goods, 9 recipes, and 10 initial traders.

## Long-run metastability

Command:

```bash
cargo run -p game-cli --release -- --economy-diagnostics 10000
```

Final accepted run:

- Exit status: success
- Physical energy: expected 45,080; actual 45,080; difference 0
- Processor solvency: 11 checked; 0 insolvent
- Stage transitions: 1,632
- Population changes: 2
- Fleet: 10 NPCs; 1 lifetime spawn; 0 retirements
- Stationary-laden NPCs: 0
- Unsupplied life support: 0
- Metastability validator: passed its extinction, population-ratchet, final-window activity, stable changed-population, and reconciliation checks

Bounded retirement behavior, including laden liquidation and retirement, is covered by focused core tests; Phase 4's dynamic diagnostics separately observed both spawn and retirement events.

## Player-impact probe

Command:

```bash
cargo run -p game-cli --release -- --player-impact \
  --impact-target frontier:system_04 --impact-tick 300 \
  --impact-good core:energy --impact-quantity 500 --impact-horizon 500
```

Result:

- Exactly one recorded 500-energy external delivery
- First divergence at tick 395
- Baseline stage: Throttled
- Intervention stage: Normal
- Baseline and intervention reconciliation differences: 0

## Governor and terminal flow

The non-interactive environment validated terminal behavior through Ratatui `TestBackend` and app actor tests rather than a live terminal session. Coverage confirms:

- All four brownout stages and textual distress feedback
- Seasonal and population projections
- Governed-market reserve, margin, import-priority, and investment-allocation edits through typed requests
- Autonomous investment execution without per-tick player upkeep
- Read-only non-governed markets and typed rejection feedback
- Route-subsidy suppression in Emergency/Starvation and automatic resumption after recovery

No screenshot workflow was used: this is a terminal Rust project, not a Unity project, and deterministic buffer assertions provide the visual regression evidence.
