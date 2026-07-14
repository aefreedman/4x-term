# Mode terminal UI validation evidence

Generated 2026-07-14 from Ratatui `TestBackend` buffers and real pseudo-terminal keyboard runs.

## Text-buffer captures

Compact (`80x30`) and regular (`160x45`) captures are included for Systems, Trade, Governance, and Intelligence. Additional Systems captures cover `159x44` and `200x60`.

The captures demonstrate:

- persistent F1–F4 activity bar and textual active marker;
- stable `>` selection plus `LOC`, `GOV`, and warning labels;
- local Trade versus remote read-only inspection;
- explicit route proposal or disabled reason;
- Governance row selection, allocation totals, and read-only labeling;
- Intelligence event range and player/fleet summaries;
- compact and regular compositions without internal content IDs.

## Keyboard and resize playthrough

Two real terminal runs completed with exit status 0:

1. `80x30`: Systems navigation and sorting, Trade quantity cancel, Governance navigation/edit attempt, Intelligence scrolling, help open/close, resize to `160x45` and back to `80x30`, then quit.
2. `160x45`: visited all four activities with keyboard-only navigation, then quit.

The pseudo-terminal logs were retained only under ignored `target/ui-playthroughs/`; they contain ANSI control sequences and are not source artifacts.

## Review notes

- No hidden-target mutation was observed; obsolete punctuation shortcuts are covered as inert by automated tests.
- Local/remote target labels remained explicit during mode changes.
- Compact footer text truncates before nonessential trailing status, while selected-action availability remains visible in the main surface.
- No target mistakes or missing critical action state were observed in the exercised flow.
- Save/content compatibility is unaffected; this change restructures immutable presentation data and TUI-local state only.
