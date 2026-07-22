---
title: "Design Documentation Consolidation and Reconciliation"
type: documentation-plan
status: draft
date: 2026-07-22
review_state: awaiting-owner-responses
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
5. If `current/` and `direction/` differ, treat that as an intentional
   current-to-future gap unless the documents explicitly identify a conflict.
   Do not silently implement the directional state.
6. Plans and todos describe execution. They do not override design documents.

> **OWNER QUESTION 1 — Direction authority:** Should `direction/` contain
> committed design constraints that future work must honor, or should it be
> advisory until promoted into `current/`?
>
> **OWNER RESPONSE:**

> **OWNER QUESTION 2 — Conflict behavior:** When a current contract conflicts
> with a committed direction, should agents always preserve current behavior
> unless a task explicitly authorizes migration, as proposed above?
>
> **OWNER RESPONSE:**

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
- `future-possible`

Optional provenance fields such as `source`, `supersedes`, or `refines` may be
retained where useful, but they do not change authority. Relative links should
be updated after moves.

> **OWNER QUESTION 3 — Metadata vocabulary:** Are the proposed field names and
> controlled values suitable for your indexing workflow, or do you prefer
> shorter/different values?
>
> **OWNER RESPONSE:**

> **OWNER QUESTION 4 — Draft indexing:** Should draft documents remain inside
> these four folders with `status: draft`, or should the authoritative design
> tree contain only approved/active files?
>
> **OWNER RESPONSE:**

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

Preserve G-number identifiers so existing plans and discussions can continue to
cite G1–G22. Git history is the path-level historical record; do not retain a
second copy at the old location.

> **OWNER QUESTION 5 — Founding filename:** Is
> `governance-sandbox-foundations.md` the desired durable name, or would
> `foundations.md` / `game-direction.md` be preferable?
>
> **OWNER RESPONSE:**

### Lore

Create `docs/design/lore/README.md`. Extract the loose fiction premise from the
founding document into a proposed canonical lore page:

```text
docs/design/lore/precursor-aftermath.md
```

The page would capture the post-apocalyptic 4X premise, precursor collapse,
ruined-grid framing, origin-community identity, and intentionally unresolved
fiction. It must explicitly say that lore does not cause ruins, factions, or
other entities to exist mechanically unless a current design contract defines
them.

> **OWNER QUESTION 6 — Lore status:** Is the current “loose, generative” premise
> canonical lore, or should it remain directional inspiration with the lore
> folder initially containing only its README?
>
> **OWNER RESPONSE:**

> **OWNER QUESTION 7 — Lore granularity:** Should unresolved fiction questions
> remain at the end of the lore page, move to `ideas/`, or remain in the
> direction foundation as design constraints awaiting fiction?
>
> **OWNER RESPONSE:**

### Ideas

Move `docs/ideas.md` to a proposed stable path:

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
> **OWNER RESPONSE:**

> **OWNER QUESTION 9 — Idea file shape:** Keep one indexed future-ideas file for
> now, or split it into topic files such as `information.md`, `ruins.md`, and
> `population.md` during this consolidation?
>
> **OWNER RESPONSE:**

## Reconciliation decisions required

The following are primarily authority/horizon discrepancies. They should be
resolved in documentation without changing current gameplay.

### R1. Remote control and delegation

Current contracts make a founded system directly commandable after the origin
receives its successful outcome. The foundation describes delegation and
attenuated remote control as G15, while current ship design explicitly excludes
that behavior. Evidence:

- `docs/design/ships-and-expansion.md:149-157`
- `docs/design/systems-and-resources.md:140-147`
- `docs/2026-07-20-design-direction-governance-sandbox.md:140-149`

Proposed reconciliation: current direct control remains authoritative;
delegation-by-distance is labeled committed long-term direction, not current
behavior.

> **OWNER QUESTION 10 — G15 commitment:** Is delegation-by-distance a committed
> destination, or only a promising idea?
>
> **OWNER RESPONSE:**

### R2. Information channels

G10 specifies periodic thin communications carrying runtime summaries. Current
scouting has delayed probe/ship observations and explicitly excludes population,
stocks, developments, queues, and similar runtime state. The ideas file again
presents richer two-channel information as a possible later feature. Evidence:

- `docs/design/scouting-and-knowledge.md:8-11,98-113,175-181`
- `docs/2026-07-20-design-direction-governance-sandbox.md:94-113`
- `docs/ideas.md:199-211`

Proposed reconciliation: retain ship/probe observations as current; classify the
full comms-plus-ships model as directional only if G10 is still committed. Keep
specific payloads, cadence, and authority consequences as ideas/open questions.

