---
title: "Design Documentation Consolidation and Reconciliation"
type: documentation-plan
status: completed
date: 2026-07-22
review_state: owner-approved-and-implemented
tags:
  - design
  - documentation
  - governance
  - metadata
---
# Design Documentation Consolidation and Reconciliation

## Overview

Reorganize the game-design documentation into four explicit scopes under
`docs/design/`:

```text
docs/design/
├── README.md
├── current/
│   ├── README.md
│   └── ...approved current mechanical contracts...
├── direction/
│   ├── README.md
│   └── ...foundational and committed directional decisions...
├── lore/
│   ├── README.md
│   └── ...canonical fiction and setting context...
└── ideas/
    ├── README.md
    └── ...non-authoritative future possibilities...
```

The reorganization will use YAML frontmatter and README guidance so humans and
agents can determine a document's authority, time horizon, and intended use
without inferring those properties from filenames or prose.

This plan covers documentation structure, metadata, reconciliation, links, and
validation. It does not change game code or gameplay behavior. It also does not
yet audit implementation plans for design rules that need extraction; that is a
separate follow-up after the new design hierarchy is approved.

## Goals

1. Make `docs/design/current/` the authoritative source for approved current
   mechanical behavior.
2. Preserve the founding governance document as the foundation of the current
   project while distinguishing durable direction from implemented mechanics.
3. Keep lore canonical but non-mechanical.
4. Keep future ideas explicitly non-authoritative and non-committal.
5. Give package indexing enough metadata for agents to filter documents by
   authority and horizon.
6. Remove ambiguity caused by duplicated contracts, mixed time horizons, and
   implementation plans serving as de facto design authority.

## Non-goals

- Changing game behavior, tuning, schemas, or generator output.
- Auditing every file in `docs/plans/` during this pass.
- Rewriting implementation history to match the new hierarchy.
- Preserving old design-document paths as duplicate compatibility copies.
- Turning every design question into an implementation todo.
- Treating ideas as roadmap commitments.

## Proposed authority model

Each subfolder answers a different question:

| Folder | Question answered | Authority |
| --- | --- | --- |
| `current/` | What is the approved game contract now? | Normative for current mechanics |
| `direction/` | What durable principles and committed outcomes guide future design? | Normative as a design constraint, but not evidence of implementation |
| `lore/` | What fiction and setting context is canonical? | Canonical context, not a mechanical requirement unless a current design page says so |
| `ideas/` | What possibilities might be explored later? | Non-authoritative and non-committal |

Proposed agent precedence:

1. Use `current/` to answer questions about existing or approved mechanics.
2. Use `direction/` to evaluate whether a proposal fits the intended game.
3. Use `lore/` for fiction and terminology, never to infer an unstated mechanic.
4. Use `ideas/` only for exploration; never implement an idea without explicit
   promotion into an approved current design and an implementation plan.
5. If `current/` and `direction/` differ, inspect the cause rather than applying
   automatic precedence. The current state may be an intentional pragmatic step,
   the direction may describe a future migration, or the mismatch may be an
   implementation/documentation defect. Agents must surface the classification
   and use task context and human review instead of silently choosing.
6. Plans and todos describe execution. They do not override design documents.
   Implementation plans must include applicable updates to `current/` and
   `direction/`, completed after implementation is reviewed and before merge.

> **OWNER QUESTION 1 — Direction authority:** Should `direction/` contain
> committed design constraints that future work must honor, or should it be
> advisory until promoted into `current/`?
>
> **OWNER RESPONSE:** We can use a workflow where the implementation plans that agents use in docs/plans/ include updating design/ and direction/ after implementation is reviewed and approved for merging

> **OWNER QUESTION 2 — Conflict behavior:** When a current contract conflicts
> with a committed direction, should agents always preserve current behavior
> unless a task explicitly authorizes migration, as proposed above?
>
> **OWNER RESPONSE:** Agents and humans need to use their judgement in these cases. A conflict could be because the current version is pragmatic, but the direction is still the future intention. Or it could be an implementation defect. It's case-by-case.

