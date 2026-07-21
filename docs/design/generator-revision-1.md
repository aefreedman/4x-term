---
title: "Frontier Generator Revision 1"
type: design
status: approved
---
# Frontier Generator Revision 1

This page freezes the deterministic engineering algorithm behind
`core:frontier_world@1`. Profile values remain editable; changing an algorithm
on this page requires a new generator revision.

See [World Generation](world-generation.md), [Generator Identity](generator-identity.md),
and [Tuning Profiles](tuning-profiles.md).

## Canonical configuration bytes

Normalize the strict profile into a tree containing only signed/unsigned
integers, booleans, UTF-8 strings, sequences, and maps. Optional source fields
must be resolved or rejected before encoding; there are no floating values or
implicit defaults.

Prefix the encoding with ASCII `4XFG`, followed by encoding revision `1` as a
little-endian `u16`. Encode values recursively:

| Tag | Value encoding |
| --- | --- |
| `0x01` | unsigned integer as little-endian `u64` |
| `0x02` | signed integer as little-endian two's-complement `i64` |
| `0x03` | boolean byte `0` or `1` |
| `0x04` | UTF-8 string as little-endian `u32` byte length plus bytes |
| `0x05` | sequence as little-endian `u32` count plus values in domain order |
| `0x06` | map/struct as little-endian `u32` count plus key/value pairs |

A struct is a map whose keys are its normalized field names. Maps are sorted by
the complete canonical encoded key bytes. Sets are normalized into sorted
sequences. Stable IDs encode as strings. Ratios encode as maps containing
nonzero unsigned `numerator` and `denominator`, reduced to lowest terms.

The profile fingerprint is SHA-256 of these complete bytes. Provenance is not
part of this encoding.

## Stream derivation

Every random decision uses its own domain-separated stream. Derive a stream key
as SHA-256 of this byte sequence:

1. ASCII `4x-term.frontier-stream` followed by one zero byte;
2. canonical family string encoded as `u32` byte length plus UTF-8 bytes;
3. generator revision as little-endian `u32`;
4. world seed as little-endian `u64`;
5. stage tag as `u32` byte length plus UTF-8 bytes; and
6. canonical entity key as `u32` byte length plus bytes.

The initial SplitMix64 state is the first eight digest bytes interpreted as a
little-endian `u64`. Entity keys use canonical fixed-width integers and/or
length-prefixed stable IDs; they never use collection iteration order or ECS
IDs.

Revision-1 stage tags are:

- `noise_lattice/<octave>` keyed by signed lattice X then signed lattice Y;
- `cell_presence` keyed by row-major cell ordinal;
- `cell_jitter_x` and `cell_jitter_y` keyed by row-major cell ordinal;
- `system_strength` and `system_body_count` keyed by generated system ID;
- `body_eccentricity` and `body_slot_count` keyed by generated body ID;
- `resource_presence/<resource_id>` keyed by generated system ID;
- `resource_body_count/<resource_id>` keyed by generated system ID;
- `resource_body_pick/<resource_id>/<pick_index>` keyed by generated system ID;
- `resource_quantity/<resource_id>` keyed by generated body ID;
- `origin_body_count` keyed by `core:origin`;
- `origin_body_slot_count` keyed by generated origin body ID;
- `origin_resource_body_count/<resource_id>` keyed by `core:origin`;
- `origin_resource_body_pick/<resource_id>/<pick_index>` keyed by
  `core:origin`; and
- `origin_resource_quantity/<resource_id>` keyed by generated origin body ID.

Adding or renaming a tag is an output-affecting algorithm change.

## SplitMix64

For each next value, update and mix with wrapping `u64` arithmetic:

```text
state += 0x9E3779B97F4A7C15
z = state
z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9
z = (z ^ (z >> 27)) * 0x94D049BB133111EB
return z ^ (z >> 31)
```

A stream initialized directly to zero produces:

```text
e220a8397b1dcdaf
6e789e6aa1b965f4
06c45d188009454f
```

These are required implementation vectors.

For an unbiased integer in `0..bound`, require `bound > 0`, compute
`threshold = (0_u64.wrapping_sub(bound)) % bound`, discard values below the
threshold, and return `value % bound`. This numeric rejection is not world
rerolling.

## Discrete triangular draws