> **OWNER QUESTION 11 — G10 commitment:** Is the two-channel model itself
> settled, with only its payload and cadence open, or is the entire model still
> optional?
>
> **OWNER RESPONSE:**

### R3. Specialists and tertiary production

G11 and G12 call specialists-on-population and tertiary production decisions,
while the ideas file describes both as possible later features and current
design contains only Ore → Alloy. Evidence:

- `docs/2026-07-20-design-direction-governance-sandbox.md:114-124`
- `docs/ideas.md:112-130,186-197`

Proposed reconciliation: if the principles remain settled, keep them in
`direction/`; leave exact specialist types, training, locks, recipes, and upkeep
in `ideas/`. If not settled, move the concepts wholly to ideas.

> **OWNER QUESTION 12 — G11/G12 commitment:** Are specialists as pop state and a
> tertiary specialist substrate committed constraints?
>
> **OWNER RESPONSE:**

### R4. Ruins and current world generation

The foundation makes precursor ruins central to the premise, but current
revision-1 generation has no reclaimable-site requirement. Resource ruins and
site reclamation appear as future ideas. Evidence:

- `docs/design/world-generation.md:151-155`
- `docs/2026-07-20-design-direction-governance-sandbox.md:135-139,151-160`
- `docs/ideas.md:159-184`

Proposed reconciliation: current frontier generation produces empty geography
and resources; ruins are lore and/or committed direction but not generated
mechanics. Reword G17 so “the world starts dead” does not imply revision 1
already generates ruins.

> **OWNER QUESTION 13 — Ruin commitment:** Are the two ruin categories in G14 a
> committed long-term taxonomy or still ideas?
>
> **OWNER RESPONSE:**

### R5. Bodies and slots

The foundation combines the current body → slot → development hierarchy with
future body types, suitability, and precursor infrastructure. Current slots are
generic, and ideas explicitly reject speculative slot fields in the current
schema. Evidence:

- `docs/design/systems-and-resources.md:14-21,42-54`
- `docs/2026-07-20-design-direction-governance-sandbox.md:125-134`
- `docs/ideas.md:5-27`

Proposed reconciliation: keep the hierarchy in current design; classify body
and slot differentiation as ideas unless it is a committed direction.

> **OWNER QUESTION 14 — Slot differentiation:** Does direction commit to bodies
> or slots eventually becoming mechanically differentiated, or is that entirely
> optional?
>
> **OWNER RESPONSE:**

### R6. Failure persistence and run structure

G1 states that failed communities persist as reclaimable ruins across or within
later play, while current design lacks community ruin transitions and the
foundation leaves origin succession/run structure open in Q10. Evidence:

- `docs/2026-07-20-design-direction-governance-sandbox.md:54-57,272-275`
- `docs/design/population-and-habitats.md:50-56`

Proposed reconciliation: retain “the simulation absorbs valid gameplay failure”
as direction. Keep persistence, reclamation, succession, and cross-run behavior
open until separately approved.

> **OWNER QUESTION 15 — G1 scope:** Is world-absorbed failure the committed
> principle while cross-run persistence remains an idea, or is persistent ruin
> state itself committed?
>
> **OWNER RESPONSE:**

### R7. Collector upkeep premise

The ideas file says Collectors incur normal upkeep, but the approved resource
engine gives functional Collectors zero Energy upkeep. Evidence:

- `docs/ideas.md:29-39`
- `docs/plans/2026-07-20-feature-constructive-world-generation-stage-4-plan.md:98,163`

Proposed reconciliation: correct the idea's current-state premise to zero
operating upkeep. The future idea may explore curtailment together with a new
operating-cost model, but should not imply that cost exists now.

> **OWNER QUESTION 16 — Curtailment:** Should the idea remain about avoiding
> overflow only, or should it explicitly explore adding Collector operating
> costs that curtailment could avoid?
>
> **OWNER RESPONSE:**

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
> **OWNER RESPONSE:**

## Implementation phases

### Phase 1 — Approve taxonomy and reconciliation answers

- [ ] Record owner responses in this plan.
- [ ] Resolve Questions 1–17 or explicitly mark any as deferred.
- [ ] Finalize folder authority, metadata values, and promotion workflow.
- [ ] Finalize the status of G1–G22 as current foundation, committed direction,
      open direction, or superseded/refined.
- [ ] Confirm which content is canonical lore.

Deliverable: this plan changes from `draft` to `approved` with decisions
recorded.

### Phase 2 — Establish structure and metadata

- [ ] Create the four subfolders.
- [ ] Rewrite `docs/design/README.md` as the authority and navigation index.
- [ ] Add a metadata-bearing README to every subfolder.
- [ ] Move current design pages using Git-aware moves.
- [ ] Add normalized frontmatter to the founding and ideas documents.
- [ ] Normalize existing current-page frontmatter without changing mechanics.