## Proposed metadata contract

Use a small common schema rather than relying on title or directory alone:

```yaml
---
title: "Scouting and Knowledge"
type: design-current
status: approved
authority: normative
horizon: current
tags:
  - scouting
  - knowledge
---
```

Recommended controlled values:

### `type`

- `design-index`
- `design-current`
- `design-direction`
- `design-lore`
- `design-idea`

### `status`

- `active` — maintained index or living foundation
- `approved` — reviewed contract or canonical statement
- `draft` — under active review and not authoritative
- `superseded` — retained only when historical retention is explicitly useful

### `authority`

- `normative` — current mechanical contract
- `directional` — committed constraint or destination, not current behavior
- `canonical-context` — accepted lore without implied mechanics
- `non-authoritative` — idea or brainstorming material

### `horizon`

- `current`
- `long-term`
- `setting`
- `exploratory`

Optional provenance fields such as `source`, `supersedes`, or `refines` may be
retained where useful, but they do not change authority. Relative links should
be updated after moves.

> **OWNER QUESTION 3 — Metadata vocabulary:** Are the proposed field names and
> controlled values suitable for your indexing workflow, or do you prefer
> shorter/different values?
>
> **OWNER RESPONSE:** `exploratory` is approved as the replacement for
> `future-possible`.

> **OWNER QUESTION 4 — Draft indexing:** Should draft documents remain inside
> these four folders with `status: draft`, or should the authoritative design
> tree contain only approved/active files?
>
> **OWNER RESPONSE:** draft documents shouldn't be in current/ to avoid any accidental confusion. The other folders are tolerant of drafts.

## Semantic decision references

Numbered G/Q labels are an arbitrary founding-document convention and will not
remain the primary reference system.

Use canonical relative page/heading links for ordinary citations. Add semantic
IDs only for durable decisions that are cited across current design, direction,
agent guidance, tests, or active implementation plans. Example metadata:

```yaml
design_ids:
  - worldgen.constructive-origin
  - testing.generated-world-invariants
legacy_ids:
  - G18
```

Rules:

- IDs describe one focused proposition rather than a bundle of unrelated rules.
- IDs use stable domain-and-concept names such as
  `information.two-channel`, `governance.delegation-by-distance`, and
  `economy.physical-resources`.
- Authority, horizon, priority, and implementation status do not appear in the
  ID; those properties may change while the concept remains identifiable.
- Most pages and headings need no explicit ID. Do not create a replacement ID
  bureaucracy when a canonical link is sufficient.
- Ideas receive no durable decision ID until promoted into direction.
- New sequential G/Q labels stop being introduced.
- `direction/README.md` maintains the legacy G-to-semantic/page mapping needed
  by completed plans and historical discussion.
- Active documents such as `AGENTS.md`, current design, and active plans migrate
  to semantic links. Completed plans may retain G references because the legacy
  mapping keeps them resolvable.

Compound entries may split into more than one semantic decision. For example,
G18 can map separately to `worldgen.constructive-origin` and
`testing.generated-world-invariants` rather than preserving its accidental
bundling.

## Proposed file layout and moves

### Root index

`docs/design/README.md` remains the entry point. It will:

- explain the four scopes and precedence rules;
- tell agents which folders are safe to treat as requirements;
- link each subfolder README;
- state that plans are implementation artifacts rather than design authority;
- provide a short promotion workflow; and
- avoid describing current and future pages together as one undifferentiated
  set of “durable contracts.”

The existing index currently calls its pages contracts for “current and future
stages” and lists the governance sandbox, completed plan, and ideas together
under one evidence-policy heading. That presentation is the primary ambiguity
being removed. Evidence: `docs/design/README.md:8-10,43-49`.

### Current contracts

Move the existing approved topic pages into `docs/design/current/`:

