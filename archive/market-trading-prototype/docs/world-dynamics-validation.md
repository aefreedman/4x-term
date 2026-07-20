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

Routine workspace result after the review-response additions: 122 tests passed, one intentionally ignored long-running acceptance, and no failures. The ignored test is instead mandatory in the pre-merge gate below. Content validation loaded 20 systems, 11 goods, 9 recipes, and 10 initial traders.

## Enforced soak gates

The exact commands and ownership rule are recorded under **Enforced world-dynamics pre-merge gate** in `archive/market-trading-prototype/docs/energy-economy.md`. Changes to the economy/world-dynamics surfaces named there cannot merge until both soaks and the player-impact probe pass and this evidence file is refreshed.

### 1,000-tick deterministic activity soak

Command:

```bash
cargo test -p game-content \
  tests::repository_energy_economy_remains_active_and_deterministic_for_1000_ticks \
  -- --ignored --exact --nocapture
```

Captured final line from the accepted 2026-07-13 run:

```text
1000-tick acceptance: reconciliation_difference=0 stage_transitions=45 population_changes=0 energy_loaded=926262 energy_delivered=924168 trades_after_300=5790 production_after_300=325
```

The test executes the complete run twice and requires identical events/snapshot/metrics. Zero demographic settlements at 1,000 ticks is expected with the 500-tick sufficiency window and slow growth; the required changed-population criterion belongs to—and passed in—the 10,000-tick gate.

### 10,000-tick metastability soak

Command:

```bash
cargo run -p game-cli --release -- --economy-diagnostics 10000
```

Captured final criteria from the accepted 2026-07-13 run:

```text
energy_reconciliation initial=40000 external_inflow=0 generated=2543682 burned=2148252 curtailed=390350 expected=45080 actual=45080 difference=0 status=ok
network_stages current[normal=80.00% throttled=20.00% emergency=0.00% starvation=0.00%] occupancy[normal=88.67% throttled=11.32% emergency=0.00% starvation=0.00%] transitions=1632 population_changes=2 population_milestones=0 npc_fleet_size=10 normalized_unserved_opportunity_per_system=65 opportunity_persistence=0 fleet_spawns=1 fleet_retirements=0
processor_structural_solvency total=11 insolvent=0 status=ok
```

Additional enforced results:

- Exit status and metastability validator: success
- No extinction or global population ratchet; one changed population remained stable over the final 100 ticks
- Continued final-window trade and stage activity, including post-midpoint transitions
- Stationary-laden NPCs: 0
- Unsupplied life support: 0

Bounded retirement behavior, including laden liquidation and retirement, is covered by `laden_sustained_unprofitable_trader_uses_anti_strand_cleanup_and_retires`; Phase 4's dynamic diagnostics separately observed both spawn and retirement events.

## Player-impact probe

Command:

```bash
cargo run -p game-cli --release -- --player-impact \
  --impact-target frontier:system_04 --impact-tick 300 \
  --impact-good core:energy --impact-quantity 500 --impact-horizon 500
```

Captured result:

```text
baseline energy_reconciliation initial=40000 external_inflow=0 generated=124502 burned=131349 curtailed=0 expected=33153 actual=33153 difference=0 status=ok
intervention energy_reconciliation initial=40000 external_inflow=500 generated=124502 burned=131561 curtailed=0 expected=33441 actual=33441 difference=0 status=ok
player_impact first_divergence_tick=395 target=frontier:system_04 baseline_stage=Throttled intervention_stage=Normal baseline_population=5 intervention_population=5 status=bounded
```

The single delivery occurs at tick 300, so the observed player-response delay is 95 ticks.

## Balance note and demurrage decision

Accepted content parameters:

- Brownout entry runway: Throttled 12 ticks, Emergency 6, Starvation 1; recovery runway: 16/8/3; minimum stage residence: 1 tick
- Throughput: Throttled 50%, Emergency/Starvation 0%
- Population: 500-tick sufficiency window, 90% growth gate, decline 10 per thousand, growth 1 per thousand, preserving the intended 10:1 decline/growth ratio
- Fleet: normalized opportunity threshold 100 for 50 consecutive ticks, 100-tick spawn cooldown, 200-tick retirement window, initial/max NPC count 9/20
- Authored seasonal cycles: System 05 is 30%/120 ticks, System 16 is 40%/160 ticks, and System 19 is 50%/200 ticks. Their accepted effective-output ranges were 14–28 (amplitude 14), 5–11 (6), and 6–18 (12), respectively. Collector purchases can also widen whole-run output ranges on otherwise fixed systems.
- Designed player opportunity: fleet expansion needs a full 50-tick persistent signal and is rate-limited by the 100-tick cooldown; the controlled player delivery produced a stage difference 95 ticks after intervention.

