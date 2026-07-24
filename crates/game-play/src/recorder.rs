use game_tui::{PlaytestEvent, PlaytestObserver, TraceIntentKind};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufWriter, Write},
    time::Instant,
};

const TRACE_ENVELOPE_VERSION: u32 = 1;
const SUMMARY_SCHEMA_VERSION: u32 = 1;

pub struct RonlRecorder {
    output: BufWriter<File>,
    started_at: Instant,
    next_sequence: u64,
    accumulator: SummaryAccumulator,
}

impl RonlRecorder {
    #[must_use]
    pub fn new(output: File) -> Self {
        Self {
            output: BufWriter::new(output),
            started_at: Instant::now(),
            next_sequence: 1,
            accumulator: SummaryAccumulator::default(),
        }
    }

    pub fn finish(mut self, summary_file: File, orderly: bool) -> Result<CompletedTrace, String> {
        self.output
            .flush()
            .map_err(|error| format!("could not flush the playtest trace: {error}"))?;
        let duration_milliseconds = milliseconds(self.started_at.elapsed().as_millis());
        let summary = self.accumulator.finish(orderly, duration_milliseconds);
        let event_count = summary.event_count;
        let final_tick = summary.final_tick;
        let encoded = ron::ser::to_string(&summary)
            .map_err(|error| format!("could not encode the playtest summary: {error}"))?;
        let mut summary_file = BufWriter::new(summary_file);
        summary_file
            .write_all(encoded.as_bytes())
            .and_then(|()| summary_file.write_all(b"\n"))
            .and_then(|()| summary_file.flush())
            .map_err(|error| format!("could not write the playtest summary: {error}"))?;
        Ok(CompletedTrace {
            event_count,
            final_tick,
        })
    }
}

impl PlaytestObserver for RonlRecorder {
    fn observe(&mut self, event: &PlaytestEvent) -> Result<(), String> {
        let sequence = self.next_sequence;
        let elapsed_milliseconds = milliseconds(self.started_at.elapsed().as_millis());
        let line = TraceLine {
            trace_envelope_version: TRACE_ENVELOPE_VERSION,
            sequence,
            elapsed_milliseconds,
            event,
        };
        let encoded_line = ron::ser::to_string(&line)
            .map_err(|error| format!("could not encode playtest event {sequence}: {error}"))?;
        self.output
            .write_all(encoded_line.as_bytes())
            .and_then(|()| self.output.write_all(b"\n"))
            .and_then(|()| self.output.flush())
            .map_err(|error| format!("could not write playtest event {sequence}: {error}"))?;

        self.accumulator
            .observe(sequence, elapsed_milliseconds, event)?;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| "playtest event sequence exhausted u64".to_owned())?;
        Ok(())
    }
}

#[derive(Serialize)]
struct TraceLine<'a> {
    trace_envelope_version: u32,
    sequence: u64,
    elapsed_milliseconds: u64,
    event: &'a PlaytestEvent,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CompletedTrace {
    pub event_count: u64,
    pub final_tick: Option<u64>,
}

#[derive(Default)]
struct SummaryAccumulator {
    event_count: u64,
    events_by_family: BTreeMap<String, u64>,
    intent_attempts_by_kind: BTreeMap<String, u64>,
    intent_acceptances_by_kind: BTreeMap<String, u64>,
    rejections_by_reason: BTreeMap<String, u64>,
    assessments_by_kind_and_availability: BTreeMap<String, u64>,
    draft_transitions: BTreeMap<String, u64>,
    surface_opens: BTreeMap<String, u64>,
    tick_sources: BTreeMap<String, u64>,
    stock_deltas: BTreeMap<String, DeltaSummary>,
    population_deltas: BTreeMap<String, DeltaSummary>,
    selection_changes: u64,
    milestones: Vec<MilestoneSummary>,
    last_sequence: Option<u64>,
    last_elapsed_milliseconds: Option<u64>,
    final_tick: Option<u64>,
    commitment_since_tick: bool,
    in_possible_banking_window: bool,
    possible_banking_windows_inferred: u64,
}