- `energy-and-seasons.md`
- `generator-identity.md`
- `generator-revision-1.md`
- `population-and-habitats.md`
- `scouting-and-knowledge.md`
- `ships-and-expansion.md`
- `simulation-timing.md`
- `systems-and-resources.md`
- `tuning-profiles.md`
- `world-generation.md`

Create `docs/design/current/README.md` as a contract map identifying the
canonical owner for each mechanic. This index should help agents avoid copying a
rule from a secondary summary when a dedicated owning page exists.

### Direction

Move and rename the founding document to a stable, non-date-based path, proposed:

```text
docs/design/direction/governance-sandbox-foundations.md
```

Create `docs/design/direction/README.md` describing the document as the founding
basis for the current project and explaining that a directional decision is not
proof that its mechanics exist today.

Retire G-number identifiers as the primary vocabulary. Existing plans and
historical discussions may continue to cite G1–G22 through an explicit legacy
mapping, while active guidance uses canonical page/heading links and semantic
decision IDs where stable cross-document references are useful. Git history is
the path-level historical record; do not retain a second copy at the old
location.

The early framework phase moves the founding document as
`docs/design/direction/foundations.md` without broadly decomposing it. The final
content phase of this same plan then splits the foundation into focused
direction and lore pages after the metadata, README, authority conventions, and
current-document reconciliation are established. The direction README may
summarize decision classifications needed to prevent agent confusion before
that final decomposition.

> **OWNER QUESTION 5 — Founding filename:** Is
> `governance-sandbox-foundations.md` the desired durable name, or would
> `foundations.md` / `game-direction.md` be preferable?
>
> **OWNER RESPONSE:** `foundations.md` is good to start.

### Lore

Create `docs/design/lore/README.md` in this framework sweep. It will define lore
as canonical context that cannot imply unstated mechanics and will link to the
rudimentary lore still present in `direction/foundations.md`.

The final foundation-decomposition phase of this plan will extract the loose
fiction premise into focused lore pages, beginning with a likely page such as:

```text
docs/design/lore/precursor-aftermath.md
```

That page can capture the post-apocalyptic 4X premise, precursor collapse,
ruined-grid framing, origin-community identity, and intentionally unresolved
fiction. Unresolved fiction may remain in lore pages. Lore must explicitly say
that it does not cause ruins, factions, or other entities to exist mechanically
unless a current design contract defines them.

> **OWNER QUESTION 6 — Lore status:** Is the current “loose, generative” premise
> canonical lore, or should it remain directional inspiration with the lore
> folder initially containing only its README?
>
> **OWNER RESPONSE:** the foundation document contains rudimentary lore. those can be added to lore/ to get us started

> **OWNER QUESTION 7 — Lore granularity:** Should unresolved fiction questions
> remain at the end of the lore page, move to `ideas/`, or remain in the
> direction foundation as design constraints awaiting fiction?
>
> **OWNER RESPONSE:** Unresolved fiction can stay in lore pages.

### Ideas

Move `docs/design/ideas/README.md` to a proposed stable path:

```text
docs/design/ideas/future-feature-ideas.md
```

Create `docs/design/ideas/README.md` stating that every file in the folder is
non-authoritative. An idea becomes a contract only after explicit review and
promotion into `current/`; an implementation plan alone is insufficient.

Keep ideas grouped in one file during this pass unless splitting materially
improves ownership or indexing. Topic-level splitting can happen later without
changing the authority model.

> **OWNER QUESTION 8 — Idea promotion:** Should an approved long-term commitment
> move first into `direction/`, then later into `current/`, while purely
> optional possibilities remain in `ideas/`?
>
> **OWNER RESPONSE:** Yes, let's try that.

> **OWNER QUESTION 9 — Idea file shape:** Keep one indexed future-ideas file for
> now, or split it into topic files such as `information.md`, `ruins.md`, and
> `population.md` during this consolidation?
>
> **OWNER RESPONSE:** Splitting the files up makes more sense to me.

