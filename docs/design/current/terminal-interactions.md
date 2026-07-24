---
title: Terminal Interactions
type: design-contract
status: active
authority: normative
horizon: current
founded: 2026-07-21
tags:
  - terminal
  - interaction
  - tui
  - player-interface
---
# Terminal Interactions

## Purpose

This page records approved current player-facing behavior for the terminal dashboard, local management, and probe planning. It is the current contract, not a complete inventory of widgets or implementation structure.

All current surfaces are expected to satisfy the durable [Terminal Experience](../direction/terminal-experience.md) principles. New or changed surfaces must complete the [Terminal UX Review Checklist](../../tui-ux-guidelines.md).

## Dashboard and local management

| Item | Dashboard | Local management |
| --- | --- | --- |
| Entry | Start the current preview, or press Esc from a child surface | Activate Manage for a commandable system |
| Exit | Quit confirmation for the live session | Esc returns to the dashboard |
| Visible focus | System list | Bodies / Slots list |
| Selection | Last selected visible system row | First visible slot on entry; retained visible slot after refresh |
| Directions | Up/Down traverses system rows; map and detail synchronize read-only | Up/Down traverses visible slot rows across read-only body headings |
| Tab order | None; only one interactive focus target | None; only one interactive focus target |
| Enter | `[Enter Manage]` opens controllable systems directly; `[Enter Details]` opens received knowledge for read-only systems | Contextual commands use their named keys; Enter is not advertised |
| Unavailable case | Read-only details remain browsable; management is offered only for commandable local systems | Unavailable Build/Habitat actions are omitted or explained in detail |
| Refresh | Selected system remains visible and valid | Selected system and slot remain visible and valid |

The dashboard map and summary panels synchronize with the selected system but remain read-only and are not styled as independent focus targets. In local management, only slot rows receive selection markers; body headings remain read-only structure.

## Probe planning

Probe planning treats target and calculated route as the primary decision. It uses the probe's maximum capability automatically. Typed numbers create an optional maximum-jump-per-leg override.

An edited override must be explicitly applied before launch review. Applying it refreshes the route, Energy requirement, and availability together. Until then, the reviewed route continues to represent the previously applied value rather than uncommitted editor text.

## Current unavailable and refresh behavior

Unavailable inspection or management remains in the current context with an admitted reason; it does not open a dead-end surface. Entering a child surface or changing a parent selection establishes a valid visible child selection. View refresh preserves selected stable system or slot identity when it remains admitted and valid, and otherwise chooses a valid visible fallback.

Exact keyboard-layout mappings, terminal-size safety, pacing controls, editor precedence, and other implemented presentation behavior remain documented by the completed Stage 5 plans and executable help until a current contract needs to own them explicitly.
