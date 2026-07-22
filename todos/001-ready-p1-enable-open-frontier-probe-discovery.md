---
status: ready
priority: p1
issue_id: 001
tags: [scouting, probes, frontier, progression, tui, ux]
dependencies: []
---
# Enable Open-Frontier Probe Discovery

## Problem Statement

A probe can currently identify an unknown system only when that system happens to be an intermediate stop on a route to a system that is already identified. Reveal scans create anonymous indications, but anonymous systems cannot be targeted. Consequently, there is no general player action for investigating a detected system that is not conveniently located on a route between already identified systems.

This is a progression gap, not only a discoverability problem. Lowering the per-leg jump limit can sometimes force useful intermediate stops, but it cannot provide systematic outward exploration and may offer no productive route in a valid generated frontier. The playable expansion loop needs a player-safe way to send probes toward anonymous frontier indications without revealing their hidden identity or facts before observation.

## Findings

- `docs/design/current/scouting-and-knowledge.md` defines four knowledge levels. Only `IdentifiedSummary` and `Complete` systems are targetable; `Anonymous` systems are deliberately not targetable.
- Initial knowledge identifies systems within one maximum probe jump and creates anonymous indications at two or three geometric legs. More distant systems remain unknown.
- `crates/game-core/src/knowledge.rs` implements that initial one-/two-/three-leg knowledge boundary in `initial_origin_knowledge()`.
- `crates/game-core/src/ships.rs` implements the actual expansion mechanism: each probe stop creates a complete observation of that system and an anonymous reveal scan around the stop.
- Core coverage proves that multi-leg probes can traverse and reveal hidden stops (`crates/game-core/tests/ships_expansion.rs`) and that player routes redact hidden stop identities (`crates/game-core/tests/routing_knowledge.rs`). The simulation mechanism is therefore present.
- `crates/game-tui/src/state.rs` offers probe targets only from `PlayingView.systems`, excluding the source. This correctly excludes anonymous/unknown systems, but gives the player no direct action on frontier fog or anonymous indications.
- Probe planning defaults to the authored maximum jump limit. A player can type a lower override and inspect the resulting redacted route, but this is the only apparent way to deliberately create hidden intermediate stops.
- `crates/game-tui/src/render/mod.rs` labels route entries as `-- hidden stop --`, but neither the mission planner nor contextual Help explains that visiting those stops is how new systems become identified.
- The probe reveal scan adds anonymous existence facts. Those facts alone do not make a system targetable, so further progress still depends on the indicated system coincidentally appearing as an intermediate stop on a route to an already identified target.
- This path constraint means reachable frontier systems can remain operationally undiscoverable even though the player has received an existence indication for them.
- Generated frontiers are not guaranteed to be connected. That remains approved world texture and must not be “fixed” with a statistical world-quality gate. However, a reachable anonymous indication should have an intentional investigation action rather than relying on route coincidence.

## Proposed Solution

**Approach:**

1. Change the scouting design so a probe may investigate an `Anonymous` indication while expeditions remain limited to `IdentifiedSummary` or `Complete` targets.
2. Project anonymous indications through a typed, opaque player-safe handle or survey choice. The UI may distinguish indications for selection, but must not receive or display the underlying system `ContentId`, exact position, or map facts.
3. Resolve the selected indication to its physical system only inside the trusted application/core boundary, derive a deterministic route under the chosen jump limit, and retain existing route redaction for all unobserved stops.
4. On probe arrival, keep the existing complete observation, reveal scan, communication delay, and report-admission behavior. The indication becomes an ordinary identified/complete system only when its observation is received.
5. Add deterministic Tier 1 core, application, and TUI scenarios proving outward discovery of an anonymous system that is not an intermediate stop on any route to an initially identified target.
6. Keep lower-jump hidden-stop surveying as an additional tactic and explain both discovery paths in the probe planner and contextual Help.

**Why this approach:**

Anonymous knowledge already represents a detected physical system and is created specifically by frontier scans. Making that indication probe-targetable closes the progression loop without making unknown systems targetable or exposing hidden facts. It also preserves the distinction between scouting and settlement: expeditions still require identified destinations.

**Trade-offs / risks:**

- This intentionally changes the approved `Anonymous` targetability rule and requires a design-document update.
- An opaque handle must remain stable enough for a planning interaction without becoming a disguised leaked system identity.
- The application/core boundary must revalidate that the indication still exists and is still legally targetable when launching.
- No acceptance criterion should require every generated seed to offer indefinite expansion or a connected frontier.

## Recommended Action

