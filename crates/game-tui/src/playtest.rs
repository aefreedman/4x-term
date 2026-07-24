use game_app::{
    ActionAvailability, ApplicationOutcome, ExpeditionAssessmentView, IntentKind, LimitingReason,
    PlayingView, ProbeAssessmentView, ProjectId, SessionIntent, SessionOutcome, ShipId,
    TickDeltaView,
};
use serde::Serialize;

pub const PLAYTEST_TRACE_SCHEMA_VERSION: u32 = 1;

pub trait PlaytestObserver {
    fn observe(&mut self, event: &PlaytestEvent) -> Result<(), String>;
}

#[derive(Clone, Debug, Serialize)]
pub enum PlaytestEvent {
    TraceStarted {
        schema_version: u32,
        package_version: String,
        seed: u64,
        profile_name: String,
        current_tick: Option<u64>,
    },
    StartupAction {
        kind: StartupActionKind,
        succeeded: bool,
        accepted_seed: Option<u64>,
        accepted_profile_name: Option<String>,
        current_tick: Option<u64>,
    },
    SurfaceChanged {
        from: TraceSurface,
        to: TraceSurface,
        current_tick: Option<u64>,
    },
    DraftChanged {
        kind: TraceDraftKind,
        transition: DraftTransition,
        current_tick: Option<u64>,
    },
    IntentDispatched {
        kind: TraceIntentKind,
        context: TraceIntentContext,
        current_tick: Option<u64>,
    },
    IntentResolved {
        kind: TraceIntentKind,
        accepted: bool,
        limiting_reason: Option<String>,
        draft_disposition: Option<String>,
        project_id: Option<String>,
        ship_id: Option<String>,
        current_tick: Option<u64>,
    },
    AssessmentResolved {
        kind: AssessmentKind,
        available: bool,
        limiting_reason: Option<String>,
        source_id: String,
        target_id: String,
        travel_energy: Option<u64>,
        target_knowledge: String,
        route_stop_count: Option<usize>,
        current_tick: Option<u64>,
    },
    TickAdvanced {
        source: TickSource,
        from_tick: u64,
        resulting_tick: u64,
        stock_changes: Vec<TraceStockChange>,
        population_changes: Vec<TracePopulationChange>,
        newly_identified_systems: Vec<String>,
        mission_changes: usize,
    },
    SelectionChanged {
        screen: TraceScreen,
        system_id: Option<String>,
        current_tick: Option<u64>,
    },
    MilestoneObserved {
        kind: MilestoneKind,
        tick: u64,
    },
    TraceEnded {
        orderly: bool,
        final_tick: Option<u64>,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum MilestoneKind {
    OriginCommandable,
    FirstConstructionQueued,
    FirstDevelopmentCompleted,
    FirstDevelopmentOperationChanged,
    FirstShipProjectQueued,
    FirstProbeReady,
    FirstExpeditionQueued,
    FirstProbeLaunched,
    FirstProbeAwaitingReport,
    FirstProbeReportReceived,
    FirstExpeditionReady,
    FirstExpeditionLaunched,
    FirstFoundingOutcomeReceived,
    FirstDaughterCommandable,
    FirstDaughterGovernanceAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum StartupActionKind {
    PreviewGenerated,
    PreviewRegenerated,
    PreviewGenerationFailed,
    PreviewAccepted,
    PreviewAcceptanceFailed,
    StartupAbandoned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum TraceScreen {
    Dashboard,
    SystemDetails,
    Local,
    Operations,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum TraceModal {
    Help,
    Settings,
    Editor,
    Confirmation,
    Rejection,
    Batch,
    Mission,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TraceSurface {
    pub screen: TraceScreen,
    pub modal: Option<TraceModal>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum TraceDraftKind {
    Construction,
    DevelopmentOperation,
    Habitat,
    Probe,
    Expedition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum DraftTransition {
    Started,
    Cancelled,
    Committed,
    Retained,
    Invalidated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum TraceIntentKind {
    Construction,
    DevelopmentOperation,
    Habitat,
    Alias,
    AdvanceOneTick,
    EnqueueProbe,
    EnqueueExpedition,
    CancelShipProject,
    AssessProbe,
    LaunchProbe,
    AssessExpedition,
    LaunchExpedition,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct TraceIntentContext {
    pub system_id: Option<String>,
    pub source_id: Option<String>,
    pub target_id: Option<String>,
    pub project_id: Option<String>,
    pub ship_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum AssessmentKind {
    Probe,
    Expedition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum TickSource {
    Direct,
    Batch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TraceStockChange {
    pub system_id: String,
    pub resource_id: String,
    pub before: u64,
    pub after: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TracePopulationChange {
    pub system_id: String,
    pub before: u64,
    pub after: u64,
}

impl TraceIntentKind {
    #[must_use]
    pub fn from_intent(intent: &SessionIntent) -> Self {
        match intent {
            SessionIntent::EnqueueConstruction { .. } => Self::Construction,
            SessionIntent::SetDevelopmentOperationalEnabled { .. } => Self::DevelopmentOperation,
            SessionIntent::SetHabitatGenerationEnabled { .. } => Self::Habitat,
            SessionIntent::SetSystemAlias { .. } => Self::Alias,
            SessionIntent::AdvanceOneTick => Self::AdvanceOneTick,
            SessionIntent::EnqueueShipProject {
                kind: game_app::ShipProjectKind::Probe,
                ..
            } => Self::EnqueueProbe,
            SessionIntent::EnqueueShipProject {
                kind: game_app::ShipProjectKind::Expedition,
                ..
            } => Self::EnqueueExpedition,
            SessionIntent::CancelShipProject { .. } => Self::CancelShipProject,
            SessionIntent::AssessProbeLaunch { .. } => Self::AssessProbe,
            SessionIntent::LaunchProbe { .. } => Self::LaunchProbe,
            SessionIntent::AssessExpeditionLaunch { .. } => Self::AssessExpedition,
            SessionIntent::LaunchExpedition { .. } => Self::LaunchExpedition,
        }
    }

    #[must_use]
    pub fn from_outcome(kind: IntentKind) -> Self {
        match kind {
            IntentKind::Construction => Self::Construction,
            IntentKind::DevelopmentOperation => Self::DevelopmentOperation,
            IntentKind::Habitat => Self::Habitat,
            IntentKind::Alias => Self::Alias,
            IntentKind::AdvanceOneTick => Self::AdvanceOneTick,
            IntentKind::EnqueueProbe => Self::EnqueueProbe,
            IntentKind::EnqueueExpedition => Self::EnqueueExpedition,
            IntentKind::CancelShipProject => Self::CancelShipProject,
            IntentKind::AssessProbe => Self::AssessProbe,
            IntentKind::LaunchProbe => Self::LaunchProbe,
            IntentKind::AssessExpedition => Self::AssessExpedition,
            IntentKind::LaunchExpedition => Self::LaunchExpedition,
        }
    }
}

impl TraceIntentContext {
    #[must_use]
    pub fn from_intent(intent: &SessionIntent) -> Self {
        match intent {
            SessionIntent::EnqueueConstruction { system_id, .. }
            | SessionIntent::SetDevelopmentOperationalEnabled { system_id, .. }
            | SessionIntent::SetHabitatGenerationEnabled { system_id, .. }
            | SessionIntent::SetSystemAlias { system_id, .. }
            | SessionIntent::EnqueueShipProject { system_id, .. } => Self {
                system_id: Some(system_id.to_string()),
                ..Self::default()
            },
            SessionIntent::CancelShipProject { project_id } => Self {
                project_id: Some(project_id_key(project_id)),
                ..Self::default()
            },
            SessionIntent::AssessProbeLaunch {
                source_id,
                ship_id,
                target_id,
                ..
            }
            | SessionIntent::LaunchProbe {
                source_id,
                ship_id,
                target_id,
                ..
            }
            | SessionIntent::AssessExpeditionLaunch {
                source_id,
                ship_id,
                target_id,
                ..
            }
            | SessionIntent::LaunchExpedition {
                source_id,
                ship_id,
                target_id,
                ..
            } => Self {
                source_id: Some(source_id.to_string()),
                target_id: Some(target_id.to_string()),
                ship_id: Some(ship_id_key(ship_id)),
                ..Self::default()
            },
            SessionIntent::AdvanceOneTick => Self::default(),
        }
    }
}

#[must_use]
pub fn resolved_event(outcome: &ApplicationOutcome, current_tick: Option<u64>) -> PlaytestEvent {
    PlaytestEvent::IntentResolved {
        kind: TraceIntentKind::from_outcome(outcome.intent),
        accepted: outcome.accepted,
        limiting_reason: outcome.limiting_reason.as_ref().map(limiting_reason_key),
        draft_disposition: outcome.draft_disposition.map(|value| format!("{value:?}")),
        project_id: outcome.project_id.as_ref().map(project_id_key),
        ship_id: outcome.ship_id.as_ref().map(ship_id_key),
        current_tick,
    }
}

#[must_use]
pub fn probe_assessment_event(
    assessment: &ProbeAssessmentView,
    current_tick: Option<u64>,
) -> PlaytestEvent {
    let (available, limiting_reason) = availability(&assessment.availability);
    PlaytestEvent::AssessmentResolved {
        kind: AssessmentKind::Probe,
        available,
        limiting_reason,
        source_id: assessment.source_id.to_string(),
        target_id: assessment.target_id.to_string(),
        travel_energy: assessment.travel_energy,
        target_knowledge: format!("{:?}", assessment.target_knowledge),
        route_stop_count: assessment.route.as_ref().map(|route| route.stops.len()),
        current_tick,
    }
}

#[must_use]
pub fn expedition_assessment_event(
    assessment: &ExpeditionAssessmentView,
    current_tick: Option<u64>,
) -> PlaytestEvent {
    let (available, limiting_reason) = availability(&assessment.availability);
    PlaytestEvent::AssessmentResolved {
        kind: AssessmentKind::Expedition,
        available,
        limiting_reason,
        source_id: assessment.source_id.to_string(),
        target_id: assessment.target_id.to_string(),
        travel_energy: assessment.travel_energy,
        target_knowledge: format!("{:?}", assessment.target_knowledge),
        route_stop_count: assessment.route.as_ref().map(|route| route.stops.len()),
        current_tick,
    }
}

#[must_use]
pub fn tick_event(delta: &TickDeltaView, source: TickSource) -> PlaytestEvent {
    PlaytestEvent::TickAdvanced {
        source,
        from_tick: delta.from_tick,
        resulting_tick: delta.to_tick,
        stock_changes: delta
            .stock_changes
            .iter()
            .map(|change| TraceStockChange {
                system_id: change.system_id.to_string(),
                resource_id: change.resource_id.to_string(),
                before: change.before,
                after: change.after,
            })
            .collect(),
        population_changes: delta
            .population_changes
            .iter()
            .map(|change| TracePopulationChange {
                system_id: change.system_id.to_string(),
                before: change.before,
                after: change.after,
            })
            .collect(),
        newly_identified_systems: delta
            .newly_identified_systems
            .iter()
            .map(ToString::to_string)
            .collect(),
        mission_changes: delta.mission_changes,
    }
}

#[must_use]
pub fn outcome_view(outcome: &SessionOutcome) -> Option<&PlayingView> {
    match outcome {
        SessionOutcome::Applied { view, .. }
        | SessionOutcome::ProbeLaunched { view, .. }
        | SessionOutcome::ExpeditionLaunched { view, .. } => Some(view),
        SessionOutcome::Tick(step) => Some(&step.view),
        SessionOutcome::ProbeAssessment(_)
        | SessionOutcome::ExpeditionAssessment(_)
        | SessionOutcome::Rejected(_) => None,
    }
}

fn project_id_key(id: &ProjectId) -> String {
    format!("{}#{}", id.system, id.sequence)
}

fn ship_id_key(id: &ShipId) -> String {
    format!("{}#{}", id.system, id.sequence)
}

fn availability(value: &ActionAvailability) -> (bool, Option<String>) {
    match value {
        ActionAvailability::Available => (true, None),
        ActionAvailability::Unavailable { reason, .. } => {
            (false, Some(limiting_reason_key(reason)))
        }
    }
}

fn limiting_reason_key(reason: &LimitingReason) -> String {
    match reason {
        LimitingReason::InvalidAlias => "InvalidAlias",
        LimitingReason::AliasTooWide { .. } => "AliasTooWide",
        LimitingReason::SystemNotCharted => "SystemNotCharted",
        LimitingReason::UnknownSystem => "UnknownSystem",
        LimitingReason::SystemNotCommandable => "SystemNotCommandable",
        LimitingReason::UnknownBody => "UnknownBody",
        LimitingReason::UnknownSlot => "UnknownSlot",
        LimitingReason::SlotUnavailable => "SlotUnavailable",
        LimitingReason::ExtractorTargetRequired => "ExtractorTargetRequired",
        LimitingReason::UnexpectedExtractorTarget => "UnexpectedExtractorTarget",
        LimitingReason::IncompatibleExtractorTarget => "IncompatibleExtractorTarget",
        LimitingReason::InsufficientResource { .. } => "InsufficientResource",
        LimitingReason::UnknownProject => "UnknownProject",
        LimitingReason::ProjectAlreadyBegun => "ProjectAlreadyBegun",
        LimitingReason::NotFunctionalShipyard => "NotFunctionalShipyard",
        LimitingReason::NoOperationalShipyard => "NoOperationalShipyard",
        LimitingReason::UnknownShip => "UnknownShip",
        LimitingReason::WrongShipKind => "WrongShipKind",
        LimitingReason::ShipNotReady => "ShipNotReady",
        LimitingReason::InvalidTarget => "InvalidTarget",
        LimitingReason::InvalidJumpLimit { .. } => "InvalidJumpLimit",
        LimitingReason::NoRoute => "NoRoute",
        LimitingReason::ReservationsRequired => "ReservationsRequired",
        LimitingReason::ReservationsNotAllowed => "ReservationsNotAllowed",
        LimitingReason::IncompleteReservationKnowledge => "IncompleteReservationKnowledge",
        LimitingReason::InvalidReservation => "InvalidReservation",
        LimitingReason::NoResidentPopulation => "NoResidentPopulation",
        LimitingReason::ArithmeticOverflow => "ArithmeticOverflow",
        LimitingReason::InvalidState => "InvalidState",
    }
    .to_owned()
}
