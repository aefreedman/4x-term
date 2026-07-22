//! Synchronous application/session boundary for human play.
//!
//! `StartupCoordinator` is the only owner of generated artifacts before play and
//! `Session` is the only owner of mutable [`game_core::WorldState`] after play.
//! Consumers receive immutable, player-safe DTOs and submit typed intents.

use game_content::{
    GeneratedWorldArtifact, GenerationRequest, GeneratorVersion, generate_world,
    load_generation_profile_file,
};
use game_core::{
    BodySnapshot, CompletedAsset, CoreError, FactKey, FactValue, MissionState, PlayerWorldView,
    Position3, RedactedRoute, ResourceStore, SystemSnapshot, WorldDefinition, WorldState,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use thiserror::Error;
use unicode_width::UnicodeWidthStr;

// Stable player-intent IDs and safe domain enums are re-exported so adapters do
// not need a direct `game-core` dependency.
pub use game_core::{
    Commandability, ContentId, DevelopmentCondition, DevelopmentRole, ExpeditionReservations,
    FoundingLossReason, KnowledgeLevel, ProbeReportStatus, ProjectId, ResourceRichness, ShipId,
    ShipProjectKind, SimulationTime, SlotCoordinate,
};

const ALIAS_MAX_DISPLAY_CELLS: usize = 32;

/// Machine-local profile selection. This value is startup-only and is never
/// retained by a running [`Session`] or included in [`PlayingView`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileDescriptor {
    pub machine_path: PathBuf,
    pub logical_source_id: String,
    pub display_name: String,
}

