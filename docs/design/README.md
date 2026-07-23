---
title: Game Design
type: design-index
status: active
authority: normative
horizon: current
tags:
  - design-governance
  - navigation
---
# Game Design

`docs/design/` is the source of truth for game design. Directory and metadata determine what kind of truth a page contains; a plan, todo, or implementation does not silently override it.

## Authority scopes

| Scope | Question answered | Authority |
| --- | --- | --- |
| [Current](current/README.md) | What is the approved mechanical contract now? | Normative |
| [Direction](direction/README.md) | What committed outcomes constrain future design? | Directional; not proof of implementation |
| [Lore](lore/README.md) | What setting context is canonical? | Canonical context; does not imply mechanics |
| [Ideas](ideas/README.md) | What might be explored? | Non-authoritative |

Drafts are prohibited from `current/`. Other scopes may contain drafts, but `status: draft` is never authoritative.

## How to use discrepancies

Use current design to describe approved behavior and direction to evaluate its long-term fit. When they differ, determine whether the current state is a pragmatic stage, the direction is awaiting implementation, or one side exposes a defect. Surface that classification for review instead of silently choosing. Lore cannot create an unstated mechanic. Ideas cannot be implementation requirements.

## Promotion and implementation workflow

An idea may be promoted into direction when its outcome becomes a commitment. It moves into current only when its mechanical contract is approved. Direct idea-to-current promotion is allowed when appropriate, but must still be explicit.

Implementation plans describe execution rather than design authority. A plan must identify applicable current and direction updates. Make those updates after implementation review and before merge approval so the design hierarchy records the accepted result.

## Numeric authority

Mutable shipped tuning values are owned by same-repository configuration, such as [`content/profiles/starter.ron`](../../content/profiles/starter.ron). Design docs explain semantics, relationships, reviewed constraints, and rationale without copying mutable value tables. Reviewed generator ranges remain design decisions, and revision-frozen constants and test vectors remain exact in the applicable generator-revision page.

## Related project references

- [Architecture](../architecture.md)
- [Testing stance](../plans/2026-07-20-testing-stance-correction.md)
- [Engine invariant registry](../2026-07-20-engine-invariant-registry.md)