## Reconciliation decisions required

The following are primarily authority/horizon discrepancies. They should be
resolved in documentation without changing current gameplay.

### R1. Remote control and delegation

Current contracts make a founded system directly commandable after the origin
receives its successful outcome. The foundation describes delegation and
attenuated remote control as G15, while current ship design explicitly excludes
that behavior. Evidence:

- `docs/design/current/ships-and-expansion.md:149-157`
- `docs/design/current/systems-and-resources.md:140-147`
- `docs/design/direction/README.md#legacy-g-label-mapping`

Proposed reconciliation: current direct control remains authoritative;
delegation-by-distance is labeled committed long-term direction, not current
behavior.

> **OWNER QUESTION 10 — G15 commitment:** Is delegation-by-distance a committed
> destination, or only a promising idea?
>
> **OWNER RESPONSE:** It is a long-term direction.

### R2. Information channels

G10 specifies periodic thin communications carrying runtime summaries. Current
scouting has delayed probe/ship observations and explicitly excludes population,
stocks, developments, queues, and similar runtime state. The ideas file again
presents richer two-channel information as a possible later feature. Evidence:

- `docs/design/current/scouting-and-knowledge.md:8-11,98-113,175-181`
- `docs/design/direction/README.md#legacy-g-label-mapping`
- `docs/design/ideas/README.md`

Proposed reconciliation: retain ship/probe observations as current; classify the
full comms-plus-ships model as directional only if G10 is still committed. Keep
specific payloads, cadence, and authority consequences as ideas/open questions.

> **OWNER QUESTION 11 — G10 commitment:** Is the two-channel model itself
> settled, with only its payload and cadence open, or is the entire model still
> optional?
>
> **OWNER RESPONSE:** Two-channel model is directional. The idea is that ships are faster-than-light and thus are able to carry information faster than light-based distant comms.

### R3. Specialists and tertiary production

G11 and G12 call specialists-on-population and tertiary production decisions,
while the ideas file describes both as possible later features and current
design contains only Ore → Alloy. Evidence:

- `docs/design/direction/README.md#legacy-g-label-mapping`
- `docs/design/ideas/README.md`

Proposed reconciliation: if the principles remain settled, keep them in
`direction/`; leave exact specialist types, training, locks, recipes, and upkeep
in `ideas/`. If not settled, move the concepts wholly to ideas.

> **OWNER QUESTION 12 — G11/G12 commitment:** Are specialists as pop state and a
> tertiary specialist substrate committed constraints?
>
> **OWNER RESPONSE:** The directional aspect is the overall game loop. specialists, training, etc. are ideas for ways to accomplish the directional goal.

### R4. Ruins and current world generation

The foundation makes precursor ruins central to the premise, but current
revision-1 generation has no reclaimable-site requirement. Resource ruins and
site reclamation appear as future ideas. Evidence:

- `docs/design/current/world-generation.md:151-155`
- `docs/design/direction/README.md#legacy-g-label-mapping`
- `docs/design/ideas/README.md`

Proposed reconciliation: current frontier generation produces empty geography
and resources; ruins are lore and/or committed direction but not generated
mechanics. Reword G17 so “the world starts dead” does not imply revision 1
already generates ruins.

> **OWNER QUESTION 13 — Ruin commitment:** Are the two ruin categories in G14 a
> committed long-term taxonomy or still ideas?
>
> **OWNER RESPONSE:** The directional aspect is the world generation and the world having pre-existing "stuff" akin to Dwarf Fortress. We have some rudimentary half-baked implementation. Some is "current" and some are just ideas to improve the implementation to align better with the directional goal.

### R5. Bodies and slots

The foundation combines the current body → slot → development hierarchy with
future body types, suitability, and precursor infrastructure. Current slots are
generic, and ideas explicitly reject speculative slot fields in the current
schema. Evidence:

- `docs/design/current/systems-and-resources.md:14-21,42-54`
- `docs/design/direction/README.md#legacy-g-label-mapping`
- `docs/design/ideas/README.md`

