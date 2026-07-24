---
title: Terminal Experience
type: design-direction
status: active
authority: directional
horizon: long-term
founded: 2026-07-21
tags:
  - terminal
  - interaction
  - accessibility
  - tui
---
# Terminal Experience

## Purpose

The terminal is a keyboard-first human play surface, not merely a diagnostic adapter. Its interaction model must make focus, navigation, availability, commitment, and recovery predictable before the player presses a key.

These principles constrain future terminal surfaces without claiming that any particular screen or control is implemented. The approved current behavior is recorded in [Terminal Interactions](../current/terminal-interactions.md), and surfaces are evaluated with the [Terminal UX Review Checklist](../../tui-ux-guidelines.md).

## Interaction principles

1. **One visible focus.** Exactly one component owns directional input. Its border/title treatment must agree with the component containing the selected row. Read-only synchronized panels are never styled as focused.
2. **Visible effects only.** A navigation event may change only state represented on the current surface. It must not move a cursor in a hidden list or retain an invalid child cursor after changing its parent.
3. **Spatially honest directions.** Up/Down traverses vertically presented rows; Left/Right traverses horizontal choices or an explicitly pictured hierarchy. Directions are not assigned merely because they are unused.
4. **Focus and selection are distinct.** Focus identifies the component that receives input. Selection identifies one row inside it. Nested parent and child rows do not both use the same selection marker.
5. **Tab means focus traversal.** Tab/Shift-Tab appears in hints and help only when a composition has at least two interactive focus targets with a defined order. It never substitutes for ordinary list traversal.
6. **Activation matches its label.** A contextual label names the resulting surface or action. An action is not labelled “Details” when details are already visible or when it opens a management screen.
7. **Unavailable transitions remain in context.** If inspection or management is unavailable, the surface shows the reason before activation and does not navigate to a dead end.
8. **Hints are executable contracts.** A focused panel owns its contextual action bar. Every shown action works in that context, and every discoverable primary action is shown there. The global bar contains only stable globals.
9. **Screen changes establish valid context.** Changing a parent selection or entering a child surface establishes a valid, visible child selection.
10. **No memorized mode grammar.** Reusing a key with contextual meaning is acceptable only when focus, geometry, title, and hints make the meaning predictable before the key is pressed.

## Experience outcomes

A conforming terminal flow should let the player:

- predict navigation from visible geometry;
- distinguish focus, selection, availability, and activation;
- understand what an action will open or commit;
- recover with Esc without losing unrelated state;
- see every accepted navigation effect in the next frame;
- retain valid context as admitted game information changes; and
- operate the primary game loop without prior explanation of hidden control modes.

When a current surface conflicts with these principles, classify the conflict as an implementation defect, an intentional staged limitation, or a requested direction change. Do not silently weaken the principle or rewrite the current contract to hide the discrepancy.
