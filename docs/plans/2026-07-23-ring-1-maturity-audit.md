---
title: Ring 1 Maturity Audit
type: design-audit
status: recorded
date: 2026-07-23
tags:
  - core-loop
  - tui
  - prioritization
  - audit
---
# Ring 1 Maturity Audit

## Purpose

This audit applies the [Concentric Development](../design/direction/concentric-development.md) model to the existing game. It inventories evidence; it does not silently change current mechanics, promote ideas, or declare the core loop strategically successful.

## Audit boundary

Ring 1 requires the complete playable cycle:

```text
bootstrap -> produce/survive -> bank/develop -> scout
-> prepare/launch -> found -> receive outcome -> continue governing
```

The audit distinguishes technical availability from dependability. Existing implementation plans and code provide strong evidence that the complete path exists. Whether its decisions remain meaningful and legible over repeated full-cycle play is a separate product question.

## Evidence reviewed

- current contracts indexed by [Current Design](../design/current/README.md);
- the completed [Stage 4b bounded-expansion plan](2026-07-20-feature-constructive-world-generation-stage-4b-plan.md);
- the completed and re-audited [Stage 5 playable-startup plan](2026-07-21-feature-playable-startup-stage-5-plan.md);
- [Terminal Experience](../design/direction/terminal-experience.md), [current Terminal Interactions](../design/current/terminal-interactions.md), and the [Terminal UX Review Checklist](../tui-ux-guidelines.md);
- TUI state, rendering, and focused interaction tests under `crates/game-tui/src/`;
- application intents, views, assessments, and tests under `crates/game-app/src/`; and
- deterministic simulation coverage under `crates/game-core/tests/` and `crates/game-core/src/stage5_boundary_tests.rs`.

The Stage 5 implementation audit records successful formatting, checking, Clippy, all-feature tests, and extensive manual playtesting as of 2026-07-21. A fresh test run was attempted for this audit, but `cargo` is unavailable in the current agent environment; this document therefore cites that recorded validation rather than claiming a new run.

## Maturity inventory

Legend:

- **strong** — direct current contract plus implementation/player/test evidence;
- **recorded** — prior implementation audit or playtest records the evidence;
- **partial** — the path exists, but the new concentric criterion asks a broader question;
- **open** — requires present full-cycle play and judgment rather than repository inspection alone.

| Ring 1 domain | Contracted | Implemented | Exposed in TUI | Legible | Verified | Dependable |
| --- | --- | --- | --- | --- | --- | --- |
| Generate and start a world | strong | strong | strong | recorded | strong | open |
| Bootstrap population and survive seasonal Energy pressure | strong | strong | strong | recorded | strong | open |
| Inspect stocks, developments, commitments, and limiting reasons | strong | strong | strong | recorded | strong | open |
| Bank or develop physical margin | partial | strong | strong | partial | strong mechanically | open strategically |
| Construct and operate Shipyards | strong | strong | strong | recorded | strong | open |
| Scout, travel, observe, and receive delayed reports | strong | strong | strong | recorded | strong | open |
| Prepare, launch, and resolve an expedition | strong | strong | strong | recorded | strong | open |
| Found and unlock a remote system after delayed outcome receipt | strong | strong | strong | recorded | strong | open |
| Continue governing origin and founded systems through the TUI | strong | strong | strong | recorded | strong | open |
| Understand the complete cycle as one repeated strategy loop | directional | implemented as composed parts | exposed as composed parts | partial | no single scenario required | open |

## Findings

### 1. The full technical cycle is present

The current contracts cover every required physical and information transition. Stage 4b records the headless implementation. Stage 5 records TUI exposure and manual acceptance for bootstrap, scouting, expeditions, delayed outcomes, founding, and daughter-system commandability.

Repository evidence agrees with those records: the TUI dispatches construction, Habitat, tick, probe, expedition, and launch intents; renders awaiting, founded, and loss states; and receives player-safe application views. Core tests cover deterministic routing, knowledge, travel, founding, loss, global phase order, and atomicity.

### 2. TUI exposure is part of Ring 1 and is substantially present

The new model does not permit implemented-but-inaccessible simulation mechanics to count as primary completion. Stage 5 explicitly exposes the full scouting and founding loops, and the terminal guidelines define interaction-review standards. The remaining question is not whether screens and commands exist, but whether a player can understand the complete cycle without relying on implementation knowledge.

### 3. Mechanical verification is stronger than strategic validation

Existing deterministic tests appropriately prove mechanical contracts. They do not and should not prove that a generated seed is fun, viable, or strategically rich. Prior manual playtesting accepted the component journeys, but it predates the explicit Ring 1 question: do bank, develop, and expand produce meaningful competing commitments across repeated full-cycle play?

### 4. Banking is the least explicit primary choice

Development and expansion create physical commitments through established commands. Banking currently occurs by declining, delaying, cancelling where allowed, or disabling eligible activity under fixed spending order. The [Margin and Energy Allocation idea](../design/ideas/margin-and-energy-allocation.md) exists because the current model may not make reserve intent sufficiently explicit.

This is not yet evidence that a reserve policy is required. It identifies the most important question for playtesting: can the player intentionally preserve margin, understand what is protected, and later explain why banking was preferable to development or expansion?

If not, margin allocation is a Ring 1 design or legibility repair even though a particular reserve-floor mechanism remains exploratory.

### 5. Dependability remains an experiential sign-off

The repository supports a provisional conclusion of **player-complete, decision-quality validation pending**. Declaring Ring 1 dependable requires current repeated play, not another implementation inventory.

## Recommended first internal milestone

### Ring 1 dependable baseline

**Type:** foundation

**Player outcome:** A player can complete and repeat the full bootstrap-to-founding cycle through the TUI, understand the physical and informational consequences, and identify credible reasons to bank, develop, or expand at multiple points.

**Work:**

1. Run several goal-directed playtests across generated worlds without treating any seed outcome as a quality gate.
2. Record each consequential bank/develop/expand decision, the evidence visible at the time, the expected consequence, and the observed consequence.
3. Record interaction friction, hidden or misleading commitments, unexplained pauses, and places where implementation knowledge was needed.
4. Classify each finding as a Ring 1 defect, a legibility defect, a tuning/content observation, or a possible outer-ring opportunity.
5. Repair inner defects before generalizing outer mechanics.
6. Use a bounded vertical slice only when it names a hypothesis that cannot be answered from the existing loop.

**Exit evidence:**

- the full cycle is completed through the ordinary TUI without privileged state inspection;
- primary actions and unavailable states are discoverable and attributable;
- the player can explain consequential allocation decisions from information visible when they were made;
- repeated play no longer reveals foundational interaction failures;
- mechanical defects receive deterministic Tier 1 regression coverage; and
- remaining requests are explicitly classified as inner repairs, secondary depth, or tertiary breadth.

## Roadmap consequence

Do not choose the first feature release solely from the existing idea list. First use the Ring 1 dependable-baseline milestone to decide whether the next work is:

- a foundation repair, such as making banking or consequence feedback explicit; or
- a bounded vertical slice through a dependable primary interaction.

After that decision, create the mutable roadmap outside `docs/design/direction/` and link its milestones back to this audit and the concentric model.