Proposed reconciliation: keep the hierarchy in current design; classify body
and slot differentiation as ideas unless it is a committed direction.

> **OWNER QUESTION 14 — Slot differentiation:** Does direction commit to bodies
> or slots eventually becoming mechanically differentiated, or is that entirely
> optional?
>
> **OWNER RESPONSE:** Just an idea.

### R6. Failure persistence and run structure

G1 states that failed communities persist as reclaimable ruins across or within
later play, while current design lacks community ruin transitions and the
foundation leaves origin succession/run structure open in Q10. Evidence:

- `docs/design/direction/README.md#legacy-g-label-mapping`
- `docs/design/current/population-and-habitats.md:50-56`

Proposed reconciliation: retain “the simulation absorbs valid gameplay failure”
as direction. Keep persistence, reclamation, succession, and cross-run behavior
open until separately approved.

> **OWNER QUESTION 15 — G1 scope:** Is world-absorbed failure the committed
> principle while cross-run persistence remains an idea, or is persistent ruin
> state itself committed?
>
> **OWNER RESPONSE:** It's a _very_ long-term direction. Low priority. It's so far from being implementable that I don't know how to handle it as design direction.

### R7. Collector upkeep premise

The ideas file says Collectors incur normal upkeep, but the approved resource
engine gives functional Collectors zero Energy upkeep. Evidence:

- `docs/design/ideas/README.md`
- `docs/plans/2026-07-20-feature-constructive-world-generation-stage-4-plan.md:98,163`

Proposed reconciliation: correct the idea's current-state premise to zero
operating upkeep. The future idea may explore curtailment together with a new
operating-cost model, but should not imply that cost exists now.

> **OWNER QUESTION 16 — Curtailment:** Should the idea remain about avoiding
> overflow only, or should it explicitly explore adding Collector operating
> costs that curtailment could avoid?
>
> **OWNER RESPONSE:** Collectors have zero energy upkeep. Curtailment is on shaky ground as a mechanic.

## Tuning values and configuration authority

Mutable shipped tuning values should not be copied into design prose. Because
configuration and documentation live in the same repository,
`content/profiles/starter.ron` is the operational source of truth for the
current `starter` values. Design pages should link directly to that file and
explain meaning, relationships, invariants, and design rationale rather than
reprinting mutable tables.

The consolidation must distinguish three categories:

1. **Mutable balance/profile values** — owned by the applicable file under
   `content/profiles/`; docs link to the configuration instead of duplicating
   values.
2. **Reviewed design constraints and generator ranges** — documented as intent
   and changed only with design review, even when their active representation
   also lives in configuration.
3. **Version-frozen algorithm constants and test vectors** — remain exact in
   `generator-revision-1.md` because they define reproducibility rather than
   mutable balance.

A configuration value being operationally authoritative does not make every
value a timeless design guarantee. Conversely, removing duplicated numeric
prose must not erase named invariants, fixed relationships, or revision-frozen
engineering contracts.

## Canonical ownership and duplication reduction

Create a contract-ownership table in `current/README.md`. Proposed ownership:

| Contract | Canonical page |
| --- | --- |
| Origin/frontier structural generation | `world-generation.md` |
| Generator reproduction identity | `generator-identity.md` |
| Frozen revision-1 algorithm | `generator-revision-1.md` |
| Editable values and profile schema | `tuning-profiles.md` |
| Physical ownership and resource lifecycle | `systems-and-resources.md` |
| Energy production, seasons, priority, retention | `energy-and-seasons.md` |
| Population identity, Habitat support and generation | `population-and-habitats.md` |
| Knowledge facts, observations and merging | `scouting-and-knowledge.md` |
| Ship projects, launch and expedition workflow | `ships-and-expansion.md` |
| Global phase order, activation and atomicity | `simulation-timing.md` |

During reconciliation:

