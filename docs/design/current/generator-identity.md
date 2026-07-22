---
title: Generator Identity
type: design-current
status: approved
authority: normative
horizon: current
---
# Generator Identity

A seed alone is not a world-generation identity. Reproduction depends on the generator algorithm revision and the complete normalized configuration as well as the seed.

See also:

- [World generation](world-generation.md)
- [Tuning profiles](tuning-profiles.md)
- [Frontier generator revision 1](generator-revision-1.md)
- [Stage 4b implementation plan](../../plans/2026-07-20-feature-constructive-world-generation-stage-4b-plan.md)

## Identity tuple

Every successfully generated artifact records:

- a structured generator version consisting of a `ContentId` family and a nonzero monotonic revision;
- an unsigned 64-bit seed;
- a SHA-256 fingerprint of the canonical normalized generator configuration;
- source-document provenance;
- stable generated IDs derived from deterministic generation order and kind; and
- the complete normalized generated world definition.

The first canonical version is:

```text
core:frontier_world@1
```

This is represented as family `core:frontier_world` and revision `1`.

For an identical family/revision, seed, and configuration fingerprint, generation must produce an equal normalized `WorldDefinition`.

## Configuration fingerprint

The fingerprint covers normalized, output-affecting generator configuration. Its canonical byte encoding is versioned and uses:

- fixed-width integers;
- length-prefixed UTF-8 strings; and
- canonical stable-ID and key ordering.

Semantically equivalent input ordering must therefore produce the same fingerprint.

Source provenance is deliberately not a fingerprint input. Logical source identity and source-content hash are retained as provenance, while machine-local paths are excluded. This distinguishes “what generated the world” from “where this machine loaded the source.”

Changing profile values changes the normalized fingerprint. It does not by itself change the generator revision.

## Revision policy

Any output-affecting algorithm or behavior change increments the generator revision. A newer implementation does not have to reproduce an older revision unless support for that older revision is explicitly retained.

Revision identity is not a compatibility promise for pre-generator authored worlds, removed schemas, or runtime event-log replay. Full runtime replay is a separate concern.

## Deterministic random streams

Revision 1 uses the exact canonical encoding, domain-separated SplitMix64
streams, distributions, and placement algorithm in
[Frontier Generator Revision 1](generator-revision-1.md). A stream is derived from:

- the seed;
- generator family and revision;
- a stable stage tag; and
- a canonical ordinal.

This separation prevents an unrelated added draw from silently perturbing earlier generation stages.

Bounded random values use unbiased integer rejection sampling. Weighted choices use checked cumulative integer weights in canonical candidate order. PRNG rejection removes numeric bias; it is not world rejection, rerolling, or quality screening.

Spatial probability comparisons, noise, weighted choices, and jitter use deterministic integer/fixed-point operations. Generation and validation use checked arithmetic.

## Stable generated identity

Generated systems, bodies, slots, and other generated records receive stable IDs based on deterministic generation order and kind. They are never random UUIDs or ECS entity IDs.

Canonical ordering is part of reproduction. It controls generated IDs, weighted-choice candidate order, tie-breaking, normalization, and equality of the final definition.

## Artifact and failure contract

The compiled generated-world artifact contains:

1. the reproduction identity;
2. provenance metadata; and
3. the normalized `WorldDefinition`.

Strict source parsing and canonical normalization happen before successful generation is exposed. Invalid configuration, checked-arithmetic failure, or invalid generated references reject generation without returning a partial world.