Deliverable: every indexed design file declares type, status, authority, and
horizon.

### Phase 3 — Separate foundation, lore, and ideas

- [ ] Move the founding document into `direction/` while retaining G identifiers.
- [ ] Add a decision-status matrix for G1–G22.
- [ ] Extract approved lore if Questions 6–7 authorize it.
- [ ] Move the future-ideas document into `ideas/`.
- [ ] Remove or reframe content that describes a committed direction as merely
      optional, according to owner responses.
- [ ] Ensure ideas use current pages as links rather than independently asserting
      current mechanics.

Deliverable: no file requires readers to infer whether a paragraph is current,
directional, lore, or speculative.

### Phase 4 — Reconcile current contracts and links

- [ ] Apply R1–R7 using the approved responses.
- [ ] Add the canonical contract-ownership table.
- [ ] Reduce duplicated contract prose to the approved scope.
- [ ] Update all relative links within `docs/design/`.
- [ ] Update inbound links from other docs, plans, todos, and root documentation.
- [ ] Do not retain duplicate files or old-path aliases unless explicitly
      requested.

Deliverable: all links target one canonical physical document.

### Phase 5 — Validate and record deferred plan extraction

- [ ] Run metadata/index validation.
- [ ] Run relative Markdown-link validation.
- [ ] Search for old paths and obsolete titles.
- [ ] Review the diff for accidental mechanic changes.
- [ ] Record, but do not execute, a follow-up audit of `docs/plans/` for current
      contracts not represented under `docs/design/current/`.

Known deferred extraction candidate:
`docs/design/tuning-profiles.md:73-77` currently delegates retained
resource-engine behavior to the completed Stage 4 plan. This violates the target
self-contained authority model and should seed the later plan-audit discussion.

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

### Link and path tests

- Validate every relative Markdown link beneath `docs/design/` resolves.
- Search the repository for:
  - `docs/ideas.md`;
  - `2026-07-20-design-direction-governance-sandbox.md`;
  - old `docs/design/<topic>.md` paths.
- Update intentional citations; report any intentionally retained historical
  text rather than silently leaving a broken path.

### Content review

- Compare moved current pages before and after normalization to ensure mechanics
  did not change unintentionally.
- Review every use of “current,” “approved,” “future,” “decision,” “idea,” and
  “outside this design” in the migrated corpus.
- Verify each G-number remains unique and searchable.
- Verify no idea is presented as an implementation requirement.
- Verify lore does not imply an unstated current mechanic.

### Repository checks

- Run `git diff --check`.
- Confirm no generated index, build output, or machine-local configuration is
  included in the change.
- Update `CHANGELOG.md` only if the reconciliation intentionally changes
  player-facing design, not for path-only documentation organization.

## Acceptance criteria

- [ ] `docs/design/` contains `current/`, `direction/`, `lore/`, and `ideas/`,
      each with an indexed README.
- [ ] Every design document has explicit type, status, authority, and horizon
      metadata.
- [ ] Root and subfolder README files explain agent behavior and authority
      precedence in plain language.
- [ ] Current mechanical contracts are located under `current/` and remain
      mechanically unchanged unless an owner response explicitly approves a
      reconciliation change.
- [ ] The founding document remains identifiable as the project's foundation
      and retains stable G1–G22 references.
- [ ] Committed direction is distinguishable from current implementation.
- [ ] Lore is distinguishable from mechanical contracts.
- [ ] Ideas are explicitly non-authoritative and contain no accidental current
      requirements.
- [ ] R1–R7 are resolved or explicitly documented as deferred conflicts.
- [ ] Relative links resolve and repository references use canonical new paths.
- [ ] Package-index searches can reliably include current design and exclude
      ideas using metadata.
- [ ] No duplicate compatibility copies of moved documents remain.
- [ ] A separate follow-up need for extracting current contracts from plans is
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
  `docs/ideas.md:1-3`;
  `docs/2026-07-20-design-direction-governance-sandbox.md:1-9`.
- The current index mixes approved contracts, a completed implementation plan,
  the founding direction, and future ideas in one navigation section. Evidence:
  `docs/design/README.md:43-49`.
- No relevant institutional solution documents were found under
  `docs/solutions/`.
- The repository is Git-based and uses `.gitignore`; moved documents should use
  normal Git-aware file operations.

## Related documents

- `docs/design/README.md`
- `docs/ideas.md`
- `docs/2026-07-20-design-direction-governance-sandbox.md`
- `docs/architecture.md`
- `AGENTS.md`