impl SummaryAccumulator {
    fn observe(
        &mut self,
        sequence: u64,
        elapsed_milliseconds: u64,
        event: &PlaytestEvent,
    ) -> Result<(), String> {
        if self
            .last_sequence
            .is_some_and(|previous| sequence <= previous)
        {
            return Err(format!(
                "playtest summary received duplicate or out-of-order sequence {sequence}"
            ));
        }
        if self
            .last_elapsed_milliseconds
            .is_some_and(|previous| elapsed_milliseconds < previous)
        {
            return Err(format!(
                "playtest summary received decreasing elapsed time at sequence {sequence}"
            ));
        }

        self.event_count += 1;
        self.last_sequence = Some(sequence);
        self.last_elapsed_milliseconds = Some(elapsed_milliseconds);
        *self
            .events_by_family
            .entry(event_family(event).to_owned())
            .or_default() += 1;
        if let Some(tick) = event_tick(event) {
            self.final_tick = Some(tick);
        }

        match event {
            PlaytestEvent::IntentDispatched { kind, .. } => {
                increment(&mut self.intent_attempts_by_kind, format!("{kind:?}"));
            }
            PlaytestEvent::IntentResolved {
                kind,
                accepted,
                limiting_reason,
                ..
            } => {
                if *accepted {
                    increment(&mut self.intent_acceptances_by_kind, format!("{kind:?}"));
                    if is_commitment(*kind) {
                        self.commitment_since_tick = true;
                        self.in_possible_banking_window = false;
                    }
                } else {
                    increment(
                        &mut self.rejections_by_reason,
                        limiting_reason
                            .clone()
                            .unwrap_or_else(|| "Unclassified".to_owned()),
                    );
                }
            }
            PlaytestEvent::AssessmentResolved {
                kind, available, ..
            } => increment(
                &mut self.assessments_by_kind_and_availability,
                format!(
                    "{kind:?}.{}",
                    if *available {
                        "Available"
                    } else {
                        "Unavailable"
                    }
                ),
            ),
            PlaytestEvent::DraftChanged {
                kind, transition, ..
            } => increment(
                &mut self.draft_transitions,
                format!("{kind:?}.{transition:?}"),
            ),
            PlaytestEvent::SurfaceChanged { to, .. } => {
                increment(
                    &mut self.surface_opens,
                    format!("{:?}.{:?}", to.screen, to.modal),
                );
            }
            PlaytestEvent::TickAdvanced {
                source,
                stock_changes,
                population_changes,
                ..
            } => {
                increment(&mut self.tick_sources, format!("{source:?}"));
                if !self.commitment_since_tick && !self.in_possible_banking_window {
                    self.possible_banking_windows_inferred += 1;
                    self.in_possible_banking_window = true;
                }
                if self.commitment_since_tick {
                    self.in_possible_banking_window = false;
                }
                self.commitment_since_tick = false;
                for change in stock_changes {
                    self.stock_deltas
                        .entry(format!("{}::{}", change.system_id, change.resource_id))
                        .or_default()
                        .observe(change.before, change.after);
                }
                for change in population_changes {
                    self.population_deltas
                        .entry(change.system_id.clone())
                        .or_default()
                        .observe(change.before, change.after);
                }
            }
            PlaytestEvent::SelectionChanged { .. } => self.selection_changes += 1,
            PlaytestEvent::MilestoneObserved { kind, tick } => {
                let previous = self.milestones.last();
                self.milestones.push(MilestoneSummary {
                    milestone: format!("{kind:?}"),
                    sequence,
                    elapsed_milliseconds,
                    tick: *tick,
                    elapsed_since_previous_milliseconds: previous
                        .map(|value| elapsed_milliseconds - value.elapsed_milliseconds),
                    ticks_since_previous: previous.and_then(|value| tick.checked_sub(value.tick)),
                });
            }
            PlaytestEvent::TraceStarted { .. }
            | PlaytestEvent::StartupAction { .. }
            | PlaytestEvent::TraceEnded { .. } => {}
        }
        Ok(())
    }

    fn finish(self, orderly: bool, duration_milliseconds: u64) -> PlaytestSummary {
        PlaytestSummary {
            summary_schema_version: SUMMARY_SCHEMA_VERSION,
            status: if orderly {
                SummaryStatus::Orderly
            } else {
                SummaryStatus::Incomplete
            },
            duration_milliseconds,
            final_tick: self.final_tick,
            event_count: self.event_count,
            events_by_family: self.events_by_family,
            milestones: self.milestones,
            intent_attempts_by_kind: self.intent_attempts_by_kind,
            intent_acceptances_by_kind: self.intent_acceptances_by_kind,
            rejections_by_reason: self.rejections_by_reason,
            assessments_by_kind_and_availability: self.assessments_by_kind_and_availability,
            draft_transitions: self.draft_transitions,
            surface_opens: self.surface_opens,
            tick_sources: self.tick_sources,
            selection_changes: self.selection_changes,
            stock_deltas: self.stock_deltas,
            population_deltas: self.population_deltas,
            possible_banking_windows_inferred: self.possible_banking_windows_inferred,
        }
    }
}

