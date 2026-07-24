---
title: "Terminal UX Review Checklist"
type: review-checklist
status: active
authority: procedural
date: 2026-07-21
---
# Terminal UX Review Checklist

## Purpose

This checklist turns the committed [Terminal Experience](design/direction/terminal-experience.md) principles and approved [current terminal interactions](design/current/terminal-interactions.md) into reviewable evidence. It applies to every keyboard-driven surface, including temporary drafts and overlays.

The linked design pages own experience direction and current behavior. This page owns review procedure and does not override either contract.

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

A blank or “implicit” answer is a design gap. Once approved, current behavior belongs in [Terminal Interactions](design/current/terminal-interactions.md) rather than being duplicated here.

## Review process

### 1. Static coherence review

Compare the approved interaction table with rendering and routing code. Reject the surface if focus styling, key hints, and action handling disagree, even when each works independently.

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
