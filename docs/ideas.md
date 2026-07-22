# Future Feature Ideas

This document records promising directions that are not current implementation requirements. An idea must move into an approved design document and focused implementation plan before it becomes a contract.

## Slot restrictions and bonuses

Development slots are deliberately generic: any approved development may occupy any empty slot, and slots provide no inherent modifiers.

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

Do not add speculative slot-type, compatibility, or bonus fields to the current schema.

## Automatic Energy curtailment

Energy Collectors continue operating and incur normal upkeep when storage is full; Energy left after same-tick spending overflows explicitly.

A later feature could automatically curtail Collector output when Energy cannot be consumed or retained, potentially avoiding some operating upkeep. Before implementation, define:

- whether curtailment is automatic, player-controlled, or policy-driven;
- whether upkeep is avoided entirely or only partially;
- how expected same-tick demand is calculated without order-dependent behavior;
- how curtailed potential differs from produced-and-overflowed Energy in accounting and presentation; and
- a Tier 1 scenario where dispatch control creates a meaningful decision rather than merely removing visible waste.

## Partial infrastructure operation

Development operation is all-or-nothing per tick: a functional development receives its complete upkeep and recipe inputs and produces its complete consequence, or consumes and produces nothing.

A later feature could allow partial upkeep, partial recipe consumption, or proportionally reduced output. Before implementation, define:

- which development kinds support partial operation;
- whether scaling is continuous or uses discrete operating bands;
- deterministic rounding and physical-resource reconciliation;
- how partial operation interacts with priority, shortages, and automatic curtailment; and
- a Tier 1 scenario where partial operation creates a meaningful choice rather than obscuring resource accounting.

## Extractor upgrades and specialization

Resource quantities are body-owned, and multiple same-body Extractors draw from
one total in stable slot order. That stacking behavior is not deferred here.

A later feature could further expand extraction through:

- upgrading an existing Extractor;
- development states or tiers with greater throughput or efficiency;
- adding specialized slots or attached facilities; or
- logistical investments that increase effective extraction without another
  mine.

Before implementation, define construction and replacement semantics,
throughput and upkeep scaling, slot usage, deterministic ordering, and a Tier 1
scenario where upgrades create a meaningful investment choice.

## Three-dimensional frontier positions

Generator revision 1 places the frontier in two dimensions with `z = 0` while
retaining the existing three-coordinate position type.

A later feature could generate systems throughout a true three-dimensional
volume if vertical separation creates enough strategic value to justify its
presentation and navigation costs. Before implementation, define:

- what decisions the third axis adds beyond additional geometric distance;
- map projection, overlap handling, depth cues, slicing, and route presentation;
- volume dimensions relative to system count, noise scale, and ship jump ranges;
- deterministic 3D density sampling and minimum-separation behavior;
- checked squared-distance bounds; and
- focused route and projection scenarios without treating connectivity or visual
  preference as generated-world quality gates.

## Cultural influence and coherent management

The current information model keeps the origin as the sole report recipient and
treats remote inhabited systems as parts of one player-directed community.

A later cultural-influence mechanic could determine the distance over which the
origin community can manage itself as one coherent community. Beyond that
influence, remote communities might require delegation, gain local priorities,
or gradually become distinct political entities without becoming adversarial
NPC factions by default.

Before implementation, define:

- whether influence is measured by geometric distance, travel time,
  communication delay, network hops, or several factors;
- how Habitats, population, institutions, communication infrastructure, and
  cultural investment extend or resist influence;
- which commands, policies, information, and resource commitments remain under
  direct origin control at each influence level;
- how remote community identity forms and whether transitions are gradual or
  threshold-based;
- how this interacts with delayed observations and delegation-by-distance; and
- short deterministic scenarios that test authority mechanics without judging a
  generated world's political quality.

## Multiple production chains

The current product implements exactly one raw-to-refined production chain: `core:ore` → `core:alloy`.

A later feature could add multiple raw resources, intermediate products, refined materials, or branching recipes. Before implementation, define:

- the strategic responsibility served by each additional resource;
- which deposits and developments produce or consume it;
- whether Refineries select one recipe, support several recipes, or require specialization;
- deterministic contention and priority across chains;
- how the frontier constructively supplies required raw-resource kinds without imposing balance floors; and
- short Tier 1 scenarios showing that each added chain creates a distinct decision rather than additional bookkeeping.

## Differentiated development recipes

Current recipes use minimal differentiation to avoid a bootstrap cycle: the first Refinery is constructed from Energy and Ore, while Collectors, Batteries, and Extractors use Energy and Alloy. All recipes remain within the single Ore → Alloy chain.

As the resource catalog expands, later development kinds could require broader combinations of raw, refined, intermediate, or tertiary goods. Recipe differentiation should be designed together with multiple production chains so each material requirement traces to a concrete strategic responsibility. Before implementation, define supply paths, substitution rules if any, construction commitment accounting, frontier availability responsibilities, and Tier 1 scenarios demonstrating meaningful specialization rather than arbitrary recipe complexity.