- keep exact formulas and transition sequences on one owning page;
- replace secondary copies with short consequences and links where practical;
- preserve cross-domain invariants where omission would make a page misleading;
- do not alter a mechanic merely to eliminate duplicated prose; and
- flag conflicting duplicates for owner review rather than choosing silently.

Highest-drift areas are the origin scaffold, seasonal formula, route formulas,
founding transition, remote commandability, and tick order.

> **OWNER QUESTION 17 — Duplication scope:** Should this pass actively reduce
> duplicated mechanical prose, or only identify canonical owners and defer prose
> reduction to a second pass?
>
> **OWNER RESPONSE:** We can reduce duplication. We also want to be _especially_ careful about encoding "magic numbers" into design documentation. Exact numbers are flaky, since designers may need or want to change those as we prototype.

## Implementation phases

### Phase 1 — Approve taxonomy and reconciliation answers

- [x] Record owner responses in this plan.
- [x] Resolve Questions 1–17 or explicitly mark any as deferred.
- [x] Finalize folder authority, metadata values, and promotion workflow.
- [x] Classify the propositions currently bundled under G1–G22 as current
      foundation, committed direction, open direction, idea, or
      superseded/refined.
- [x] Confirm which content is canonical lore.

Deliverable: owner decisions are recorded before implementation; completion
records the resulting hierarchy and validation evidence.

### Phase 2 — Establish structure, metadata, and agent guidance

- [x] Create the four subfolders.
- [x] Rewrite `docs/design/README.md` as the authority and navigation index.
- [x] Add a metadata-bearing README to every subfolder.
- [x] Move current design pages using Git-aware moves.
- [x] Add normalized frontmatter to the founding and ideas documents.
- [x] Normalize existing current-page frontmatter without changing mechanics.
- [x] Update `AGENTS.md` to require reading `docs/design/README.md` for gameplay
      or design work, explain the four authority scopes, forbid treating ideas
      as requirements, require case-by-case conflict classification, and require
      implementation plans to update applicable design documents before merge.
- [x] Add the mutable-tuning rule: configuration files own active values while
      design docs own meaning, constraints, and rationale.

Deliverable: every indexed design file declares type, status, authority, and
horizon, and repository-level agent guidance points to the hierarchy.

### Phase 3 — Place foundation, lore framework, and topic ideas

- [x] Move the founding document intact to `direction/foundations.md` while
      retaining G labels temporarily as legacy references.
- [x] Put the minimum decision/horizon summary needed for safe agent use in
      `direction/README.md`; defer broad foundation decomposition until the
      final content phase of this plan.
- [x] Create `lore/README.md`, define its authority, and point to the rudimentary
      lore that remains in the foundation until the final content phase.
- [x] Move and split the future-ideas document into focused topic files under
      `ideas/`.
- [x] Remove or reframe idea content that incorrectly presents a committed
      direction as optional, according to owner responses.
- [x] Ensure ideas use current pages and configuration files as links rather than
      independently asserting current mechanics or mutable values.

Deliverable: the framework makes each file's authority discoverable before the
foundation is decomposed later in this plan.

### Phase 4 — Reconcile current contracts and links

- [x] Apply R1–R7 using the approved responses, limiting foundation edits to
      what is necessary for correctness before the final decomposition phase.
- [x] Add the canonical contract-ownership table.
- [x] Reduce duplicated contract prose to the approved scope.
- [x] Replace duplicated mutable `starter` values with links to
      `content/profiles/starter.ron`; retain reviewed constraints and frozen
      revision constants where they are themselves the contract.
- [x] Update all relative links within `docs/design/`.
- [x] Update inbound links from other docs, plans, todos, and root documentation.
- [x] Do not retain duplicate files or old-path aliases unless explicitly
      requested.

Deliverable: all links target one canonical physical document before the
foundation is split.

### Phase 5 — Decompose the foundation

- [x] Split `direction/foundations.md` into focused direction pages after the
      folder framework and current contracts are stable.
