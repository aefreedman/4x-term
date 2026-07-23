---
title: "Terminal UX Guidelines and Review Checklist"
type: design-guideline
status: active
date: 2026-07-21
---
# Terminal UX Guidelines and Review Checklist

## Purpose

This document turns the terminal design foundation's focus and navigation goals into reviewable interaction rules. It applies to every keyboard-driven surface, including temporary drafts and overlays.

## Interaction rules

1. **One visible focus.** Exactly one component owns directional input. Its border/title treatment must agree with the component containing the selected row. Read-only synchronized panels are never styled as focused.
2. **Visible effects only.** A navigation event may change only state represented on the current surface. It must not move a cursor in a hidden list or retain an invalid child cursor after changing its parent.
3. **Spatially honest directions.** Up/Down traverses vertically presented rows; Left/Right traverses horizontal choices or an explicitly pictured hierarchy. Do not assign directions merely because they are unused.
4. **Focus and selection are distinct.** Focus identifies the component that receives input. Selection identifies one row inside it. Nested parent and child rows must not both use the same selection marker.
5. **Tab means focus traversal.** Tab/Shift-Tab appears in hints and help only when a composition has at least two interactive focus targets with a defined order. It never substitutes for ordinary list traversal.
6. **Activation must match its label.** Enter's contextual label names the resulting surface or action. Do not label an action “Details” when details are already visible or when it opens a management screen.
7. **Unavailable transitions remain in context.** If inspection or management is unavailable, show the reason before activation and do not navigate to a dead-end screen.
8. **Hints are executable contracts.** A focused panel owns its contextual action bar. Every shown action works in that context, and every discoverable primary action is shown there. The global bar contains only stable globals.
9. **Screen changes reset dependent cursors.** Changing a parent selection or entering a child surface establishes a valid, visible child selection.
10. **No memorized mode grammar.** Reusing a key with contextual meaning is acceptable only when focus, geometry, title, and hints make the meaning predictable before the key is pressed.

## Required interaction table

Before implementing or approving a surface, record:

| Item | Required answer |
| --- | --- |
| Entry | Which action opens the surface? |
| Exit | What does Esc return to without committing? |
| Visible focus | Which component has the focus marker? |
| Selection | Which visible row is selected initially? |
| Directions | What visible change does each accepted direction produce? |
| Tab order | Which interactive components are traversed, or why is Tab absent? |
| Enter | Exact contextual label and resulting state/surface |
| Unavailable case | Visible reason and retained context |
| Refresh | Which focus, selection, and draft state survives a view refresh? |

A blank or “implicit” answer is a design gap.

## Dashboard interaction table

| Item | Dashboard | Local management |
| --- | --- | --- |
| Entry | Start current preview or Esc from a child surface | Enter from a commandable system's detail surface |
| Exit | Quit confirmation for the live session | Esc returns to dashboard |
| Visible focus | System list | Bodies / Slots list |
| Selection | Last selected visible system row | First visible slot on entry; retained visible slot after refresh |
| Directions | Up/Down traverses system rows; map/detail synchronize read-only | Up/Down traverses visible slot rows across read-only body headings |
| Tab order | None; only one interactive focus target | None; only one interactive focus target |
| Enter | `[Enter Manage]` opens controllable systems directly; `[Enter Details]` opens received knowledge for read-only systems | Contextual commands use their named keys; Enter is not advertised |
| Unavailable case | Read-only details remain browsable; management is offered only for commandable local systems | Unavailable Build/Habitat actions are omitted or explained in detail |
| Refresh | Selected system remains visible and valid | Selected system and slot remain visible and valid |

Probe planning treats target and calculated route as the primary decision. It uses the probe's maximum capability automatically; typed numbers create an optional maximum-jump-per-leg override. Edited overrides must be explicitly applied so route, Energy, and availability are refreshed before launch review.

## Review process

### 1. Static coherence review

Compare the interaction table with rendering and routing code. Reject the surface if focus styling, key hints, and action handling disagree, even when each works independently.

### 2. Prediction walkthrough

At the minimum supported terminal size, a reviewer unfamiliar with the implementation states what they expect each shown key to do before pressing it. Record any mismatch as a UX defect; do not explain the control scheme first. The walkthrough must include forward navigation, backtracking, unavailable content, and at least one parent/child list.

### 3. No-hidden-state probe

For every accepted navigation key, verify that the next frame visibly changes focus or selection. Also verify that off-screen selections and unrelated parent identities remain unchanged.

### 4. Event-loop acceptance

Exercise representative key sequences through the real event loop, not only by calling state actions directly. Allow at least one pacing/redraw cycle after opening each overlay or changing screens. This catches lifecycle bugs that state and render tests in isolation cannot detect.

### 5. Automated evidence

Focused TUI tests should prove:

- one focused panel and one selected row in representative renders;
- contextual hints match the active screen;
- directions traverse rows in displayed order, including boundaries;
- local navigation cannot change a hidden system selection;
- unavailable activation does not enter a dead-end surface;
- parent changes establish valid child selections; and
- overlays and drafts survive redraw/pacing cycles.

Use semantic assertions rather than full-screen snapshots, but test complete interaction sequences where bugs can occur between independently correct layers.

## Review sign-off questions

- Can a player predict every displayed navigation key from the geometry alone?
- Does every key affect the panel visibly marked as focused?
- Can any key mutate a selection that is not on screen?
- Are focus, selection, and activation represented by distinct cues?
- Does Enter's label describe what appears next?
- Can the player recover with Esc without losing unrelated state?
- Did an independent reviewer complete the flow without prior explanation?
