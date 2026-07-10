# Initial Prototype Validation Evidence

Date: 2026-07-10  
Branch: `loop/20260710-initial-prototype-implementation`

## Automated checks

The following commands pass locally:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo run -p game-cli -- --validate-content
cargo run -p game-cli -- --headless
```

GitHub Actions run [29129343977](https://github.com/aefreedman/4x-term/actions/runs/29129343977) passed the configured format, check, Clippy, test, content-validation, and headless jobs before the final coverage pass. A final CI run is required after the remaining commits are pushed.

## Pseudo-TTY interaction smoke test

An Expect-controlled PTY launched `target/debug/game-cli`, answered Crossterm's cursor-position query, and exercised:

- Single step
- Quantity entry (`2`)
- Buy
- System selection
- Begin travel
- Continuous run and pause
- Help open/close
- Clean quit

The process exited successfully. Captured output contained both cursor restoration (`CSI ?25h`) and alternate-screen exit (`CSI ?1049l`) sequences.

A forced cursor-query timeout was also exercised. The process returned an error and still emitted cursor and alternate-screen restoration sequences, validating the recoverable-error cleanup path.

Resize and constrained-layout behavior are covered through Ratatui `TestBackend` tests at normal, minimum, narrow, and short dimensions because the non-interactive Expect host cannot reliably resize its child PTY.

## Headless acceptance

`crates/game-cli/tests/boundaries.rs` loads repository content and drives the player through a multi-hop trade using the same core session as the TUI:

1. Buy Ferrite Ore at System 01.
2. Start a route to nonadjacent System 20.
3. Confirm trading is rejected in transit.
4. Step until arrival.
5. Sell the cargo and verify ledger/inventory state.

The workspace tests additionally cover deterministic economy activity, every processing layer, automated nonadjacent trade selection, pricing monotonicity, transaction conservation/rejections, timer behavior, channel bounds, view projections, input mapping, and terminal cleanup.