- [x] Keep `foundations.md` as the concise founding index and durable-principles
      entry point rather than preserving the original mixed document unchanged.
- [x] Break compound G entries into focused propositions where their concepts,
      authority, or horizon differ.
- [x] Assign semantic decision IDs only to durable cross-document decisions;
      use canonical page/heading links for everything else.
- [x] Add `legacy_ids` metadata where useful and build an explicit mapping from
      every historical G label to its semantic decision(s) and canonical page.
- [x] Replace active G references, including G18 in `AGENTS.md`, while leaving
      completed implementation plans untouched.
- [x] Stop introducing sequential G/Q labels.
- [x] Move rudimentary setting material into focused lore pages, including
      `lore/precursor-aftermath.md` when the extracted content supports it.
- [x] Keep unresolved fiction in lore pages and mark it clearly so canonical
      setting context is distinguishable from undecided detail.
- [x] Move optional mechanisms into the appropriate focused idea pages rather
      than retaining them as directional commitments.
- [x] Keep committed long-term outcomes in direction pages while linking to the
      current pragmatic implementation where one exists.
- [x] Update direction and lore README indexes after the split.
- [x] Review the decomposed result as a content change separately from the
      earlier path/metadata moves so semantic changes remain visible.

Deliverable: the founding material is expressed through focused direction and
lore pages without losing its provenance or role as the project foundation;
historical G references remain resolvable through a legacy mapping rather than
remaining the active vocabulary.

### Phase 6 — Validate and record deferred plan extraction

- [x] Run metadata/index validation.
- [x] Run relative Markdown-link validation.
- [x] Search for old paths and obsolete titles.
- [x] Review the diff for accidental mechanic changes.
- [x] Record, but do not execute, a follow-up audit of `docs/plans/` for current
      contracts not represented under `docs/design/current/`.

The deferred audit is recorded in
`todos/003-pending-p2-audit-plans-for-design-truth.md`. The originally known
`tuning-profiles.md` dependency was resolved during consolidation by linking
focused current owners and same-repository configuration; the broader plan audit
remains intentionally deferred.

## Validation strategy

This is a documentation migration, so Rust unit or integration tests are not
required unless implementation files are unexpectedly touched. Validation
should be deterministic and repository-local.

### Metadata/index tests

- Rebuild or strictly refresh the Compound Game Dev artifact index.
- Query each `type`, `authority`, and `horizon` value and confirm expected files
  are returned.
- Confirm ideas can be excluded from current-design queries.
- Confirm every Markdown file beneath `docs/design/` has valid frontmatter,
  except where the indexing tool intentionally exempts a file.
- Confirm `horizon: exploratory` and `authority: non-authoritative` reliably
  select idea documents without selecting committed direction.

### Link and path tests

- Validate every relative Markdown link beneath `docs/design/` resolves.
- Search the repository for retired root-level ideas paths, the retired dated
  foundation filename, and old `docs/design/<topic>.md` paths.
- Update intentional citations; report any intentionally retained historical
  text rather than silently leaving a broken path.

### Content review

- Compare moved current pages before and after normalization to ensure mechanics
  did not change unintentionally.
- Review every use of “current,” “approved,” “future,” “direction,” “decision,”
  “idea,” and “outside this design” in the migrated corpus.
- Verify every historical G label resolves through the legacy mapping.
- Verify semantic decision IDs are unique, focused, and searchable.
- Verify active guidance no longer relies on G/Q labels and no new sequential
  labels were introduced.
- Verify no idea is presented as an implementation requirement.
- Verify lore does not imply an unstated current mechanic.
- Verify mutable profile values are linked to `content/profiles/starter.ron`
  rather than duplicated, while reviewed ranges, named invariants, formulas,
  and generator-revision constants remain documented where appropriate.

### Repository checks

- Run `git diff --check`.
- Confirm no generated index, build output, or machine-local configuration is
  included in the change.
