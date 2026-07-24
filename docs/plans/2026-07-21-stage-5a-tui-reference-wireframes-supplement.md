---
title: "Stage 5a TUI Planning Supplement — Reference Wireframes"
type: plan-supplement
status: approved
date: 2026-07-21
approved: 2026-07-21
source: "2026-07-21-stage-5a-tui-design-foundation-supplement.md"
---
# Stage 5a TUI Planning Supplement — Reference Wireframes

## Reading the wireframes

Each frame is exactly `160x45` ASCII cells. The panels use the complete workspace from rows 1–43; row 44 contains only stable global actions. Focused panel prompts appear on the panel's bottom interior row as button-like actions. Example labels and quantities are fixture data, not production balance promises. These reference compositions, component edge states, multi-tick behavior, and correctable-rejection behavior are approved in the design foundation.

See the [design foundation](2026-07-21-stage-5a-tui-design-foundation-supplement.md) for component, interaction, knowledge, and application-view contracts.

### Navigation coherence amendment

The dashboard and local compositions retain the reference geometry, but their original duplicated action hints are superseded by the active interaction rules. On the dashboard, only the System list is focused: Up/Down selects its visible rows, the map and summary panels synchronize read-only, and Enter opens controllable systems directly in local management while opening a browsable detail surface for read-only systems. In local management, only slot rows receive selection markers and Up/Down traverses those rows across body headings. Tab is not advertised where there is only one interactive focus target. See the approved [current Terminal Interactions](../design/current/terminal-interactions.md).

## A. Startup and world preview

```text
4X-TERM / NEW FRONTIER                                                                                                                                          
+- NEW WORLD ----------------------------------------------++- WORLD PREVIEW ----------------------------------------------------------------------------------+
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|  Profile                                                 ||  Seed       18446744073709551615                                                                 |
|  > starter                                               ||  Profile    starter                                                                              |
|    content/profiles/starter.ron                          ||                                                                                                  |
|                                                          ||  ORIGIN                                                                                          |
|  Seed                                                    ||  Origin Community              Population 0                                                      |
|  > 18446744073709551615                                  ||                                                                                                  |
|                                                          ||  Stocks                                                                                          |
|  Choose a profile and seed, then generate                ||  Energy                         10                                                               |
|  a new frontier.                                         ||  Ore                            10                                                               |
|                                                          ||  Alloy                           0                                                               |
|                                                          ||                                                                                                  |
|                                                          ||  Bodies                          4                                                               |
|                                                          ||                                                                                                  |
|                                                          ||  Infrastructure                                                                                  |
|                                                          ||  Collector             functional                                                                |
|                                                          ||  Battery               functional                                                                |
|                                                          ||  Extractor             functional                                                                |
|                                                          ||  Refinery              functional                                                                |
|                                                          ||                                                                                                  |
|                                                          ||  The origin is the only living community.                                                        |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|                                                          ||                                                                                                  |
|  [Enter Generate] [Tab Next Field]                       ||  [Enter Start] [r Regenerate] [Esc Back]                                                         |
|                                                          ||                                                                                                  |
+----------------------------------------------------------++--------------------------------------------------------------------------------------------------+
[? Help] [F2 Settings] [q Quit]                                                                                                                                 
```

## B. Main-play dashboard

```text
4X-TERM / ORIGIN                                      Tick 24   Season 5/10                                                                                     
+- FRONTIER ---------------------------------------------------------------------------------------++- SYSTEMS ------------------++- ORIGIN -------------------+
|                                                                                                  ||                            ||                            |
|                                                                                                  ||                            ||                            |
|                                                                                                  ||  > @ Origin                ||  Population            0   |
|                                                                                                  ||      FSC 000004            ||  Bodies                4   |
|                                              .                                                   ||      FSC 000017    --      ||  Command          Origin   |
|           .                                                                                      ||      FSC 000031    --      ||                            |
|                                                                                                  ||                            ||  Stocks                    |
|                                                                                         .        ||    Uncharted: 12           ||  Energy               34   |
|                                                                                                  ||                            ||  Ore                  18   |
|                                                                                                  ||    1 / 11                  ||  Alloy                 2   |
|                        *                                                                         ||                            ||                            |
|                                                                                                  ||                            ||                            |
|                                                                                                  ||                            ||                            |
|                                                                     *                            ||                            ||                            |
|                                                                                                  ||                            ||                            |
|                                                                                                  ||                            ||                            |
|                                                                                                  ||                            ||                            |
|                                                     .                                            ||                            ||  [Enter Details]           |
|                                                                                                  ||                            ||                            |
|                                                                                                  ||                            |+----------------------------+
|                                                                                                  ||                            |+- ENERGY -------------------+
|                                                                                                  ||                            ||                            |
|                                 @                                                                ||                            ||                            |
|                                                                                                  ||                            ||  Current        34 / 110   |
|                                                                                                  ||                            ||  Headroom             76   |
|                                                                                                  ||                            ||  Season             5/10   |
|                                                                                                  ||                            ||                            |
|                                                                                 *                ||                            ||  Last tick                 |
|                                                                                                  ||                            ||  Life support          0   |
|                                                                                                  ||                            ||  Paid / unpaid     0 / 0   |
|                                                                                                  ||                            ||  Supported / short 0 / 0   |
|                                                                                                  ||                            ||  Overflow              6   |
|                 .                                                                                ||                            ||                            |
|                                                                                                  ||                            ||                            |
|                                                            *                                     ||                            ||                            |
|                                                                                                  ||                            ||                            |
|                                                                                                  ||                            ||                            |
|                                                                                                  ||                            ||                            |
|                                                                                                  ||                            ||                            |
|  [Arrows/u/n/e/i Select] [Enter] [r Rename]                                                      ||  [Enter] [r Rename]        ||  [Enter Energy Details]    |
|                                                                                                  ||                            ||                            |
+--------------------------------------------------------------------------------------------------++----------------------------++----------------------------+
[. Advance Tick] [t Advance Many] [? Help] [F2 Settings] [q Quit]                                                                                               
```

