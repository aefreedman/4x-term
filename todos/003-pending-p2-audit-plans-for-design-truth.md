---
status: pending
priority: p2
issue_id: 003
tags:
  - design
  - documentation
  - audit
dependencies: []
---
# Audit implementation plans for retained design truth

## Problem Statement

The design hierarchy under `docs/design/` is now authoritative, but completed
implementation plans may still contain approved mechanical contracts that were
never extracted into `docs/design/current/`. Plans are execution history and
must not remain the only source for current behavior.

## Findings

- The documentation consolidation established current, direction, lore, and idea
  authority scopes.
- The consolidation intentionally did not audit every file under `docs/plans/`.
- The previously known dependency in `tuning-profiles.md` was resolved during
  consolidation by linking focused current pages and configuration.
- Other plan-only contracts may remain and require a bounded audit.

## Proposed Solution

**Approach:**
- Audit completed implementation plans against the canonical ownership table in
  `docs/design/current/README.md`.
- Extract only still-current approved contracts, reconcile conflicts with code
  and current design, and replace plan dependencies with canonical links.

**Why this approach:**
- It makes design authority self-contained without rewriting implementation
  history.

**Trade-offs / risks:**
- Historical plans may describe superseded behavior; extraction requires
  evidence and owner review rather than copying prose wholesale.

## Recommended Action

Start with plans cited by current design or code-facing documentation, record
candidate contracts by canonical owning page, and review ambiguous or conflicting
items before editing current design.

## Technical Details

**Affected files/assets:**
- `docs/plans/` - historical implementation evidence to audit
- `docs/design/current/` - canonical destinations for retained contracts

**Related systems:**
- Design documentation governance
- Compound Game Dev artifact indexing

**Data/content impact:**
- Save data affected? No
- Serialized assets or prefabs affected? No
- Migration or content reimport needed? No

## Resources

- **Documentation:** `docs/design/README.md`
- **Documentation:** `docs/design/current/README.md`
- **Plan:** `docs/plans/2026-07-22-design-documentation-consolidation-plan.md`

## Acceptance Criteria

- [ ] Completed plans are searched for normative statements not represented in current design.
- [ ] Every retained candidate is assigned to one canonical current-design owner.
- [ ] Superseded plan behavior is not promoted.
- [ ] Ambiguous conflicts receive owner review.
- [ ] Current pages become self-contained and plan citations remain evidence only.
- [ ] Metadata and relative-link validation pass after updates.

## Work Log

### 2026-07-22 - Todo created

**By:** Pi coding agent

**Actions:**
- Recorded the deferred plan-extraction audit from the documentation consolidation.

**Learnings:**
- The original tuning-profile dependency was removed during consolidation, but a
  repository-wide plan audit remains intentionally deferred.