fn increment(values: &mut BTreeMap<String, u64>, key: String) {
    *values.entry(key).or_default() += 1;
}

fn is_commitment(kind: TraceIntentKind) -> bool {
    matches!(
        kind,
        TraceIntentKind::Construction
            | TraceIntentKind::DevelopmentOperation
            | TraceIntentKind::Habitat
            | TraceIntentKind::EnqueueProbe
            | TraceIntentKind::EnqueueExpedition
            | TraceIntentKind::LaunchProbe
            | TraceIntentKind::LaunchExpedition
    )
}

#[derive(Default, Serialize)]
struct DeltaSummary {
    observations: u64,
    increases: u64,
    decreases: u64,
    absolute_magnitude: u64,
}

impl DeltaSummary {
    fn observe(&mut self, before: u64, after: u64) {
        self.observations += 1;
        self.absolute_magnitude = self
            .absolute_magnitude
            .saturating_add(before.abs_diff(after));
        match after.cmp(&before) {
            std::cmp::Ordering::Greater => self.increases += 1,
            std::cmp::Ordering::Less => self.decreases += 1,
            std::cmp::Ordering::Equal => {}
        }
    }
}

#[derive(Serialize)]
struct PlaytestSummary {
    summary_schema_version: u32,
    status: SummaryStatus,
    duration_milliseconds: u64,
    final_tick: Option<u64>,
    event_count: u64,
    events_by_family: BTreeMap<String, u64>,
    milestones: Vec<MilestoneSummary>,
    intent_attempts_by_kind: BTreeMap<String, u64>,
    intent_acceptances_by_kind: BTreeMap<String, u64>,
    rejections_by_reason: BTreeMap<String, u64>,
    assessments_by_kind_and_availability: BTreeMap<String, u64>,
    draft_transitions: BTreeMap<String, u64>,
    surface_opens: BTreeMap<String, u64>,
    tick_sources: BTreeMap<String, u64>,
    selection_changes: u64,
    stock_deltas: BTreeMap<String, DeltaSummary>,
    population_deltas: BTreeMap<String, DeltaSummary>,
    // A window is a contiguous run of committed ticks with no accepted physical
    // commitment since the previous tick. It is a proxy, never player intent.
    possible_banking_windows_inferred: u64,
}

#[derive(Serialize)]
enum SummaryStatus {
    Orderly,
    Incomplete,
}

#[derive(Serialize)]
struct MilestoneSummary {
    milestone: String,
    sequence: u64,
    elapsed_milliseconds: u64,
    tick: u64,
    elapsed_since_previous_milliseconds: Option<u64>,
    ticks_since_previous: Option<u64>,
}

fn event_family(event: &PlaytestEvent) -> &'static str {
    match event {
        PlaytestEvent::TraceStarted { .. } => "TraceStarted",
        PlaytestEvent::StartupAction { .. } => "StartupAction",
        PlaytestEvent::SurfaceChanged { .. } => "SurfaceChanged",
        PlaytestEvent::DraftChanged { .. } => "DraftChanged",
        PlaytestEvent::IntentDispatched { .. } => "IntentDispatched",
        PlaytestEvent::IntentResolved { .. } => "IntentResolved",
        PlaytestEvent::AssessmentResolved { .. } => "AssessmentResolved",
        PlaytestEvent::TickAdvanced { .. } => "TickAdvanced",
        PlaytestEvent::SelectionChanged { .. } => "SelectionChanged",
        PlaytestEvent::MilestoneObserved { .. } => "MilestoneObserved",
        PlaytestEvent::TraceEnded { .. } => "TraceEnded",
    }
}

fn event_tick(event: &PlaytestEvent) -> Option<u64> {
    match event {
        PlaytestEvent::TraceStarted { current_tick, .. }
        | PlaytestEvent::StartupAction { current_tick, .. }
        | PlaytestEvent::SurfaceChanged { current_tick, .. }
        | PlaytestEvent::DraftChanged { current_tick, .. }
        | PlaytestEvent::IntentDispatched { current_tick, .. }
        | PlaytestEvent::IntentResolved { current_tick, .. }
        | PlaytestEvent::AssessmentResolved { current_tick, .. }
        | PlaytestEvent::SelectionChanged { current_tick, .. } => *current_tick,
        PlaytestEvent::TickAdvanced { resulting_tick, .. } => Some(*resulting_tick),
        PlaytestEvent::MilestoneObserved { tick, .. } => Some(*tick),
        PlaytestEvent::TraceEnded { final_tick, .. } => *final_tick,
    }
}