## C. Slot-initiated construction rejection

```text
4X-TERM / ORIGIN / DEVELOPMENT                        Tick 24   Season 5/10                                                                                     
+- BODIES / SLOTS -----------------------------------------++- DEVELOPMENT ----------------------------------++- COST -----------------------------------------+
|                                                          ||                                                ||                                                |
|                                                          ||                                                ||                                                |
|  Origin Body 0                                           ||  Role                                          ||  Extractor on Origin Body 0                    |
|    slot_0  Collector                                     ||                                                ||                                                |
|  > slot_1  empty                                         ||    Collector                                   ||  Energy                    10                  |
|    slot_2  Battery                                       ||    Battery                                     ||  Alloy                       2                 |
                      +- COULD NOT START CONSTRUCTION -----------------------------------------------------------------------------------+                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |   Not enough Alloy.                                                                                              |                      
                      |                                                                                                                  |                      
                      |   Available                              0                                                                       |                      
                      |   Required                               2                                                                       |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |                                                                                                                  |                      
                      |   [Enter Edit Draft] [Esc Back]                                                                                  |                      
                      |                                                                                                                  |                      
                      +------------------------------------------------------------------------------------------------------------------+                      
|                                                          ||                                                ||                                                |
|                                                          ||                                                ||                                                |
|  [Enter Build Here] [Esc Back]                           ||  [Enter Review] [Esc Back]                     ||  [Enter Review] [Esc Back]                     |
|                                                          ||                                                ||                                                |
+----------------------------------------------------------++------------------------------------------------++------------------------------------------------+
[? Help] [F2 Settings] [q Quit]                                                                                                                                 
```

## D. Manual multi-tick advancement

```text
4X-TERM / ORIGIN                                      Tick 24   Season 5/10                                                                                     
+- FRONTIER ---------------------------------------------------------------------------------------++- SYSTEMS ------------------++- ORIGIN -------------------+
|                                                                                                  ||                            ||                            |
|                                                                                                  ||                            ||                            |
|             +- ADVANCE 10 TICKS ---------------------------------------------------------------------------------------------------------------+             |
|             |                                                                                                                                  |             |
|             |                                                                                                                                  |             |
|             |   Completed 4 of 10          Rate 5/sec          Stopped                   +- TICK 23 ---------------------------------------+   |             |
|             |                                                                            |                                                 |   |             |
|             |   TICK   CHANGES                                                           |                                                 |   |             |
|             |     21   Energy +10      Extractor 1/4                                     |   Energy                         34 / 110       |   |             |
|             |     22   Energy +10      Extractor 2/4                                     |   Headroom                             76       |   |             |
|             |   > 23   Energy +10      Extractor 3/4                                     |                                                 |   |             |
|             |     24   Extractor completed      Overflow 6                               |   Life support                          0       |   |             |
|             |                                                                            |   Paid / unpaid                     0 / 0       |   |             |
|             |                                                                            |   Supported / short                 0 / 0       |   |             |
|             |                                                                            |   Overflow                               0      |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            |   Construction                                  |   |             |
|             |                                                                            |   Extractor                            3/4      |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            |                                                 |   |             |
|             |   Stopped after tick 24.                                                   |                                                 |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            |                                                 |   |             |
|             |                                                                            +-------------------------------------------------+   |             |
|             |                                                                                                                                  |             |
|             |                                                                                                                                  |             |
|             |                                                                                                                                  |             |
|             |                                                                                                                                  |             |
|             |   [u/e Select Tick] [Enter Close] [Esc Close]                                                                                    |             |
|             |                                                                                                                                  |             |
|             +----------------------------------------------------------------------------------------------------------------------------------+             |
|                                                                                                  ||                            ||                            |
+--------------------------------------------------------------------------------------------------++----------------------------++----------------------------+
[? Help] [F2 Settings] [q Quit]                                                                                                                                 
```

## Component edge-state references

- Invalid form fields preserve entered text and show a concise diagnostic in the owning panel.
- Empty lists retain their panel and say what is absent.
- Overflowing lists keep selection visible and show hidden-row count near the list, not in a global status footer.
- Unavailable actions remain visible only when the reason helps the player.
- A charted system list exposes `[r Rename]`; the alias editor keeps the stable `FSC NNNNNN` label visible and offers Apply, Clear, and Back actions.
- A running multi-tick panel exposes Space Pause and Esc Stop; while paused it exposes Space Resume, Enter Step, and Esc Stop.
- Unknown values use `--`; zero is reserved for an observed exact zero.
- Long labels truncate in collections and appear in full in selected detail.
- Exact quantities remain unabridged and right-aligned.
- The undersized view states required/current dimensions and exposes Settings, resize recovery, and session-sensitive Quit without gameplay actions.

## Review notes

The map is a full panel, not a graph or explanatory diagram. It renders only application-provided map visuals, player-known chart points, and current active-ship positions. A visual's pivot stays within four map units of its actual system and remains after discovery; the exact point overlays one cell as `*` or selected `@`. A ship is a yellow `+`; it carries no route, direction, type, or progress detail. Identified systems without an observed position stay in the synchronized system list with `--`; uncharted systems remain only an aggregate count.

The startup preview contains only seed, profile, and origin-facing gameplay information. Keyboard mode belongs to global user settings. Generator revision, fingerprint, provenance, canvas dimensions, and other debug/reproduction data do not appear in the human interface.