- Update `CHANGELOG.md` only if the reconciliation intentionally changes
  player-facing design, not for path-only documentation organization.

## Acceptance criteria

- [x] `docs/design/` contains `current/`, `direction/`, `lore/`, and `ideas/`,
      each with an indexed README.
- [x] Every design document has explicit type, status, authority, and horizon
      metadata, using `exploratory` for uncommitted future ideas.
- [x] Root and subfolder README files explain agent behavior and authority
      precedence in plain language.
- [x] `AGENTS.md` directs agents to the design hierarchy and requires applicable
      design-document updates as part of reviewed implementation work.
- [x] Current mechanical contracts are located under `current/` and remain
      mechanically unchanged unless an owner response explicitly approves a
      reconciliation change.
- [x] The founding material remains identifiable as the project's foundation
      and is decomposed in the final content phase after the framework is
      established.
- [x] Historical G1–G22 references resolve through an explicit legacy mapping;
      active guidance uses canonical links and focused semantic IDs only where
      needed.
- [x] Compound G entries are split when they contain propositions with different
      authority or horizons, and new sequential G/Q labels are prohibited.
- [x] Committed direction is distinguishable from current implementation.
- [x] Lore is distinguishable from mechanical contracts.
- [x] Ideas are explicitly non-authoritative and contain no accidental current
      requirements.
- [x] R1–R7 are resolved or explicitly documented as deferred conflicts.
- [x] Relative links resolve and repository references use canonical new paths.
- [x] Package-index searches can reliably include current design and exclude
      ideas using metadata.
- [x] Mutable tuning values use repository config links as their source of truth;
      reviewed design constraints and revision-frozen constants remain explicit.
- [x] No duplicate compatibility copies of moved documents remain.
- [x] A separate follow-up need for extracting current contracts from plans is
      recorded without expanding this migration into a plan audit.

## Risks and mitigations

### Direction mistaken for implementation

**Risk:** Agents see an approved directional decision and implement it as current
behavior.

**Mitigation:** Distinct `authority` and `horizon` metadata, repeated README
rules, and explicit current-gap language in directional pages.

### Metadata vocabulary drifts

**Risk:** Similar values such as `future`, `directional`, and `proposed` become
inconsistent and searches become unreliable.

**Mitigation:** Publish controlled values in the root README and use only those
values in this corpus.

### Link churn hides semantic edits

**Risk:** Large move diffs make design changes difficult to review.

**Mitigation:** Prefer a move/metadata pass followed by focused reconciliation,
or otherwise review content diffs separately from path changes.

### Founding history becomes harder to recognize

**Risk:** Removing the dated filename obscures provenance.

**Mitigation:** Preserve the founding date and original-path provenance in
frontmatter while using a stable current filename.

### Current design remains incomplete without plans

**Risk:** Moving files creates the appearance of complete authority while some
retained contracts still live only in implementation plans.

**Mitigation:** State the known limitation in `current/README.md` and schedule a
separate bounded extraction audit after this consolidation.

## Research findings

Research was limited to the requested documentation corpus and its indexing
behavior. No external research or API documentation was needed.

- Existing current design pages consistently use `type: design` and
  `status: approved`, but ideas and the founding document lack equivalent
  frontmatter. Evidence: current file headers throughout `docs/design/`;
  `docs/design/ideas/README.md`;
  `docs/design/direction/README.md#legacy-g-label-mapping`.
- The current index mixes approved contracts, a completed implementation plan,
  the founding direction, and future ideas in one navigation section. Evidence:
  `docs/design/README.md:43-49`.
- No relevant institutional solution documents were found under
  `docs/solutions/`.
- The repository is Git-based and uses `.gitignore`; moved documents should use
  normal Git-aware file operations.

## Related documents

- `docs/design/README.md`
- `docs/design/ideas/README.md`
- `docs/design/direction/README.md#legacy-g-label-mapping`
- `docs/architecture.md`
- `content/profiles/starter.ron`
- `AGENTS.md`