fn milliseconds(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_tui::{
        DraftTransition, MilestoneKind, StartupActionKind, TickSource, TraceDraftKind,
        TraceIntentContext,
    };

    fn started() -> PlaytestEvent {
        PlaytestEvent::TraceStarted {
            schema_version: 1,
            package_version: "test".into(),
            seed: 7,
            profile_name: "starter".into(),
            current_tick: None,
        }
    }

    #[test]
    fn constructed_stream_produces_typed_counts_milestones_and_inferences() {
        let mut accumulator = SummaryAccumulator::default();
        accumulator.observe(1, 0, &started()).unwrap();
        accumulator
            .observe(
                2,
                4,
                &PlaytestEvent::IntentDispatched {
                    kind: TraceIntentKind::AdvanceOneTick,
                    context: TraceIntentContext::default(),
                    current_tick: Some(0),
                },
            )
            .unwrap();
        accumulator
            .observe(
                3,
                6,
                &PlaytestEvent::TickAdvanced {
                    source: TickSource::Direct,
                    from_tick: 0,
                    resulting_tick: 1,
                    stock_changes: Vec::new(),
                    population_changes: Vec::new(),
                    newly_identified_systems: Vec::new(),
                    mission_changes: 0,
                },
            )
            .unwrap();
        accumulator
            .observe(
                4,
                8,
                &PlaytestEvent::MilestoneObserved {
                    kind: MilestoneKind::OriginCommandable,
                    tick: 1,
                },
            )
            .unwrap();
        let summary = accumulator.finish(true, 10);

        assert_eq!(summary.event_count, 4);
        assert_eq!(summary.final_tick, Some(1));
        assert_eq!(summary.events_by_family["TickAdvanced"], 1);
        assert_eq!(summary.intent_attempts_by_kind["AdvanceOneTick"], 1);
        assert_eq!(summary.tick_sources["Direct"], 1);
        assert_eq!(summary.milestones[0].milestone, "OriginCommandable");
        assert_eq!(summary.possible_banking_windows_inferred, 1);
    }

    #[test]
    fn rejections_and_draft_transitions_are_typed() {
        let mut accumulator = SummaryAccumulator::default();
        accumulator
            .observe(
                1,
                1,
                &PlaytestEvent::IntentResolved {
                    kind: TraceIntentKind::Construction,
                    accepted: false,
                    limiting_reason: Some("InsufficientResource".into()),
                    draft_disposition: Some("Retain".into()),
                    project_id: None,
                    ship_id: None,
                    current_tick: Some(3),
                },
            )
            .unwrap();
        accumulator
            .observe(
                2,
                2,
                &PlaytestEvent::DraftChanged {
                    kind: TraceDraftKind::Construction,
                    transition: DraftTransition::Retained,
                    current_tick: Some(3),
                },
            )
            .unwrap();
        let summary = accumulator.finish(false, 3);
        assert_eq!(summary.rejections_by_reason["InsufficientResource"], 1);
        assert_eq!(summary.draft_transitions["Construction.Retained"], 1);
    }

    #[test]
    fn incomplete_stream_does_not_invent_a_trace_end() {
        let mut accumulator = SummaryAccumulator::default();
        accumulator
            .observe(
                1,
                2,
                &PlaytestEvent::StartupAction {
                    kind: StartupActionKind::PreviewGenerated,
                    succeeded: true,
                    accepted_seed: None,
                    accepted_profile_name: None,
                    current_tick: None,
                },
            )
            .unwrap();
        let summary = accumulator.finish(false, 3);

        assert!(matches!(summary.status, SummaryStatus::Incomplete));
        assert_eq!(summary.events_by_family.get("TraceEnded"), None);
        assert_eq!(summary.final_tick, None);
    }

    #[test]
    fn duplicate_out_of_order_and_decreasing_time_are_rejected() {
        let mut accumulator = SummaryAccumulator::default();
        accumulator.observe(2, 3, &started()).unwrap();
        assert!(accumulator.observe(2, 4, &started()).is_err());
        assert!(accumulator.observe(1, 4, &started()).is_err());
        assert!(accumulator.observe(3, 2, &started()).is_err());
    }

    #[test]
    fn equal_streams_serialize_identically() {
        fn encoded() -> String {
            let mut accumulator = SummaryAccumulator::default();
            accumulator.observe(1, 1, &started()).unwrap();
            ron::ser::to_string(&accumulator.finish(true, 4)).unwrap()
        }
        assert_eq!(encoded(), encoded());
    }
}