Treat probe-targetable anonymous indications as the proposed design correction. First specify the opaque indication identity, targetability, route assessment, stale-state revalidation, and redaction contract in `docs/design/current/scouting-and-knowledge.md`. Then implement the core/application intent and projection changes, followed by TUI selection and guidance. Validate with a hand-computable fixture where the new target is reachable but cannot be discovered as a route stop to any initially identified system. Do not alter generator ranges or add connectivity screening.

## Technical Details

**Affected files/assets:**

- `crates/game-core/src/knowledge.rs` - anonymous indication identity and legal targeting contract.
- `crates/game-core/src/ships.rs` - probe assessment/launch support for anonymous indications.
- `crates/game-core/tests/ships_expansion.rs` - outward-discovery fixture and retained multi-leg behavior evidence.
- `crates/game-app/src/lib.rs` - opaque player-safe indication projection and probe assessment intents.
- `crates/game-app/src/tests.rs` - application-level initial-knowledge-to-new-target scenario.
- `crates/game-tui/src/state.rs` - probe planning and target/override interaction.
- `crates/game-tui/src/render/mod.rs` - mission planner and contextual Help guidance.
- `crates/game-tui/src/state_tests.rs` - semantic interaction coverage for selecting a discovery-producing route.
- `docs/design/current/scouting-and-knowledge.md` - approve and document probe-only anonymous targeting.

**Related systems:**

- Origin knowledge and delayed transmissions
- Geometric route assessment and redaction
- Probe Shipyard projects and launch planning
- Frontier map/list projection

**Data/content impact:**

- Save data affected? No; sessions are not persisted.
- Serialized assets or prefabs affected? No.
- Migration or content reimport needed? No.

## Resources

- **Review/PR/changeset:** PR #16 / release `v0.8.0`
- **Related issue/card:** None
- **Log/capture:** Player report: “I'm not sure how to discover systems other than the ones shown at game start.”
- **Documentation:** `docs/design/current/scouting-and-knowledge.md`
- **Similar pattern:** `crates/game-core/tests/ships_expansion.rs::probe_duration_one_multileg_stops_reveal_and_launch_rejections_are_exact`

## Acceptance Criteria

- [ ] The approved scouting design permits probes, but not expeditions, to target anonymous indications through a documented player-safe contract.
- [ ] Anonymous survey choices expose no underlying system `ContentId`, exact position, body/slot facts, resources, or runtime state.
- [ ] Probe assessment and launch atomically revalidate the selected indication, route, jump limit, asset, and Energy commitment.
- [ ] A short deterministic fixture discovers an anonymous reachable system that is not an intermediate stop on any route to an initially identified target.
- [ ] The system remains anonymous during travel and becomes identified/complete only after the observation transmission is received.
- [ ] Existing lower-jump multi-leg discovery and hidden-stop redaction behavior remains supported.
- [ ] The probe planner and contextual Help explain anonymous surveys, hidden survey stops, launch, time advancement, and delayed reports.
- [ ] No-route cases are communicated honestly and do not trigger generator tuning, connectivity guarantees, or statistical world-quality gates.
- [ ] Manual playtesting confirms that a player unfamiliar with source documentation can deliberately expand the known frontier.
- [ ] Relevant formatting, Clippy, application, core, and TUI tests pass.

## Work Log

### 2026-07-21 - Initial Triage

**By:** Pi coding assistant

**Actions:**
- Reviewed the approved scouting and expansion design contracts.
- Traced initial knowledge, probe assessment, TUI target selection, jump override input, stop observation, reveal scans, and delayed report receipt.
- Confirmed existing core tests cover hidden multi-leg stops and route redaction.
- Initially classified the issue as a ready P2 player-facing discovery/affordance gap rather than a missing core probe simulation.

**Learnings:**
- The intended implementation can discover systems beyond the initial list, but only indirectly through probe stops.
- The only deliberate TUI control for producing additional stops is an unexplained numeric jump-limit override.
- Anonymous reveal results cannot currently be selected for follow-up.

### 2026-07-21 - Progression Gap Confirmed

**By:** User and Pi coding assistant

**Actions:**
- Re-evaluated the path constraint after the user identified that off-route systems have no discovery action.
- Promoted the todo from P2 usability work to a P1 progression/design correction.
- Recommended probe-only targeting of anonymous indications through an opaque player-safe handle.

**Learnings:**
- Better Help text cannot solve the full issue: a valid reachable system may never lie on a route to an already identified target.
- Anonymous indications need an intentional investigation mechanic to support open-frontier expansion.
- Expeditions should remain restricted to identified systems, preserving scouting as the prerequisite for settlement.

## Notes

- Preserve the rule that an individual generated seed is not defective merely because its frontier is disconnected or locally exhausted.
- Do not expose hidden identities to make route planning easier.