Post-investment concentration evidence from the final 10,000-tick market rows:

- Energy stock ranged from 62 to 5,493 per system; mean 1,987.5; population variance 3,704,583.05 energy².
- Storage occupancy ranged from 1.24% to 99.87%; mean 37.46%; population variance 1,215.65 percentage-points².
- Net per-system flow ranged from −1,930 to +4,293; median −10; exactly 10 systems were net-positive and 10 net-negative, with total net flow +9,750 over the reported interval.
- Only 3 of 20 markets finished at or above 95% storage, while exact reconciliation, final-window trade, 1,632 stage transitions, and the population gate all passed.

Demurrage is therefore not added: stock dispersion reflects active exporter/importer structure rather than a frozen global pool. Revisit demurrage if two consecutive accepted 10,000-tick runs have at least half of markets pinned at 95% storage, or if storage variance at or above this 1,215.65 percentage-points² baseline coincides with a failed final-window activity, reconciliation, extinction, or population-ratchet criterion.

## Governor and terminal flow

The non-interactive environment validated terminal behavior through Ratatui `TestBackend` and app actor tests rather than a live terminal session. Coverage confirms:

- All four brownout stages and textual distress feedback
- Seasonal and population projections
- Governed-market reserve, margin, import-priority, and investment-allocation edits through typed requests
- Autonomous investment execution without per-tick player upkeep
- Read-only non-governed markets and typed rejection feedback
- Route-subsidy suppression in Emergency/Starvation and automatic resumption after recovery

No screenshot workflow was used: this is a terminal Rust project, not a Unity project, and deterministic buffer assertions provide the visual regression evidence.

## Physical Energy logistics refresh — 2026-07-15

Branch: `feat/physical-energy-logistics`

The physical logistics implementation replaces ordinary Energy trading with exact delivery contracts, typed owned/locked bulk, deterministic recovery, archetype-aware commercial hauling, exhaustive D10 attribution, and app/TUI/CLI observability.

### Current automated evidence

Routine workspace tests, workspace Clippy with warnings denied, formatting, content validation, and diff checks pass. Focused gates cover full/partial/zero settlement, deadline timeout, recovery return/curtailment, allocation exactly-once behavior, source claims, insertion permutations, stale intents, profitability, dynamic archetype selection, all D10 causes, immutable app requests/views, and both supported TUI layouts.

The refreshed release 1,000-tick deterministic acceptance passed and printed:

```text
1000-tick acceptance: reconciliation_difference=0 stage_transitions=13 population_changes=1 energy_loaded=5120 energy_delivered=4937 contracts_accepted=3 contracts_completed=3 contracts_recovered=0 starvation_attributions=0 unsupplied_destination_ticks=0 trades_after_300=4563 production_after_300=975
```

The test executes the complete run twice and requires identical events, final snapshots, metrics, contract activity, D10 attribution totals, continuing ordinary trade/production, and exact physical reconciliation including market stock, tanks, owned bulk, and locked bulk.

The controlled five-tick player-impact unit fixture passes with a one-Energy intervention at a deliberately constructed runway boundary and exits as soon as the first stage divergence is observed. This replaced the formerly slow routine 500-tick fixture without weakening its identical-state, typed-delivery, bounded-divergence, or dual-reconciliation assertions.

Opportunity prefiltering reduced the warm deterministic 50-tick repository smoke from roughly 91 seconds to 5.45 seconds. Per explicit user direction, the 10,000-tick soak was not rerun for this refresh; the historical result above is not claimed as current physical-logistics evidence. Current long-run confidence is therefore bounded by the passing 1,000-tick gate and updated 10,000-tick validator.

App actor tests and Ratatui `TestBackend` traces replace a live-terminal manual session in the non-interactive environment. They cover request submission and phase-11 resolution, exact bulk deposit/transfer, resolved contract names, payload and route burns, fee/profit/net/freight/runway, locked ownership, deadline/blocker text, and compact/regular rendering.