impl ProfileDescriptor {
    #[must_use]
    pub fn new(machine_path: impl Into<PathBuf>, logical_source_id: impl Into<String>) -> Self {
        let machine_path = machine_path.into();
        let display_name = machine_path
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("profile")
            .to_owned();
        Self {
            machine_path,
            logical_source_id: logical_source_id.into(),
            display_name,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreviewStatus {
    Current,
    Stale,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceAmountView {
    pub resource_id: ContentId,
    pub label: String,
    pub quantity: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewDevelopmentView {
    pub body_id: ContentId,
    pub slot_id: ContentId,
    pub role: DevelopmentRole,
    pub condition: DevelopmentCondition,
}

/// Allowlisted pre-play summary. It intentionally contains no generator
/// revision, fingerprint, provenance, exact topology, or neutral-system facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationPreviewView {
    pub status: PreviewStatus,
    pub seed: u64,
    pub profile_name: String,
    pub origin_id: ContentId,
    pub origin_label: String,
    pub origin_community_label: String,
    pub origin_body_count: usize,
    pub guaranteed_developments: Vec<PreviewDevelopmentView>,
    pub initial_origin_stocks: Vec<ResourceAmountView>,
    pub frontier_fog: Vec<MapTexturePoint>,
    pub start_available: bool,
    pub start_unavailable_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDiagnosticView {
    /// Logical source identity, never the filesystem path used to read it.
    pub logical_source_id: String,
    pub definition: String,
    pub field: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartupFailure {
    Content(Vec<SourceDiagnosticView>),
    Generation(String),
    InvalidStart(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartupView {
    pub profile: ProfileDescriptor,
    pub seed_text: String,
    pub seed_error: Option<String>,
    pub generate_available: bool,
    pub preview: Option<GenerationPreviewView>,
    pub failure: Option<StartupFailure>,
    pub start_confirmation_requested: bool,
}

struct PreparedPreview {
    artifact: GeneratedWorldArtifact,
    catalogue: Catalogue,
    view: GenerationPreviewView,
}

/// Synchronous load/compile/generate/start coordinator.
pub struct StartupCoordinator {
    profile: ProfileDescriptor,
    seed_text: String,
    prepared: Option<PreparedPreview>,
    stale: bool,
    start_requested: bool,
    failure: Option<StartupFailure>,
}

impl StartupCoordinator {
    #[must_use]
    pub fn new(profile: ProfileDescriptor, seed: u64) -> Self {
        Self {
            profile,
            seed_text: seed.to_string(),
            prepared: None,
            stale: false,
            start_requested: false,
            failure: None,
        }
    }

    pub fn edit_profile(&mut self, profile: ProfileDescriptor) {
        self.profile = profile;
        self.mark_stale();
    }

    pub fn edit_seed(&mut self, value: impl Into<String>) {
        self.seed_text = value.into();
        self.mark_stale();
    }

    fn mark_stale(&mut self) {
        self.stale = self.prepared.is_some();
        self.start_requested = false;
        self.failure = None;
    }

    #[must_use]
    pub fn view(&self) -> StartupView {
        let seed_error = self
            .seed_text
            .parse::<u64>()
            .err()
            .map(|error| error.to_string());
        let preview = self.prepared.as_ref().map(|prepared| {
            let mut view = prepared.view.clone();
            if self.stale {
                view.status = PreviewStatus::Stale;
                view.start_available = false;
                view.start_unavailable_reason =
                    Some("Profile or seed changed; generate again".into());
            }
            view
        });
        StartupView {
            profile: self.profile.clone(),
            seed_text: self.seed_text.clone(),
            seed_error: seed_error.clone(),
            generate_available: seed_error.is_none(),
            preview,
            failure: self.failure.clone(),
            start_confirmation_requested: self.start_requested,
        }
    }

    /// Loads through `game-content`, retaining logical source diagnostics while
    /// keeping the selected machine path out of generated provenance.
    pub fn generate_preview(&mut self) -> Result<GenerationPreviewView, StartupFailure> {
        let seed = self.seed_text.parse::<u64>().map_err(|error| {
            StartupFailure::InvalidStart(format!("invalid unsigned 64-bit seed: {error}"))
        })?;
        let compiled = load_generation_profile_file(
            &self.profile.logical_source_id,
            &self.profile.machine_path,
        )
        .map_err(|errors| {
            StartupFailure::Content(
                errors
                    .diagnostics()
                    .iter()
                    .map(|diagnostic| SourceDiagnosticView {
                        logical_source_id: self.profile.logical_source_id.clone(),
                        definition: diagnostic.definition.clone(),
                        field: diagnostic.field.clone(),
                        message: diagnostic.message.clone(),
                    })
                    .collect(),
            )
        });
        let compiled = match compiled {
            Ok(compiled) => compiled,
            Err(failure) => {
                self.stale = self.prepared.is_some();
                self.start_requested = false;
                self.failure = Some(failure.clone());
                return Err(failure);
            }
        };
        let request = GenerationRequest {
            version: GeneratorVersion::frontier_revision_1(),
            seed,
            configuration: compiled,
        };
        let artifact = match generate_world(&request) {
            Ok(artifact) => artifact,
            Err(error) => {
                let failure = StartupFailure::Generation(error.to_string());
                self.stale = self.prepared.is_some();
                self.start_requested = false;
                self.failure = Some(failure.clone());
                return Err(failure);
            }
        };
        let catalogue = Catalogue::from_artifact(&artifact);
        let view = preview_view(&artifact, &catalogue, &self.profile.display_name);
        self.prepared = Some(PreparedPreview {
            artifact,
            catalogue,
            view: view.clone(),
        });
        self.stale = false;
        self.start_requested = false;
        self.failure = None;
        Ok(view)
    }

    pub fn request_start_current_preview(&mut self) -> Result<(), StartupFailure> {
        if self.prepared.is_none() || self.stale {
            let failure =
                StartupFailure::InvalidStart("A current generated preview is required".into());
            self.failure = Some(failure.clone());
            return Err(failure);
        }
        self.start_requested = true;
        Ok(())
    }

    pub fn cancel_start(&mut self) {
        self.start_requested = false;
    }

    /// Consumes exactly the artifact shown by the current preview.
    pub fn confirm_start_current_preview(&mut self) -> Result<Session, StartupFailure> {
        if !self.start_requested || self.stale {
            let failure = StartupFailure::InvalidStart(
                "Start was not confirmed for a current preview".into(),
            );
            self.failure = Some(failure.clone());
            return Err(failure);
        }
        let prepared = self.prepared.take().ok_or_else(|| {
            StartupFailure::InvalidStart("No generated preview is available".into())
        })?;
        let visual_coordinates =
            map_visual_coordinates(prepared.artifact.definition(), prepared.view.seed);
        let world = WorldState::new(prepared.artifact.definition().clone()).map_err(|error| {
            StartupFailure::InvalidStart(format!("generated world could not start: {error}"))
        })?;
        self.start_requested = false;
        Ok(Session {
            world,
            seed: prepared.view.seed,
            profile_name: prepared.view.profile_name,
            catalogue: prepared.catalogue,
            visual_coordinates,
            aliases: BTreeMap::new(),
            latest_outcome: None,
        })
    }
}

#[derive(Clone)]
struct Catalogue {
    resources: BTreeMap<ContentId, String>,
    origin_id: ContentId,
    origin_label: String,
    energy_resource: ContentId,
    coordinate_quanta_per_map_unit: u64,
    habitat_population_energy: u64,
    probe_maximum_jump_limit: u64,
}

impl Catalogue {
    fn from_artifact(artifact: &GeneratedWorldArtifact) -> Self {
        let definition = artifact.definition();
        let resources = definition
            .resources
            .iter()
            .map(|resource| (resource.id.clone(), resource.name.clone()))
            .collect();
        let origin_label = definition
            .locations
            .iter()
            .find(|location| location.id == definition.origin_system)
            .map_or_else(|| "Origin".into(), |location| location.name.clone());
        Self {
            resources,
            origin_id: definition.origin_system.clone(),
            origin_label,
            energy_resource: definition.tuning.energy_resource.clone(),
            coordinate_quanta_per_map_unit: definition.tuning.coordinate_quanta_per_map_unit,
            habitat_population_energy: definition.tuning.habitat_population_energy,
            probe_maximum_jump_limit: definition.tuning.probe_travel.maximum_jump_quanta,
        }
    }

    fn resource_label(&self, id: &ContentId) -> String {
        self.resources
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.as_str().to_owned())
    }

    fn system_label(&self, id: &ContentId) -> String {
        if id == &self.origin_id {
            return self.origin_label.clone();
        }
        generated_ordinal(id)
            .map(|ordinal| format!("FSC {ordinal:06}"))
            .unwrap_or_else(|| id.as_str().to_owned())
    }
}

fn generated_ordinal(id: &ContentId) -> Option<u64> {
    id.as_str()
        .strip_prefix("generated:system_")?
        .parse::<u64>()
        .ok()
}

fn body_catalogue_label(system_label: &str, body_id: &ContentId, fallback: &str) -> String {
    let Some((_, ordinal)) = body_id.as_str().rsplit_once("_body_") else {
        return fallback.to_owned();
    };
    let Ok(ordinal) = ordinal.parse::<u8>() else {
        return fallback.to_owned();
    };
    let Some(letter) = b'b'.checked_add(ordinal).filter(|value| *value <= b'z') else {
        return fallback.to_owned();
    };
    format!("{system_label} {}", char::from(letter))
}

fn preview_view(
    artifact: &GeneratedWorldArtifact,
    catalogue: &Catalogue,
    profile_name: &str,
) -> GenerationPreviewView {
    let definition = artifact.definition();
    let origin = definition
        .systems
        .iter()
        .find(|system| system.location == definition.origin_system)
        .expect("generated artifact has its validated origin");
    let guaranteed_developments = origin
        .bodies
        .iter()
        .flat_map(|body| {
            body.slots.iter().filter_map(move |slot| {
                slot.development
                    .as_ref()
                    .map(|development| PreviewDevelopmentView {
                        body_id: body.id.clone(),
                        slot_id: slot.id.clone(),
                        role: development.role,
                        condition: development.condition,
                    })
            })
        })
        .collect();
    GenerationPreviewView {
        status: PreviewStatus::Current,
        seed: artifact.identity().seed,
        profile_name: profile_name.to_owned(),
        origin_id: definition.origin_system.clone(),
        origin_label: catalogue.origin_label.clone(),
        origin_community_label: "Origin Community".into(),
        origin_body_count: origin.bodies.len(),
        guaranteed_developments,
        initial_origin_stocks: resource_rows(&origin.stocks, catalogue),
        frontier_fog: definition
            .locations
            .iter()
            .enumerate()
            .filter_map(|(index, location)| {
                let visual_key = u64::try_from(index).ok()?;
                Some(MapTexturePoint {
                    coordinate: map_visual_coordinate(
                        location.position,
                        definition.tuning.coordinate_quanta_per_map_unit,
                        artifact.identity().seed,
                        visual_key,
                    ),
                    visual_key,
                })
            })
            .collect(),
        start_available: true,
        start_unavailable_reason: None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChartCoordinate {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

fn map_visual_coordinates(
    definition: &WorldDefinition,
    seed: u64,
) -> BTreeMap<u64, ChartCoordinate> {
    definition
        .locations
        .iter()
        .enumerate()
        .filter_map(|(index, location)| {
            let visual_key = u64::try_from(index).ok()?;
            Some((
                visual_key,
                map_visual_coordinate(
                    location.position,
                    definition.tuning.coordinate_quanta_per_map_unit,
                    seed,
                    visual_key,
                ),
            ))
        })
        .collect()
}

fn map_visual_coordinate(
    position: Position3,
    coordinate_quanta_per_map_unit: u64,
    seed: u64,
    visual_key: u64,
) -> ChartCoordinate {
    let divisor = i64::try_from(coordinate_quanta_per_map_unit)
        .unwrap_or(i64::MAX)
        .max(1);
    let (offset_x, offset_y) = map_visual_pivot_offset(seed, visual_key);
    ChartCoordinate {
        x: position.x.0.div_euclid(divisor) + offset_x,
        y: position.y.0.div_euclid(divisor) + offset_y,
        z: position.z.0.div_euclid(divisor),
    }
}

fn map_visual_pivot_offset(seed: u64, visual_key: u64) -> (i64, i64) {
    let hash = format!("{seed}:{visual_key}")
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
        });
    let mut selected = (hash >> 16) % 49;
    for y in -4_i64..=4 {
        for x in -4_i64..=4 {
            if x * x + y * y > 16 {
                continue;
            }
            if selected == 0 {
                return (x, y);
            }
            selected -= 1;
        }
    }
    (0, 0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MapTexturePoint {
    pub coordinate: ChartCoordinate,
    pub visual_key: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemListEntry {
    pub system_id: ContentId,
    pub visual_key: u64,
    pub visual_coordinate: ChartCoordinate,
    pub catalogue_label: String,
    pub alias: Option<String>,
    pub display_label: String,
    pub knowledge: KnowledgeLevel,
    pub chart_position: Option<ChartCoordinate>,
    pub commandability: Option<Commandability>,
    pub last_observed_tick: Option<u64>,
    pub last_received_tick: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChartEntry {
    pub system_id: ContentId,
    pub display_label: String,
    pub coordinate: ChartCoordinate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KnownFactView {
    BodyCount(u64),
    StellarStrengthHundredths(u64),
    ResourceRichness {
        resource_id: ContentId,
        resource_label: String,
        richness: ResourceRichness,
    },
    Inhabited(bool),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemDetailView {
    pub system_id: ContentId,
    pub catalogue_label: String,
    pub alias: Option<String>,
    pub display_label: String,
    pub knowledge: KnowledgeLevel,
    pub chart_position: Option<ChartCoordinate>,
    pub facts: Vec<KnownFactView>,
    pub commandability: Option<Commandability>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnergyTickEvidenceView {
    pub required_life_support: u64,
    pub paid_life_support: u64,
    pub unpaid_life_support: u64,
    pub supported_population: u64,
    pub underserved_population: u64,
    pub retention_overflow: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnergyView {
    pub resource_id: ContentId,
    pub label: String,
    pub current: u64,
    pub capacity: u64,
    pub headroom: u64,
    /// One-based position in the ten-phase seasonal cycle.
    pub seasonal_position: u8,
    pub last_completed_tick: Option<EnergyTickEvidenceView>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentView {
    pub development_id: ContentId,
    pub role: DevelopmentRole,
    pub condition: DevelopmentCondition,
    pub enabled: bool,
    pub toggle: ActionAvailability,
    pub extractor_target: Option<ResourceAmountView>,
    pub production_progress: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HabitatView {
    pub development_id: ContentId,
    pub body_id: ContentId,
    pub slot_id: ContentId,
    pub functional: bool,
    pub occupied: bool,
    pub generation_enabled: bool,
    pub generation_progress: u64,
    pub required_energy: u64,
    pub ready_since_tick: Option<u64>,
    pub toggle: ActionAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShipyardProjectView {
    pub project_id: ProjectId,
    pub ship_id: ShipId,
    pub kind: ShipProjectKind,
    pub commitment: Vec<ResourceAmountView>,
    pub progress: u64,
    pub required_progress: u64,
    pub energy_per_progress_tick: u64,
    pub cancellable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlotView {
    pub slot_id: ContentId,
    pub slot_label: String,
    pub reserved: bool,
    pub development: Option<DevelopmentView>,
    pub habitat: Option<HabitatView>,
    pub shipyard_queue: Vec<ShipyardProjectView>,
    pub construction_options: Vec<ConstructionOptionView>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyView {
    pub body_id: ContentId,
    pub label: String,
    pub eccentricity_hundredths: u16,
    pub resources: Vec<ResourceAmountView>,
    pub slots: Vec<SlotView>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructionQueueView {
    pub project_id: ProjectId,
    pub body_id: ContentId,
    pub slot_id: ContentId,
    pub role: DevelopmentRole,
    pub extractor_resource: Option<ResourceAmountView>,
    pub cost: Vec<ResourceAmountView>,
    pub work_applied: u64,
    pub required_work: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetKindView {
    Probe,
    Expedition {
        founding_stocks: Vec<ResourceAmountView>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedAssetView {
    pub ship_id: ShipId,
    pub kind: AssetKindView,
    pub ready: bool,
    pub available_at_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSystemView {
    pub system_id: ContentId,
    pub has_operational_shipyard: bool,
    pub stocks: Vec<ResourceAmountView>,
    pub energy: EnergyView,
    pub population_count: u64,
    pub occupied_habitat_slots: Vec<SlotCoordinate>,
    pub bodies: Vec<BodyView>,
    pub construction_queue: Vec<ConstructionQueueView>,
    pub completed_assets: Vec<CompletedAssetView>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionAvailability {
    Available,
    Unavailable {
        reason: LimitingReason,
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructionOptionView {
    pub role: DevelopmentRole,
    pub extractor_resource_id: Option<ContentId>,
    pub extractor_resource_label: Option<String>,
    pub cost: Vec<ResourceAmountView>,
    pub required_work: u64,
    pub availability: ActionAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteStopView {
    pub system_id: Option<ContentId>,
    pub label: Option<String>,
    pub reached: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteView {
    pub ship_id: Option<ShipId>,
    pub stops: Vec<RouteStopView>,
    pub total_distance: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MissionView {
    AwaitingOutcome {
        ship_id: ShipId,
        target_id: ContentId,
        target_label: String,
    },
    Founded {
        ship_id: ShipId,
        target_id: ContentId,
        target_label: String,
        community_id: ContentId,
    },
    FoundingLost {
        ship_id: ShipId,
        target_id: ContentId,
        target_label: String,
        reason: FoundingLossReason,
        lost_stocks: Vec<ResourceAmountView>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeReportView {
    pub ship_id: ShipId,
    pub awaiting_report: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayingView {
    pub seed: u64,
    pub profile_name: String,
    pub time: SimulationTime,
    pub seasonal_position: u8,
    pub probe_maximum_jump_limit: u64,
    pub systems: Vec<SystemListEntry>,
    pub chart: Vec<ChartEntry>,
    pub unpositioned_systems: Vec<ContentId>,
    pub uncharted_indication_count: usize,
    pub frontier_fog: Vec<MapTexturePoint>,
    pub details: Vec<SystemDetailView>,
    pub local_systems: Vec<LocalSystemView>,
    pub missions: Vec<MissionView>,
    pub probe_reports: Vec<ProbeReportView>,
    pub active_routes: Vec<RouteView>,
    pub active_ship_positions: Vec<ChartCoordinate>,
    pub latest_outcome: Option<ApplicationOutcome>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftDisposition {
    Retain,
    InvalidateRoot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentKind {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LimitingReason {
    InvalidAlias,
    AliasTooWide {
        cells: usize,
        maximum: usize,
    },
    SystemNotCharted,
    UnknownSystem,
    SystemNotCommandable,
    UnknownBody,
    UnknownSlot,
    SlotUnavailable,
    ExtractorTargetRequired,
    UnexpectedExtractorTarget,
    IncompatibleExtractorTarget,
    InsufficientResource {
        resource_id: ContentId,
        resource_label: String,
        available: u64,
        required: u64,
    },
    UnknownProject,
    ProjectAlreadyBegun,
    NotFunctionalShipyard,
    NoOperationalShipyard,
    UnknownShip,
    WrongShipKind,
    ShipNotReady,
    InvalidTarget,
    InvalidJumpLimit {
        requested: u64,
        maximum: u64,
    },
    NoRoute,
    ReservationsRequired,
    ReservationsNotAllowed,
    IncompleteReservationKnowledge,
    InvalidReservation,
    NoResidentPopulation,
    ArithmeticOverflow,
    InvalidState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationOutcome {
    pub accepted: bool,
    pub intent: IntentKind,
    pub message: String,
    pub limiting_reason: Option<LimitingReason>,
    pub draft_disposition: Option<DraftDisposition>,
    pub project_id: Option<ProjectId>,
    pub ship_id: Option<ShipId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TickDeltaView {
    pub from_tick: u64,
    pub to_tick: u64,
    pub stock_changes: Vec<StockChangeView>,
    pub population_changes: Vec<PopulationChangeView>,
    pub newly_identified_systems: Vec<ContentId>,
    pub mission_changes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StockChangeView {
    pub system_id: ContentId,
    pub resource_id: ContentId,
    pub label: String,
    pub before: u64,
    pub after: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopulationChangeView {
    pub system_id: ContentId,
    pub before: u64,
    pub after: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TickStepView {
    pub view: PlayingView,
    pub delta: TickDeltaView,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeAssessmentView {
    pub source_id: ContentId,
    pub ship_id: ShipId,
    pub target_id: ContentId,
    pub target_label: String,
    pub requested_jump_limit: u64,
    pub minimum_jump_limit: u64,
    pub maximum_jump_limit: u64,
    pub target_knowledge: KnowledgeLevel,
    pub asset_ready: bool,
    pub travel_energy: Option<u64>,
    pub route: Option<RouteView>,
    pub availability: ActionAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpeditionAssessmentView {
    pub source_id: ContentId,
    pub ship_id: ShipId,
    pub target_id: ContentId,
    pub target_label: String,
    pub reservations: Option<ExpeditionReservations>,
    pub reservation_choices: Vec<SlotCoordinate>,
    pub complete_commitment: Vec<ResourceAmountView>,
    pub resident_population_required: u64,
    pub resident_population_available: u64,
    pub resident_population_ready: bool,
    pub target_knowledge: KnowledgeLevel,
    pub asset_ready: bool,
    pub travel_energy: Option<u64>,
    pub route: Option<RouteView>,
    pub availability: ActionAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionIntent {
    EnqueueConstruction {
        system_id: ContentId,
        body_id: ContentId,
        slot_id: ContentId,
        role: DevelopmentRole,
        extractor_resource_id: Option<ContentId>,
    },
    SetDevelopmentOperationalEnabled {
        system_id: ContentId,
        body_id: ContentId,
        slot_id: ContentId,
        enabled: bool,
    },
    SetHabitatGenerationEnabled {
        system_id: ContentId,
        body_id: ContentId,
        slot_id: ContentId,
        enabled: bool,
    },
    SetSystemAlias {
        system_id: ContentId,
        alias: Option<String>,
    },
    AdvanceOneTick,
    EnqueueShipProject {
        system_id: ContentId,
        shipyard_body_id: ContentId,
        shipyard_slot_id: ContentId,
        kind: ShipProjectKind,
    },
    CancelShipProject {
        project_id: ProjectId,
    },
    AssessProbeLaunch {
        source_id: ContentId,
        ship_id: ShipId,
        target_id: ContentId,
        jump_limit: u64,
    },
    LaunchProbe {
        source_id: ContentId,
        ship_id: ShipId,
        target_id: ContentId,
        jump_limit: u64,
    },
    AssessExpeditionLaunch {
        source_id: ContentId,
        ship_id: ShipId,
        target_id: ContentId,
        reservations: Option<ExpeditionReservations>,
    },
    LaunchExpedition {
        source_id: ContentId,
        ship_id: ShipId,
        target_id: ContentId,
        reservations: Option<ExpeditionReservations>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionOutcome {
    Applied {
        outcome: ApplicationOutcome,
        view: PlayingView,
    },
    Tick(TickStepView),
    ProbeAssessment(ProbeAssessmentView),
    ExpeditionAssessment(ExpeditionAssessmentView),
    ProbeLaunched {
        outcome: ApplicationOutcome,
        route: RouteView,
        view: PlayingView,
    },
    ExpeditionLaunched {
        outcome: ApplicationOutcome,
        route: RouteView,
        view: PlayingView,
    },
    Rejected(ApplicationOutcome),
}

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("player projection failed: {0}")]
    Projection(String),
}

/// Sole mutable simulation owner for one unsaved play session.
pub struct Session {
    world: WorldState,
    seed: u64,
    profile_name: String,
    catalogue: Catalogue,
    visual_coordinates: BTreeMap<u64, ChartCoordinate>,
    aliases: BTreeMap<ContentId, String>,
    latest_outcome: Option<ApplicationOutcome>,
}

impl Session {
    pub fn playing_view(&self) -> Result<PlayingView, ApplicationError> {
        let core = self
            .world
            .player_view()
            .map_err(|error| ApplicationError::Projection(error.to_string()))?;
        Ok(self.project(&core))
    }

    /// Dispatches exactly one application intent. Multi-tick remains repeated
    /// `AdvanceOneTick` dispatches by the caller.
    pub fn dispatch(&mut self, intent: SessionIntent) -> Result<SessionOutcome, ApplicationError> {
        match intent {
            SessionIntent::AdvanceOneTick => self.advance_one_tick(),
            SessionIntent::SetSystemAlias { system_id, alias } => self.set_alias(system_id, alias),
            SessionIntent::EnqueueConstruction {
                system_id,
                body_id,
                slot_id,
                role,
                extractor_resource_id,
            } => {
                let result = self.world.enqueue_construction(
                    &system_id,
                    &body_id,
                    &slot_id,
                    role,
                    extractor_resource_id.as_ref(),
                );
                match result {
                    Ok(project_id) => self.applied(
                        IntentKind::Construction,
                        "Construction queued",
                        Some(project_id),
                        None,
                    ),
                    Err(error) => Ok(self.rejected(
                        IntentKind::Construction,
                        error,
                        Some(construction_disposition),
                    )),
                }
            }
            SessionIntent::SetDevelopmentOperationalEnabled {
                system_id,
                body_id,
                slot_id,
                enabled,
            } => {
                match self
                    .world
                    .set_development_operational_enabled(&system_id, &body_id, &slot_id, enabled)
                {
                    Ok(()) => self.applied(
                        IntentKind::DevelopmentOperation,
                        if enabled {
                            "Development enabled"
                        } else {
                            "Development disabled"
                        },
                        None,
                        None,
                    ),
                    Err(error) => Ok(self.rejected(
                        IntentKind::DevelopmentOperation,
                        error,
                        Some(|_| DraftDisposition::Retain),
                    )),
                }
            }
            SessionIntent::SetHabitatGenerationEnabled {
                system_id,
                body_id,
                slot_id,
                enabled,
            } => {
                match self
                    .world
                    .set_habitat_generation_enabled(&system_id, &body_id, &slot_id, enabled)
                {
                    Ok(()) => self.applied(
                        IntentKind::Habitat,
                        if enabled {
                            "Habitat generation enabled"
                        } else {
                            "Habitat generation disabled"
                        },
                        None,
                        None,
                    ),
                    Err(error) => Ok(self.rejected(
                        IntentKind::Habitat,
                        error,
                        Some(|_| DraftDisposition::Retain),
                    )),
                }
            }
            SessionIntent::EnqueueShipProject {
                system_id,
                shipyard_body_id,
                shipyard_slot_id,
                kind,
            } => {
                let intent_kind = match kind {
                    ShipProjectKind::Probe => IntentKind::EnqueueProbe,
                    ShipProjectKind::Expedition => IntentKind::EnqueueExpedition,
                };
                match self.world.enqueue_ship_project(
                    &system_id,
                    &shipyard_body_id,
                    &shipyard_slot_id,
                    kind,
                ) {
                    Ok(ids) => self.applied(
                        intent_kind,
                        "Shipyard project queued",
                        Some(ids.project_id),
                        Some(ids.ship_id),
                    ),
                    Err(error) => {
                        Ok(self.rejected(intent_kind, error, Some(|_| DraftDisposition::Retain)))
                    }
                }
            }
            SessionIntent::CancelShipProject { project_id } => {
                match self.world.cancel_ship_project(&project_id) {
                    Ok(()) => self.applied(
                        IntentKind::CancelShipProject,
                        "Shipyard project cancelled",
                        Some(project_id),
                        None,
                    ),
                    Err(error) => Ok(self.rejected(IntentKind::CancelShipProject, error, None)),
                }
            }
            SessionIntent::AssessProbeLaunch {
                source_id,
                ship_id,
                target_id,
                jump_limit,
            } => {
                let assessment = self
                    .world
                    .assess_probe_launch(&source_id, &ship_id, &target_id, jump_limit);
                Ok(SessionOutcome::ProbeAssessment(
                    self.probe_assessment(assessment),
                ))
            }
            SessionIntent::LaunchProbe {
                source_id,
                ship_id,
                target_id,
                jump_limit,
            } => {
                match self
                    .world
                    .launch_probe(&source_id, &ship_id, &target_id, jump_limit)
                {
                    Ok(route) => {
                        let outcome = accepted_outcome(
                            IntentKind::LaunchProbe,
                            "Probe launched",
                            None,
                            Some(ship_id.clone()),
                        );
                        self.latest_outcome = Some(outcome.clone());
                        let view = self.playing_view()?;
                        Ok(SessionOutcome::ProbeLaunched {
                            outcome,
                            route: self.route_view(Some(ship_id), &route),
                            view,
                        })
                    }
                    Err(error) => Ok(self.rejected(
                        IntentKind::LaunchProbe,
                        error,
                        Some(expedition_disposition),
                    )),
                }
            }
            SessionIntent::AssessExpeditionLaunch {
                source_id,
                ship_id,
                target_id,
                reservations,
            } => {
                let assessment = self.world.assess_expedition_launch(
                    &source_id,
                    &ship_id,
                    &target_id,
                    reservations,
                );
                let core = self
                    .world
                    .player_view()
                    .map_err(|error| ApplicationError::Projection(error.to_string()))?;
                Ok(SessionOutcome::ExpeditionAssessment(
                    self.expedition_assessment(assessment, &core),
                ))
            }
            SessionIntent::LaunchExpedition {
                source_id,
                ship_id,
                target_id,
                reservations,
            } => {
                match self
                    .world
                    .launch_expedition(&source_id, &ship_id, &target_id, reservations)
                {
                    Ok(route) => {
                        let outcome = accepted_outcome(
                            IntentKind::LaunchExpedition,
                            "Expedition launched",
                            None,
                            Some(ship_id.clone()),
                        );
                        self.latest_outcome = Some(outcome.clone());
                        let view = self.playing_view()?;
                        Ok(SessionOutcome::ExpeditionLaunched {
                            outcome,
                            route: self.route_view(Some(ship_id), &route),
                            view,
                        })
                    }
                    Err(error) => Ok(self.rejected(
                        IntentKind::LaunchExpedition,
                        error,
                        Some(expedition_disposition),
                    )),
                }
            }
        }
    }

    fn applied(
        &mut self,
        intent: IntentKind,
        message: &str,
        project_id: Option<ProjectId>,
        ship_id: Option<ShipId>,
    ) -> Result<SessionOutcome, ApplicationError> {
        let outcome = accepted_outcome(intent, message, project_id, ship_id);
        self.latest_outcome = Some(outcome.clone());
        Ok(SessionOutcome::Applied {
            outcome,
            view: self.playing_view()?,
        })
    }

    fn rejected(
        &mut self,
        intent: IntentKind,
        error: CoreError,
        disposition: Option<fn(&CoreError) -> DraftDisposition>,
    ) -> SessionOutcome {
        let reason = limiting_reason(&error, &self.catalogue);
        let outcome = ApplicationOutcome {
            accepted: false,
            intent,
            message: player_error_message(&reason),
            limiting_reason: Some(reason),
            draft_disposition: disposition.map(|classify| classify(&error)),
            project_id: None,
            ship_id: None,
        };
        self.latest_outcome = Some(outcome.clone());
        SessionOutcome::Rejected(outcome)
    }

    fn advance_one_tick(&mut self) -> Result<SessionOutcome, ApplicationError> {
        let before = self.playing_view()?;
        match self.world.advance_tick() {
            Ok(core_after) => {
                let outcome =
                    accepted_outcome(IntentKind::AdvanceOneTick, "Advanced one tick", None, None);
                self.latest_outcome = Some(outcome);
                let after = self.project(&core_after);
                let delta = tick_delta(&before, &after);
                Ok(SessionOutcome::Tick(TickStepView { view: after, delta }))
            }
            Err(error) => Ok(self.rejected(IntentKind::AdvanceOneTick, error, None)),
        }
    }

    fn set_alias(
        &mut self,
        system_id: ContentId,
        alias: Option<String>,
    ) -> Result<SessionOutcome, ApplicationError> {
        let core = self
            .world
            .player_view()
            .map_err(|error| ApplicationError::Projection(error.to_string()))?;
        let charted = core
            .systems
            .iter()
            .find(|system| system.system == system_id)
            .is_some_and(|system| {
                system.system == self.catalogue.origin_id
                    || known_position(&system.knowledge).is_some()
            });
        if !charted {
            let outcome = ApplicationOutcome {
                accepted: false,
                intent: IntentKind::Alias,
                message: "Only a charted system can be renamed".into(),
                limiting_reason: Some(LimitingReason::SystemNotCharted),
                draft_disposition: Some(DraftDisposition::Retain),
                project_id: None,
                ship_id: None,
            };
            self.latest_outcome = Some(outcome.clone());
            return Ok(SessionOutcome::Rejected(outcome));
        }
        match alias {
            None => {
                self.aliases.remove(&system_id);
            }
            Some(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty()
                    || trimmed.lines().count() != 1
                    || trimmed.contains(['\r', '\n'])
                {
                    let outcome = alias_rejection(
                        LimitingReason::InvalidAlias,
                        "Alias must be a non-empty single line",
                    );
                    self.latest_outcome = Some(outcome.clone());
                    return Ok(SessionOutcome::Rejected(outcome));
                }
                let cells = UnicodeWidthStr::width(trimmed);
                if cells > ALIAS_MAX_DISPLAY_CELLS {
                    let outcome = alias_rejection(
                        LimitingReason::AliasTooWide {
                            cells,
                            maximum: ALIAS_MAX_DISPLAY_CELLS,
                        },
                        "Alias is wider than 32 display cells",
                    );
                    self.latest_outcome = Some(outcome.clone());
                    return Ok(SessionOutcome::Rejected(outcome));
                }
                self.aliases.insert(system_id, trimmed.to_owned());
            }
        }
        self.applied(IntentKind::Alias, "System alias updated", None, None)
    }

    fn project(&self, core: &PlayerWorldView) -> PlayingView {
        let mut systems = Vec::new();
        let mut chart = Vec::new();
        let mut unpositioned_systems = Vec::new();
        let mut details = Vec::new();
        let mut local_systems = Vec::new();
        for system in &core.systems {
            let catalogue_label = self.catalogue.system_label(&system.system);
            let alias = self.aliases.get(&system.system).cloned();
            let display_label = alias.clone().unwrap_or_else(|| catalogue_label.clone());
            let position = if system.system == self.catalogue.origin_id {
                Some(ChartCoordinate { x: 0, y: 0, z: 0 })
            } else {
                known_position(&system.knowledge).map(|position| self.chart_coordinate(position))
            };
            let observed = system
                .knowledge
                .facts
                .values()
                .map(|fact| fact.tick_observed)
                .max();
            let received = system
                .knowledge
                .facts
                .values()
                .map(|fact| fact.tick_received)
                .max();
            let commandability = system
                .local_state
                .as_ref()
                .map(|local| local.commandability);
            systems.push(SystemListEntry {
                system_id: system.system.clone(),
                visual_key: system.map_visual_key,
                visual_coordinate: *self
                    .visual_coordinates
                    .get(&system.map_visual_key)
                    .expect("every projected system retains its generated map visual"),
                catalogue_label: catalogue_label.clone(),
                alias: alias.clone(),
                display_label: display_label.clone(),
                knowledge: system.knowledge.level,
                chart_position: position,
                commandability,
                last_observed_tick: observed,
                last_received_tick: received,
            });
            if let Some(coordinate) = position {
                chart.push(ChartEntry {
                    system_id: system.system.clone(),
                    display_label: display_label.clone(),
                    coordinate,
                });
            } else {
                unpositioned_systems.push(system.system.clone());
            }
            details.push(SystemDetailView {
                system_id: system.system.clone(),
                catalogue_label,
                alias,
                display_label,
                knowledge: system.knowledge.level,
                chart_position: position,
                facts: self.fact_views(&system.knowledge),
                commandability,
            });
            if let Some(local) = &system.local_state {
                local_systems.push(self.local_view(local, core));
            }
        }
        systems.sort_by(|left, right| left.system_id.cmp(&right.system_id));
        chart.sort_by(|left, right| left.system_id.cmp(&right.system_id));
        details.sort_by(|left, right| left.system_id.cmp(&right.system_id));
        local_systems.sort_by(|left, right| left.system_id.cmp(&right.system_id));
        let missions = core
            .missions
            .iter()
            .map(|(ship_id, mission)| self.mission_view(ship_id, mission))
            .collect();
        let probe_reports = core
            .probe_reports
            .keys()
            .map(|ship_id| ProbeReportView {
                ship_id: ship_id.clone(),
                awaiting_report: true,
            })
            .collect();
        let active_routes = core
            .active_routes
            .iter()
            .map(|(ship_id, route)| self.route_view(Some(ship_id.clone()), route))
            .collect();
        PlayingView {
            seed: self.seed,
            profile_name: self.profile_name.clone(),
            time: core.time,
            seasonal_position: core.seasonal_phase + 1,
            probe_maximum_jump_limit: self.catalogue.probe_maximum_jump_limit,
            systems,
            chart,
            unpositioned_systems,
            uncharted_indication_count: core.anonymous_indication_count,
            frontier_fog: core
                .frontier_fog
                .iter()
                .map(|point| MapTexturePoint {
                    coordinate: *self
                        .visual_coordinates
                        .get(&point.map_visual_key)
                        .expect("every fog point retains its generated map visual"),
                    visual_key: point.map_visual_key,
                })
                .collect(),
            details,
            local_systems,
            missions,
            probe_reports,
            active_routes,
            active_ship_positions: core
                .active_ship_positions
                .iter()
                .map(|position| self.chart_coordinate(*position))
                .collect(),
            latest_outcome: self.latest_outcome.clone(),
        }
    }

    fn chart_coordinate(&self, position: Position3) -> ChartCoordinate {
        let divisor = i64::try_from(self.catalogue.coordinate_quanta_per_map_unit)
            .unwrap_or(i64::MAX)
            .max(1);
        ChartCoordinate {
            x: position.x.0.div_euclid(divisor),
            y: position.y.0.div_euclid(divisor),
            z: position.z.0.div_euclid(divisor),
        }
    }

    fn fact_views(&self, knowledge: &game_core::SystemKnowledge) -> Vec<KnownFactView> {
        knowledge
            .facts
            .iter()
            .filter_map(|(key, fact)| match (key, &fact.value) {
                (FactKey::BodyCount, FactValue::Unsigned(value)) => {
                    Some(KnownFactView::BodyCount(*value))
                }
                (FactKey::SystemStrength, FactValue::Unsigned(value)) => {
                    Some(KnownFactView::StellarStrengthHundredths(*value))
                }
                (FactKey::ResourceRichness { resource }, FactValue::Richness(richness)) => {
                    Some(KnownFactView::ResourceRichness {
                        resource_id: resource.clone(),
                        resource_label: self.catalogue.resource_label(resource),
                        richness: *richness,
                    })
                }
                (FactKey::Inhabited, FactValue::Boolean(value)) => {
                    Some(KnownFactView::Inhabited(*value))
                }
                _ => None,
            })
            .collect()
    }

    fn local_view(&self, snapshot: &SystemSnapshot, core: &PlayerWorldView) -> LocalSystemView {
        let occupied = &snapshot.local_population.occupied_habitat_slots;
        let bodies = snapshot
            .bodies
            .iter()
            .map(|body| self.body_view(snapshot, body, occupied))
            .collect();
        let construction_queue = snapshot
            .construction_queue
            .iter()
            .map(|item| ConstructionQueueView {
                project_id: item.id.clone(),
                body_id: item.body.clone(),
                slot_id: item.slot.clone(),
                role: item.role,
                extractor_resource: item.extractor_target.as_ref().map(|target| {
                    ResourceAmountView {
                        resource_id: target.resource.clone(),
                        label: self.catalogue.resource_label(&target.resource),
                        quantity: body_remaining(snapshot, &target.body, &target.resource),
                    }
                }),
                cost: resource_rows(&item.committed_resources, &self.catalogue),
                work_applied: item.work_applied,
                required_work: item.required_work,
            })
            .collect();
        let completed_assets = snapshot
            .completed_assets
            .iter()
            .map(|asset| self.asset_view(asset, core.time.tick))
            .collect();
        LocalSystemView {
            system_id: snapshot.location.clone(),
            has_operational_shipyard: snapshot.bodies.iter().any(|body| {
                body.slots.iter().any(|slot| {
                    slot.development.as_ref().is_some_and(|development| {
                        development.definition.role == DevelopmentRole::Shipyard
                            && development.definition.condition == DevelopmentCondition::Functional
                            && development.enabled
                    })
                })
            }),
            stocks: resource_rows(&snapshot.stocks, &self.catalogue),
            energy: EnergyView {
                resource_id: self.catalogue.energy_resource.clone(),
                label: self
                    .catalogue
                    .resource_label(&self.catalogue.energy_resource),
                current: snapshot.stocks.quantity(&self.catalogue.energy_resource),
                capacity: snapshot.energy_capacity,
                headroom: snapshot.energy_headroom,
                seasonal_position: core.seasonal_phase + 1,
                last_completed_tick: (core.time.tick > 0).then_some(EnergyTickEvidenceView {
                    required_life_support: snapshot.life_support.required_energy,
                    paid_life_support: snapshot.life_support.paid_energy,
                    unpaid_life_support: snapshot.life_support.unpaid_energy,
                    supported_population: snapshot.life_support.supported_population,
                    underserved_population: snapshot.life_support.underserved_population,
                    retention_overflow: snapshot.energy_overflow.last_tick_retention,
                }),
            },
            population_count: snapshot.local_population.population_count,
            occupied_habitat_slots: snapshot.local_population.occupied_habitat_slots.clone(),
            bodies,
            construction_queue,
            completed_assets,
        }
    }

    fn body_view(
        &self,
        snapshot: &SystemSnapshot,
        body: &BodySnapshot,
        occupied: &[SlotCoordinate],
    ) -> BodyView {
        let slots = body
            .slots
            .iter()
            .map(|slot| {
                let coordinate = SlotCoordinate {
                    body: body.id.clone(),
                    slot: slot.id.clone(),
                };
                let development = slot.development.as_ref().map(|development| {
                    let assessment = self.world.assess_development_operational_toggle(
                        &snapshot.location,
                        &body.id,
                        &slot.id,
                        !development.enabled,
                    );
                    DevelopmentView {
                        development_id: development.definition.id.clone(),
                        role: development.definition.role,
                        condition: development.definition.condition,
                        enabled: development.enabled,
                        toggle: availability(assessment.limiting_reason.as_ref(), &self.catalogue),
                        extractor_target: development.definition.extractor_target.as_ref().map(
                            |target| ResourceAmountView {
                                resource_id: target.resource.clone(),
                                label: self.catalogue.resource_label(&target.resource),
                                quantity: body.remaining_resources.quantity(&target.resource),
                            },
                        ),
                        production_progress: development.cycle.progress,
                    }
                });
                let habitat = slot.development.as_ref().and_then(|development| {
                    development.habitat.as_ref().map(|habitat| {
                        let next_enabled = !habitat.generation_enabled;
                        let assessment = self.world.assess_habitat_generation_toggle(
                            &snapshot.location,
                            &body.id,
                            &slot.id,
                            next_enabled,
                        );
                        HabitatView {
                            development_id: development.definition.id.clone(),
                            body_id: body.id.clone(),
                            slot_id: slot.id.clone(),
                            functional: development.definition.condition
                                == DevelopmentCondition::Functional,
                            occupied: occupied.contains(&coordinate),
                            generation_enabled: habitat.generation_enabled,
                            generation_progress: habitat.generation_progress,
                            required_energy: self.catalogue.habitat_population_energy,
                            ready_since_tick: habitat.ready_since_tick,
                            toggle: availability(
                                assessment.limiting_reason.as_ref(),
                                &self.catalogue,
                            ),
                        }
                    })
                });
                let shipyard_queue = slot
                    .development
                    .as_ref()
                    .and_then(|development| development.shipyard.as_ref())
                    .map_or_else(Vec::new, |shipyard| {
                        shipyard
                            .queue
                            .iter()
                            .map(|project| ShipyardProjectView {
                                project_id: project.id.clone(),
                                ship_id: project.ship_id.clone(),
                                kind: project.kind,
                                commitment: resource_rows(
                                    &project.committed_resources,
                                    &self.catalogue,
                                ),
                                progress: project.progress,
                                required_progress: project.required_progress,
                                energy_per_progress_tick: project.energy_per_progress_tick,
                                cancellable: project.progress == 0,
                            })
                            .collect()
                    });
                let construction_options =
                    if slot.development.is_none() && slot.reserved_by.is_none() {
                        self.construction_options(snapshot, body, &slot.id)
                    } else {
                        Vec::new()
                    };
                SlotView {
                    slot_id: slot.id.clone(),
                    slot_label: slot
                        .id
                        .as_str()
                        .rsplit('_')
                        .next()
                        .map_or_else(|| "Slot".into(), |value| format!("Slot {value}")),
                    reserved: slot.reserved_by.is_some(),
                    development,
                    habitat,
                    shipyard_queue,
                    construction_options,
                }
            })
            .collect();
        BodyView {
            body_id: body.id.clone(),
            label: body_catalogue_label(
                &self.catalogue.system_label(&snapshot.location),
                &body.id,
                &body.name,
            ),
            eccentricity_hundredths: body.eccentricity_hundredths,
            resources: resource_rows(&body.remaining_resources, &self.catalogue),
            slots,
        }
    }

    fn construction_options(
        &self,
        snapshot: &SystemSnapshot,
        body: &BodySnapshot,
        slot: &ContentId,
    ) -> Vec<ConstructionOptionView> {
        let mut options = Vec::new();
        for role in [
            DevelopmentRole::Collector,
            DevelopmentRole::Battery,
            DevelopmentRole::Extractor,
            DevelopmentRole::Refinery,
            DevelopmentRole::Habitat,
            DevelopmentRole::Shipyard,
        ] {
            if role == DevelopmentRole::Extractor {
                let targets = body
                    .initial_resources
                    .quantities
                    .iter()
                    .filter(|(_, quantity)| **quantity > 0)
                    .map(|(resource, _)| resource.clone())
                    .collect::<Vec<_>>();
                if targets.is_empty() {
                    let assessment = self.world.assess_construction(
                        &snapshot.location,
                        &body.id,
                        slot,
                        role,
                        None,
                    );
                    options.push(construction_option(assessment, &self.catalogue));
                } else {
                    for resource in targets {
                        let assessment = self.world.assess_construction(
                            &snapshot.location,
                            &body.id,
                            slot,
                            role,
                            Some(&resource),
                        );
                        options.push(construction_option(assessment, &self.catalogue));
                    }
                }
            } else {
                options.push(construction_option(
                    self.world
                        .assess_construction(&snapshot.location, &body.id, slot, role, None),
                    &self.catalogue,
                ));
            }
        }
        options
    }

    fn asset_view(&self, asset: &CompletedAsset, tick: u64) -> CompletedAssetView {
        match asset {
            CompletedAsset::Probe {
                ship_id,
                available_at_tick,
            } => CompletedAssetView {
                ship_id: ship_id.clone(),
                kind: AssetKindView::Probe,
                ready: tick >= *available_at_tick,
                available_at_tick: *available_at_tick,
            },
            CompletedAsset::Expedition {
                ship_id,
                payload,
                available_at_tick,
            } => CompletedAssetView {
                ship_id: ship_id.clone(),
                kind: AssetKindView::Expedition {
                    founding_stocks: resource_rows(&payload.founding_stocks, &self.catalogue),
                },
                ready: tick >= *available_at_tick,
                available_at_tick: *available_at_tick,
            },
        }
    }

    fn mission_view(&self, ship_id: &ShipId, mission: &MissionState) -> MissionView {
        match mission {
            MissionState::AwaitingOutcome { target } => MissionView::AwaitingOutcome {
                ship_id: ship_id.clone(),
                target_id: target.clone(),
                target_label: self.catalogue.system_label(target),
            },
            MissionState::Founded {
                target,
                community_id,
                ..
            } => MissionView::Founded {
                ship_id: ship_id.clone(),
                target_id: target.clone(),
                target_label: self.catalogue.system_label(target),
                community_id: community_id.clone(),
            },
            MissionState::FoundingLost {
                target,
                founding_stocks,
                reason,
                ..
            } => MissionView::FoundingLost {
                ship_id: ship_id.clone(),
                target_id: target.clone(),
                target_label: self.catalogue.system_label(target),
                reason: *reason,
                lost_stocks: resource_rows(founding_stocks, &self.catalogue),
            },
        }
    }

    fn route_view(&self, ship_id: Option<ShipId>, route: &RedactedRoute) -> RouteView {
        RouteView {
            ship_id,
            stops: route
                .stops
                .iter()
                .map(|stop| RouteStopView {
                    system_id: stop.system.clone(),
                    label: stop.system.as_ref().map(|system| {
                        self.aliases
                            .get(system)
                            .cloned()
                            .unwrap_or_else(|| self.catalogue.system_label(system))
                    }),
                    reached: stop.reached,
                })
                .collect(),
            total_distance: route.total_distance,
        }
    }

    fn probe_assessment(
        &self,
        assessment: game_core::ProbeLaunchAssessment,
    ) -> ProbeAssessmentView {
        ProbeAssessmentView {
            source_id: assessment.source,
            ship_id: assessment.ship_id.clone(),
            target_id: assessment.target.clone(),
            target_label: self.catalogue.system_label(&assessment.target),
            requested_jump_limit: assessment.requested_jump_limit,
            minimum_jump_limit: assessment.minimum_jump_limit,
            maximum_jump_limit: assessment.maximum_jump_limit,
            target_knowledge: assessment.target_knowledge,
            asset_ready: assessment.asset_ready,
            travel_energy: assessment.travel_energy,
            route: assessment
                .route
                .as_ref()
                .map(|route| self.route_view(Some(assessment.ship_id), route)),
            availability: availability(assessment.limiting_reason.as_ref(), &self.catalogue),
        }
    }

    fn expedition_assessment(
        &self,
        assessment: game_core::ExpeditionLaunchAssessment,
        core: &PlayerWorldView,
    ) -> ExpeditionAssessmentView {
        let reservation_choices = core
            .systems
            .iter()
            .find(|system| {
                system.system == assessment.target
                    && system.knowledge.level == KnowledgeLevel::Complete
            })
            .map_or_else(Vec::new, |system| complete_slot_choices(&system.knowledge));
        ExpeditionAssessmentView {
            source_id: assessment.source,
            ship_id: assessment.ship_id.clone(),
            target_id: assessment.target.clone(),
            target_label: self.catalogue.system_label(&assessment.target),
            reservations: assessment.reservations,
            reservation_choices,
            complete_commitment: resource_rows(&assessment.complete_commitment, &self.catalogue),
            resident_population_required: assessment.resident_population_required,
            resident_population_available: assessment.resident_population_available,
            resident_population_ready: assessment.resident_population_ready,
            target_knowledge: assessment.target_knowledge,
            asset_ready: assessment.asset_ready,
            travel_energy: assessment.travel_energy,
            route: assessment
                .route
                .as_ref()
                .map(|route| self.route_view(Some(assessment.ship_id), route)),
            availability: availability(assessment.limiting_reason.as_ref(), &self.catalogue),
        }
    }
}

fn known_position(knowledge: &game_core::SystemKnowledge) -> Option<Position3> {
    knowledge
        .facts
        .get(&FactKey::Position)
        .and_then(|fact| match fact.value {
            FactValue::Position(position) => Some(position),
            _ => None,
        })
}

fn complete_slot_choices(knowledge: &game_core::SystemKnowledge) -> Vec<SlotCoordinate> {
    let mut choices = Vec::new();
    let bodies = knowledge
        .facts
        .get(&FactKey::BodyOrder)
        .and_then(|fact| match &fact.value {
            FactValue::ContentIds(ids) => Some(ids),
            _ => None,
        });
    if let Some(bodies) = bodies {
        for body in bodies {
            if let Some(fact) = knowledge
                .facts
                .get(&FactKey::SlotOrder { body: body.clone() })
                && let FactValue::ContentIds(slots) = &fact.value
            {
                choices.extend(slots.iter().map(|slot| SlotCoordinate {
                    body: body.clone(),
                    slot: slot.clone(),
                }));
            }
        }
    }
    choices
}

fn resource_rows(store: &ResourceStore, catalogue: &Catalogue) -> Vec<ResourceAmountView> {
    store
        .quantities
        .iter()
        .map(|(resource, quantity)| ResourceAmountView {
            resource_id: resource.clone(),
            label: catalogue.resource_label(resource),
            quantity: *quantity,
        })
        .collect()
}

fn body_remaining(snapshot: &SystemSnapshot, body_id: &ContentId, resource: &ContentId) -> u64 {
    snapshot
        .bodies
        .iter()
        .find(|body| &body.id == body_id)
        .map_or(0, |body| body.remaining_resources.quantity(resource))
}

fn construction_option(
    assessment: game_core::ConstructionAssessment,
    catalogue: &Catalogue,
) -> ConstructionOptionView {
    ConstructionOptionView {
        role: assessment.role,
        extractor_resource_label: assessment
            .extractor_resource
            .as_ref()
            .map(|resource| catalogue.resource_label(resource)),
        extractor_resource_id: assessment.extractor_resource,
        cost: resource_rows(&assessment.cost, catalogue),
        required_work: assessment.required_work,
        availability: availability(assessment.limiting_reason.as_ref(), catalogue),
    }
}

fn availability(error: Option<&CoreError>, catalogue: &Catalogue) -> ActionAvailability {
    match error {
        None => ActionAvailability::Available,
        Some(error) => {
            let reason = limiting_reason(error, catalogue);
            let message = player_error_message(&reason);
            ActionAvailability::Unavailable { reason, message }
        }
    }
}

fn limiting_reason(error: &CoreError, catalogue: &Catalogue) -> LimitingReason {
    match error {
        CoreError::UnknownSystem(_) => LimitingReason::UnknownSystem,
        CoreError::SystemNotCommandable(_) => LimitingReason::SystemNotCommandable,
        CoreError::UnknownBody(_) => LimitingReason::UnknownBody,
        CoreError::UnknownDevelopmentSlot { .. } => LimitingReason::UnknownSlot,
        CoreError::DevelopmentSlotUnavailable { .. } => LimitingReason::SlotUnavailable,
        CoreError::ExtractorTargetRequired => LimitingReason::ExtractorTargetRequired,
        CoreError::UnexpectedExtractorTarget => LimitingReason::UnexpectedExtractorTarget,
        CoreError::IncompatibleExtractorTarget { .. } => {
            LimitingReason::IncompatibleExtractorTarget
        }
        CoreError::InsufficientResource {
            resource,
            available,
            requested,
        } => LimitingReason::InsufficientResource {
            resource_id: resource.clone(),
            resource_label: catalogue.resource_label(resource),
            available: *available,
            required: *requested,
        },
        CoreError::UnknownProject(_) | CoreError::InvalidShipProject(_) => {
            LimitingReason::UnknownProject
        }
        CoreError::ConstructionAlreadyBegun(_) | CoreError::ShipProjectAlreadyBegun(_) => {
            LimitingReason::ProjectAlreadyBegun
        }
        CoreError::NotFunctionalShipyard { .. } => LimitingReason::NotFunctionalShipyard,
        CoreError::NoOperationalShipyard(_) => LimitingReason::NoOperationalShipyard,
        CoreError::UnknownCompletedShip(_) => LimitingReason::UnknownShip,
        CoreError::WrongCompletedShipKind(_) => LimitingReason::WrongShipKind,
        CoreError::CompletedShipNotReady(_) => LimitingReason::ShipNotReady,
        CoreError::ShipTargetMustBeDistinct(_) | CoreError::SystemNotTargetable(_) => {
            LimitingReason::InvalidTarget
        }
        CoreError::InvalidProbeJumpLimit { requested, maximum } => {
            LimitingReason::InvalidJumpLimit {
                requested: *requested,
                maximum: *maximum,
            }
        }
        CoreError::NoShipRoute { .. } => LimitingReason::NoRoute,
        CoreError::CompleteKnowledgeRequiresReservations(_) => LimitingReason::ReservationsRequired,
        CoreError::SummaryKnowledgeCannotReserve(_) => LimitingReason::ReservationsNotAllowed,
        CoreError::IncompleteTargetSlotKnowledge(_) => {
            LimitingReason::IncompleteReservationKnowledge
        }
        CoreError::InvalidExpeditionReservation(_) => LimitingReason::InvalidReservation,
        CoreError::NoResidentPopulation(_) => LimitingReason::NoResidentPopulation,
        CoreError::Overflow => LimitingReason::ArithmeticOverflow,
        _ => LimitingReason::InvalidState,
    }
}

fn player_error_message(reason: &LimitingReason) -> String {
    match reason {
        LimitingReason::InsufficientResource {
            resource_label,
            available,
            required,
            ..
        } => format!("Not enough {resource_label}: {available} available, {required} required"),
        LimitingReason::SlotUnavailable => "The selected slot is no longer available".into(),
        LimitingReason::ExtractorTargetRequired => "Select an Extractor resource target".into(),
        LimitingReason::IncompatibleExtractorTarget => {
            "That resource cannot be extracted from this body".into()
        }
        LimitingReason::NoResidentPopulation => {
            "No resident population is available for departure".into()
        }
        LimitingReason::NoRoute => "No route is available within the selected jump limit".into(),
        LimitingReason::InvalidJumpLimit { requested, maximum } => {
            format!("Jump limit {requested} exceeds the authored maximum {maximum}")
        }
        LimitingReason::ReservationsRequired => {
            "Complete target knowledge requires Habitat and Collector reservations".into()
        }
        LimitingReason::ReservationsNotAllowed => {
            "Summary knowledge cannot name target reservations".into()
        }
        LimitingReason::ProjectAlreadyBegun => {
            "The project has already begun and cannot be cancelled".into()
        }
        LimitingReason::ShipNotReady => "The completed ship is not ready until a later tick".into(),
        _ => "The current state does not allow that action".into(),
    }
}

fn construction_disposition(error: &CoreError) -> DraftDisposition {
    match error {
        CoreError::UnknownSystem(_)
        | CoreError::SystemNotCommandable(_)
        | CoreError::UnknownBody(_)
        | CoreError::UnknownDevelopmentSlot { .. }
        | CoreError::DevelopmentSlotUnavailable { .. } => DraftDisposition::InvalidateRoot,
        _ => DraftDisposition::Retain,
    }
}

fn expedition_disposition(error: &CoreError) -> DraftDisposition {
    match error {
        CoreError::UnknownSystem(_)
        | CoreError::SystemNotCommandable(_)
        | CoreError::SystemNotTargetable(_)
        | CoreError::UnknownCompletedShip(_) => DraftDisposition::InvalidateRoot,
        _ => DraftDisposition::Retain,
    }
}

fn accepted_outcome(
    intent: IntentKind,
    message: &str,
    project_id: Option<ProjectId>,
    ship_id: Option<ShipId>,
) -> ApplicationOutcome {
    ApplicationOutcome {
        accepted: true,
        intent,
        message: message.into(),
        limiting_reason: None,
        draft_disposition: None,
        project_id,
        ship_id,
    }
}

fn alias_rejection(reason: LimitingReason, message: &str) -> ApplicationOutcome {
    ApplicationOutcome {
        accepted: false,
        intent: IntentKind::Alias,
        message: message.into(),
        limiting_reason: Some(reason),
        draft_disposition: Some(DraftDisposition::Retain),
        project_id: None,
        ship_id: None,
    }
}

fn tick_delta(before: &PlayingView, after: &PlayingView) -> TickDeltaView {
    let before_locals = before
        .local_systems
        .iter()
        .map(|local| (&local.system_id, local))
        .collect::<BTreeMap<_, _>>();
    let mut stock_changes = Vec::new();
    let mut population_changes = Vec::new();
    for local in &after.local_systems {
        if let Some(previous) = before_locals.get(&local.system_id) {
            let old = previous
                .stocks
                .iter()
                .map(|row| (&row.resource_id, row.quantity))
                .collect::<BTreeMap<_, _>>();
            for row in &local.stocks {
                let before_quantity = old.get(&row.resource_id).copied().unwrap_or(0);
                if before_quantity != row.quantity {
                    stock_changes.push(StockChangeView {
                        system_id: local.system_id.clone(),
                        resource_id: row.resource_id.clone(),
                        label: row.label.clone(),
                        before: before_quantity,
                        after: row.quantity,
                    });
                }
            }
            if previous.population_count != local.population_count {
                population_changes.push(PopulationChangeView {
                    system_id: local.system_id.clone(),
                    before: previous.population_count,
                    after: local.population_count,
                });
            }
        }
    }
    let old_systems = before
        .systems
        .iter()
        .map(|system| &system.system_id)
        .collect::<BTreeSet<_>>();
    let newly_identified_systems = after
        .systems
        .iter()
        .filter(|system| !old_systems.contains(&system.system_id))
        .map(|system| system.system_id.clone())
        .collect();
    TickDeltaView {
        from_tick: before.time.tick,
        to_tick: after.time.tick,
        stock_changes,
        population_changes,
        newly_identified_systems,
        mission_changes: before
            .missions
            .iter()
            .zip(&after.missions)
            .filter(|(left, right)| left != right)
            .count()
            + before.missions.len().abs_diff(after.missions.len()),
    }
}

#[cfg(test)]
mod tests;
