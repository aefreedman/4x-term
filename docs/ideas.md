# Future Feature Ideas

This document records promising directions that are not current implementation requirements. An idea must move into an approved design document and focused implementation plan before it becomes a contract.

## Slot restrictions and bonuses

Stage 4 development slots are deliberately generic: any approved development may occupy any empty slot, and slots provide no inherent modifiers.

A later feature could make bodies and slots strategically distinct through:

- restrictions on which development kinds a slot can support;
- bonuses or penalties to production throughput;
- Energy-upkeep modifiers;
- construction-work modifiers;
- Battery-capacity modifiers; or
- development-specific suitability, such as extraction-oriented or Energy-oriented slots.

This should be introduced only when body and slot differences support a concrete strategic decision. Before implementation, define:

- the player-visible reason for each restriction or modifier;
- whether compatibility belongs to the body, slot, development recipe, or a relationship between them;
- how valid construction targets are presented;
- how world generation constructively provides any required compatible slots;
- how damaged, ruined, and reclaimed developments retain or recalculate modifiers; and
- short Tier 1 scenarios demonstrating that the added constraint creates a meaningful choice.

Do not add speculative slot-type, compatibility, or bonus fields to the Stage 4 schema.

## Automatic Energy curtailment

Stage 4 Energy Collectors continue operating and incur normal upkeep when storage is full; Energy left after same-tick spending overflows explicitly.

A later feature could automatically curtail Collector output when Energy cannot be consumed or retained, potentially avoiding some operating upkeep. Before implementation, define:

- whether curtailment is automatic, player-controlled, or policy-driven;
- whether upkeep is avoided entirely or only partially;
- how expected same-tick demand is calculated without order-dependent behavior;
- how curtailed potential differs from produced-and-overflowed Energy in accounting and presentation; and
- a Tier 1 scenario where dispatch control creates a meaningful decision rather than merely removing visible waste.

## Partial infrastructure operation

Stage 4 development operation is all-or-nothing per tick: a functional development receives its complete upkeep and recipe inputs and produces its complete consequence, or consumes and produces nothing.

A later feature could allow partial upkeep, partial recipe consumption, or proportionally reduced output. Before implementation, define:

- which development kinds support partial operation;
- whether scaling is continuous or uses discrete operating bands;
- deterministic rounding and physical-resource reconciliation;
- how partial operation interacts with priority, shortages, and automatic curtailment; and
- a Tier 1 scenario where partial operation creates a meaningful choice rather than obscuring resource accounting.

## Extractor expansion, upgrades, and stacking

Stage 4 permits at most one queued or installed Extractor assignment per deposit. Extractor throughput is therefore fixed by that development’s approved recipe rather than increased by placing additional Extractors on the same deposit.

A later feature could expand extraction through:

- upgrading an existing Extractor;
- development states or tiers with greater throughput or efficiency;
- adding specialized slots or attached facilities;
- allowing multiple Extractors to share one sufficiently large deposit; or
- logistical investments that increase effective extraction without another mine.

Before implementation, define construction and replacement semantics, throughput and upkeep scaling, deposit contention, slot usage, deterministic ordering, and a Tier 1 scenario where expansion creates a meaningful investment choice.

## Multiple production chains

Stage 4 implements exactly one raw-to-refined production chain: `core:ore` → `core:alloy`.

A later feature could add multiple raw resources, intermediate products, refined materials, or branching recipes. Before implementation, define:

- the strategic responsibility served by each additional resource;
- which deposits and developments produce or consume it;
- whether Refineries select one recipe, support several recipes, or require specialization;
- deterministic contention and priority across chains;
- how the frontier constructively supplies required raw-resource kinds without imposing balance floors; and
- short Tier 1 scenarios showing that each added chain creates a distinct decision rather than additional bookkeeping.

## Differentiated development recipes

Stage 4 uses minimal recipe differentiation to avoid a bootstrap cycle: the first Refinery is constructed from Energy and Ore, while Collectors, Batteries, and Extractors use Energy and Alloy. All recipes remain within the single Ore → Alloy chain.

As the resource catalog expands, later development kinds could require broader combinations of raw, refined, intermediate, or tertiary goods. Recipe differentiation should be designed together with multiple production chains so each material requirement traces to a concrete strategic responsibility. Before implementation, define supply paths, substitution rules if any, construction commitment accounting, frontier availability responsibilities, and Tier 1 scenarios demonstrating meaningful specialization rather than arbitrary recipe complexity.