For inclusive integer `minimum <= mode <= maximum`, assign each candidate `x`
a positive checked `u128` weight.

If all three values are equal, the sole weight is `1`. If `mode == minimum`:

```text
weight(x) = maximum - x + 1
```

If `mode == maximum`:

```text
weight(x) = x - minimum + 1
```

Otherwise:

```text
x <= mode: weight(x) = 1 + (x - minimum) * (maximum - mode)
x >= mode: weight(x) = 1 + (maximum - x) * (mode - minimum)
```

Draw from the checked cumulative weight in ascending candidate order. Scalar
strength/eccentricity triangles first convert their approved hundredths to the
corresponding inclusive integer range. Truncating a resource-bearing-body count
removes invalid candidates and samples from the remaining original weights; it
does not clamp a prior result.

Uniform body selection without replacement draws an unbiased index from the
remaining bodies in stable body order, removes that body, and repeats.

## Integer value noise

Spatial bounds are minimum-inclusive and maximum-exclusive on X/Y. Width and
height must be positive multiples of placement-cell width/height. Cells are
ordered by Y row, then X column. The cell containing `(0, 0)` is reserved for the
origin and excluded from frontier placement. Target count must be at least `1`
and no greater than eligible-cell count plus one.

For octave `o`:

```text
wavelength_o = base_wavelength / lacunarity^o
```

Validation requires nonzero lacunarity and an exact positive integer wavelength
for every configured octave. At each lattice point, derive the
`noise_lattice/<o>` stream from signed lattice coordinates and use the low
sixteen bits of its first value as the lattice value in `0..=65_535`.

Evaluate value noise at the center of each placement cell. Negative coordinates
use Euclidean floor division. Convert the fractional position inside the lattice
square to unsigned Q32.32. Apply quintic fade
`6t^5 - 15t^4 + 10t^3`, then bilinearly interpolate the four lattice values.
Every fixed-point multiply floors after the Q32.32 shift; signed interpolation
uses Euclidean floor division. Checked `u128`/`i128` intermediates reject on
overflow.

Octave amplitudes begin at Q32.32 `1.0`; each next amplitude multiplies by the
profile's reduced persistence ratio. Combine octave values as the checked
amplitude-weighted average, flooring once after division. The result is
`raw_noise_i` in `0..=65_535`, and cell density weight is
`raw_noise_i + 1`.

## Cell presence and jitter

Let `target_non_origin = target_system_count - 1` and `weight_sum` be the checked
sum of every eligible cell weight. Revision 1 requires `weight_sum <= u64::MAX`;
profiles exceeding that bound reject before any placement draw so the approved
unbiased `u64` sampler is the sole bounded-draw algorithm. For each cell:

```text
numerator   = target_non_origin * cell_weight
denominator = weight_sum
```

If `numerator >= denominator`, place a system. Otherwise draw an unbiased value
in `0..denominator` from the cell's `cell_presence` stream and place when it is
less than `numerator`. Do not redistribute capped probability or repair the
resulting count.

For a placed cell, draw X and Y offsets independently in the half-open ranges
`0..cell_width` and `0..cell_height` from its jitter streams. Add offsets to the
cell minimum; generated Z is zero. Half-open non-overlapping cells make generated
positions unique without a separation pass.

Sort placed cells in row-major order before assigning generated identity:

```text
generated:system_000000
generated:system_000001
...
generated:system_000000_body_000
generated:system_000000_body_000_slot_000
```

The origin retains `core:origin`; its generated bodies/slots use the same
body/slot suffix convention under the origin ID. Decimal ordinals are zero-
padded to six digits for systems and three digits for bodies/slots. Exceeding the
representable ordinal range rejects generation.

## Required mechanism tests

- canonical profile permutations produce identical bytes and fingerprint;
- each primitive/container encoding has a fixed byte vector;
- the SplitMix64 vectors above pass;
- domain changes alter a stream while unrelated streams remain unchanged;
- triangle endpoint/mode/asymmetric and truncated weights are exact;
- negative-coordinate lattice and cell boundaries are exact;
- origin-cell exclusion and half-open jitter preserve unique positions;
- valid deterministic outputs may be above or below target count; and
- invalid arithmetic/configuration returns no partial generated artifact.

These tests validate the algorithm, not whether a generated world is desirable.
