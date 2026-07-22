use crate::input::{Action, Direction, KeyboardLayout, map_key};
use crate::terminal::TerminalEvent;
use crossterm::event::KeyEvent;
use game_app::{
    ActionAvailability, ApplicationError, ApplicationOutcome, BodyView, ContentId,
    DraftDisposition, ExpeditionAssessmentView, ExpeditionReservations, GenerationPreviewView,
    PlayingView, ProbeAssessmentView, ProfileDescriptor, Session, SessionIntent, SessionOutcome,
    ShipActionView, ShipProjectKind, StartupCoordinator, StartupFailure, StartupView, TickStepView,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const MIN_WIDTH: u16 = 160;
pub const MIN_HEIGHT: u16 = 45;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Dashboard,
    SystemDetails,
    Local,
    Operations,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatchStatus {
    Running,
    Paused,
    Stopped,
    Complete,
    Rejected,
}

#[derive(Clone, Debug)]
pub struct TickBatch {
    pub requested: u8,
    pub rate: u8,
    pub status: BatchStatus,
    pub history: Vec<TickStepView>,
    pub selected_history: usize,
    pub rejection: Option<ApplicationOutcome>,
    next_due: Duration,
}

impl TickBatch {
    fn new(requested: u8, rate: u8, now: Duration) -> Self {
        Self {
            requested,
            rate,
            status: BatchStatus::Running,
            history: Vec::new(),
            selected_history: 0,
            rejection: None,
            next_due: now,
        }
    }

    #[must_use]
    pub fn completed(&self) -> usize {
        self.history.len()
    }

    fn period(&self) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(self.rate))
    }
}

#[derive(Clone, Debug)]
pub struct ConstructionDraft {
    pub system_id: ContentId,
    pub body_id: ContentId,
    pub slot_id: ContentId,
    pub options: Vec<game_app::ConstructionOptionView>,
    pub selected: usize,
}

#[derive(Clone, Debug)]
pub enum MissionDraft {
    Probe(ProbeAssessmentView),
    Expedition(ExpeditionAssessmentView),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorKind {
    Profile,
    Seed,
    Alias,
    Batch,
}

#[derive(Clone, Debug)]
pub struct EditorState {
    pub kind: EditorKind,
    pub value: String,
    pub rate: u8,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Confirmation {
    Quit,
    Development {
        system_id: ContentId,
        body_id: ContentId,
        slot_id: ContentId,
        label: String,
        enabled: bool,
    },
    Habitat {
        system_id: ContentId,
        body_id: ContentId,
        slot_id: ContentId,
        enabled: bool,
        progress: u64,
    },
    Construction,
    Probe,
    Expedition,
}

#[derive(Clone, Debug)]
pub enum Modal {
    Help,
    Settings,
    Editor(EditorState),
    Confirm(Confirmation),
    Rejection(ApplicationOutcome),
    Batch(TickBatch),
    Mission(Box<MissionDraft>),
}

/// Stateful terminal adapter. It owns only presentation state plus the two
/// public game-app coordinators; authoritative simulation remains in `Session`.
pub struct TuiState {
    startup: Option<StartupCoordinator>,
    session: Option<Session>,
    startup_view: Option<StartupView>,
    playing_view: Option<PlayingView>,
    pub layout: KeyboardLayout,
    pub screen: Screen,
    pub modal: Option<Modal>,
    pub selected_system: usize,
    pub selected_body: usize,
    pub selected_slot: usize,
    pub construction: Option<ConstructionDraft>,
    pub mission_draft: Option<MissionDraft>,
    pub mission_jump_override: Option<u64>,
    pub mission_jump_input: Option<String>,
    pub mission_jump_error: Option<String>,
    pub notice: Option<ApplicationOutcome>,
    pub startup_focus: usize,
    pub undersized: bool,
    pub terminal_size: (u16, u16),
    pub should_quit: bool,
    pub default_batch_rate: u8,
}

impl TuiState {
    #[must_use]
    pub fn new(profile: ProfileDescriptor, seed: u64) -> Self {
        let startup = StartupCoordinator::new(profile, seed);
        let startup_view = Some(startup.view());
        Self {
            startup: Some(startup),
            session: None,
            startup_view,
            playing_view: None,
            layout: KeyboardLayout::default(),
            screen: Screen::Dashboard,
            modal: None,
            selected_system: 0,
            selected_body: 0,
            selected_slot: 0,
            construction: None,
            mission_draft: None,
            mission_jump_override: None,
            mission_jump_input: None,
            mission_jump_error: None,
            notice: None,
            startup_focus: 0,
            undersized: false,
            terminal_size: (MIN_WIDTH, MIN_HEIGHT),
            should_quit: false,
            default_batch_rate: 5,
        }
    }

    #[must_use]
    pub fn startup_view(&self) -> Option<&StartupView> {
        self.startup_view.as_ref()
    }

    #[must_use]
    pub fn playing_view(&self) -> Option<&PlayingView> {
        self.playing_view.as_ref()
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.session.is_some()
    }

    #[must_use]
    pub fn selected_system_id(&self) -> Option<&ContentId> {
        self.playing_view
            .as_ref()?
            .systems
            .get(self.selected_system)
            .map(|row| &row.system_id)
    }

    #[must_use]
    pub fn selected_preview(&self) -> Option<&GenerationPreviewView> {
        self.startup_view.as_ref()?.preview.as_ref()
    }

    pub fn handle_event(
        &mut self,
        event: TerminalEvent,
        now: Duration,
    ) -> Result<(), ApplicationError> {
        match event {
            TerminalEvent::Resize { width, height } => self.resize(width, height),
            TerminalEvent::Key(key) => self.handle_key(key, now)?,
            TerminalEvent::Wake => {}
        }
        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyEvent, now: Duration) -> Result<(), ApplicationError> {
        let editor = matches!(self.modal, Some(Modal::Editor(_)));
        let action = map_key(key, self.layout, editor, true);
        self.handle_action(action, now)
    }

    pub fn handle_action(&mut self, action: Action, now: Duration) -> Result<(), ApplicationError> {
        // Safety has absolute precedence. Existing quit confirmation is still
        // operable, but no gameplay action can pass this branch.
        if self.undersized {
            match (&self.modal, action) {
                (Some(Modal::Confirm(Confirmation::Quit)), Action::Confirm) => {
                    self.should_quit = true
                }
                (Some(Modal::Confirm(Confirmation::Quit)), Action::Cancel) => self.modal = None,
                (_, Action::Quit) if self.is_playing() => {
                    self.modal = Some(Modal::Confirm(Confirmation::Quit))
                }
                (_, Action::Quit) => self.should_quit = true,
                _ => {}
            }
            return Ok(());
        }

        if matches!(self.modal, Some(Modal::Editor(_))) {
            return self.handle_editor(action, now);
        }
        if self.modal.is_some() {
            return self.handle_modal(action, now);
        }

        match action {
            Action::Help => self.modal = Some(Modal::Help),
            Action::Settings => self.modal = Some(Modal::Settings),
            Action::Quit if self.is_playing() => {
                self.modal = Some(Modal::Confirm(Confirmation::Quit))
            }
            Action::Quit => self.should_quit = true,
            _ if self.is_playing() => self.handle_playing(action, now)?,
            _ => self.handle_startup(action)?,
        }
        Ok(())
    }

    fn handle_startup(&mut self, action: Action) -> Result<(), ApplicationError> {
        const STARTUP_TARGETS: usize = 5;
        match action {
            Action::NextFocus | Action::Navigate(Direction::Down | Direction::Right) => {
                self.startup_focus = (self.startup_focus + 1) % STARTUP_TARGETS;
            }
            Action::PreviousFocus | Action::Navigate(Direction::Up | Direction::Left) => {
                self.startup_focus = (self.startup_focus + STARTUP_TARGETS - 1) % STARTUP_TARGETS;
            }
            Action::Confirm => match self.startup_focus {
                0 => self.open_editor(EditorKind::Profile),
                1 => self.open_editor(EditorKind::Seed),
                2 => self.choose_random_seed(),
                3 => self.generate(),
                _ => self.start_current_preview()?,
            },
            Action::Character('n') => self.choose_random_seed(),
            Action::Character('g') => self.generate(),
            _ => {}
        }
        Ok(())
    }

    fn choose_random_seed(&mut self) {
        let current = self
            .startup_view
            .as_ref()
            .and_then(|view| view.seed_text.parse::<u64>().ok())
            .unwrap_or_default();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let mut seed = timestamp ^ current.wrapping_add(0x9e37_79b9_7f4a_7c15);
        seed = (seed ^ (seed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        seed = (seed ^ (seed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        seed ^= seed >> 31;
        if seed == current {
            seed = seed.wrapping_add(1);
        }
        if let Some(startup) = self.startup.as_mut() {
            startup.edit_seed(seed.to_string());
            self.startup_view = Some(startup.view());
        }
    }

    fn open_editor(&mut self, kind: EditorKind) {
        let value = match kind {
            EditorKind::Profile => self.startup_view.as_ref().map_or_else(String::new, |view| {
                view.profile.machine_path.display().to_string()
            }),
            EditorKind::Seed => self
                .startup_view
                .as_ref()
                .map_or_else(String::new, |view| view.seed_text.clone()),
            EditorKind::Alias => self
                .playing_view
                .as_ref()
                .and_then(|view| view.systems.get(self.selected_system))
                .and_then(|row| row.alias.clone())
                .unwrap_or_default(),
            EditorKind::Batch => "10".into(),
        };
        self.modal = Some(Modal::Editor(EditorState {
            kind,
            value,
            rate: self.default_batch_rate,
            error: None,
        }));
    }

    fn generate(&mut self) {
        if let Some(startup) = self.startup.as_mut() {
            let _ = startup.generate_preview();
            self.startup_view = Some(startup.view());
            if self
                .startup_view
                .as_ref()
                .and_then(|view| view.preview.as_ref())
                .is_some()
            {
                self.startup_focus = 4;
            }
        }
    }

    fn start_current_preview(&mut self) -> Result<(), ApplicationError> {
        let Some(mut startup) = self.startup.take() else {
            return Ok(());
        };
        if startup.request_start_current_preview().is_err() {
            self.startup_view = Some(startup.view());
            self.startup = Some(startup);
            return Ok(());
        }
        match startup.confirm_start_current_preview() {
            Ok(session) => {
                self.playing_view = Some(session.playing_view()?);
                self.session = Some(session);
                self.startup_view = None;
            }
            Err(_) => {
                self.startup_view = Some(startup.view());
                self.startup = Some(startup);
            }
        }
        Ok(())
    }

    fn handle_editor(&mut self, action: Action, now: Duration) -> Result<(), ApplicationError> {
        let Some(Modal::Editor(mut editor)) = self.modal.take() else {
            return Ok(());
        };
        match action {
            Action::Cancel => return Ok(()),
            Action::Backspace => {
                editor.value.pop();
            }
            Action::Delete => editor.value.clear(),
            Action::Character(character) => {
                let accepted = match editor.kind {
                    EditorKind::Seed | EditorKind::Batch => character.is_ascii_digit(),
                    EditorKind::Profile | EditorKind::Alias => !character.is_control(),
                };
                if accepted {
                    editor.value.push(character);
                }
            }
            Action::Navigate(Direction::Left) if editor.kind == EditorKind::Batch => {
                editor.rate = match editor.rate {
                    10 => 5,
                    5 => 1,
                    _ => 10,
                };
            }
            Action::Navigate(Direction::Right) if editor.kind == EditorKind::Batch => {
                editor.rate = match editor.rate {
                    1 => 5,
                    5 => 10,
                    _ => 1,
                };
            }
            Action::Confirm => {
                match editor.kind {
                    EditorKind::Profile => {
                        if let Some(startup) = self.startup.as_mut() {
                            let logical = self
                                .startup_view
                                .as_ref()
                                .map_or("profile", |view| view.profile.logical_source_id.as_str())
                                .to_owned();
                            startup.edit_profile(ProfileDescriptor::new(editor.value, logical));
                            self.startup_view = Some(startup.view());
                        }
                    }
                    EditorKind::Seed => {
                        if editor.value.parse::<u64>().is_err() {
                            editor.error = Some("Enter an unsigned 64-bit seed".into());
                            self.modal = Some(Modal::Editor(editor));
                            return Ok(());
                        }
                        if let Some(startup) = self.startup.as_mut() {
                            startup.edit_seed(editor.value);
                            self.startup_view = Some(startup.view());
                        }
                    }
                    EditorKind::Alias => {
                        if let Some(system_id) = self.selected_system_id().cloned() {
                            let alias =
                                (!editor.value.trim().is_empty()).then_some(editor.value.clone());
                            if let Some(outcome) =
                                self.dispatch(SessionIntent::SetSystemAlias { system_id, alias })?
                                && !outcome.accepted
                            {
                                editor.error = Some(outcome.message.clone());
                                self.notice = Some(outcome);
                                self.modal = Some(Modal::Editor(editor));
                            }
                        }
                    }
                    EditorKind::Batch => match editor.value.parse::<u8>() {
                        Ok(count @ 1..=100) => {
                            self.default_batch_rate = editor.rate;
                            self.modal =
                                Some(Modal::Batch(TickBatch::new(count, editor.rate, now)));
                        }
                        _ => {
                            editor.error = Some("Count must be 1..100".into());
                            self.modal = Some(Modal::Editor(editor));
                        }
                    },
                }
                return Ok(());
            }
            _ => {}
        }
        self.modal = Some(Modal::Editor(editor));
        Ok(())
    }

    fn handle_modal(&mut self, action: Action, now: Duration) -> Result<(), ApplicationError> {
        let Some(modal) = self.modal.take() else {
            return Ok(());
        };
        match modal {
            Modal::Help => {
                if !matches!(action, Action::Cancel | Action::Help) {
                    self.modal = Some(Modal::Help);
                }
            }
            Modal::Settings => match action {
                Action::Cancel | Action::Settings => {}
                Action::Confirm
                | Action::Navigate(
                    Direction::Left | Direction::Right | Direction::Up | Direction::Down,
                ) => {
                    self.layout = self.layout.toggled();
                    self.modal = Some(Modal::Settings);
                }
                _ => self.modal = Some(Modal::Settings),
            },
            Modal::Confirm(confirmation) => self.handle_confirmation(confirmation, action)?,
            Modal::Rejection(outcome) => {
                if matches!(action, Action::Confirm | Action::Cancel) {
                    match outcome.draft_disposition {
                        Some(DraftDisposition::InvalidateRoot) => {
                            self.construction = None;
                            self.mission_draft = None;
                            self.screen = Screen::Local;
                        }
                        Some(DraftDisposition::Retain) if self.construction.is_some() => {
                            self.modal = Some(Modal::Confirm(Confirmation::Construction));
                        }
                        Some(DraftDisposition::Retain) => {
                            if let Some(mission) = self.mission_draft.take() {
                                self.modal = Some(Modal::Mission(Box::new(mission)));
                            }
                        }
                        None => {}
                    }
                } else {
                    self.modal = Some(Modal::Rejection(outcome));
                }
            }
            Modal::Batch(mut batch) => {
                match action {
                    Action::Pause if batch.status == BatchStatus::Running => {
                        batch.status = BatchStatus::Paused
                    }
                    Action::Pause if batch.status == BatchStatus::Paused => {
                        batch.status = BatchStatus::Running;
                        batch.next_due = now.saturating_add(batch.period());
                    }
                    Action::Confirm if batch.status == BatchStatus::Paused => {
                        self.step_batch(&mut batch, now, true)?;
                    }
                    Action::Cancel
                        if matches!(batch.status, BatchStatus::Running | BatchStatus::Paused) =>
                    {
                        batch.status = BatchStatus::Stopped
                    }
                    Action::Cancel | Action::Confirm
                        if matches!(
                            batch.status,
                            BatchStatus::Complete | BatchStatus::Stopped | BatchStatus::Rejected
                        ) =>
                    {
                        return Ok(());
                    }
                    Action::Navigate(Direction::Up) if !batch.history.is_empty() => {
                        batch.selected_history = batch.selected_history.saturating_sub(1)
                    }
                    Action::Navigate(Direction::Down) if !batch.history.is_empty() => {
                        batch.selected_history =
                            (batch.selected_history + 1).min(batch.history.len() - 1)
                    }
                    _ => {}
                }
                self.modal = Some(Modal::Batch(batch));
            }
            Modal::Mission(mut mission) => {
                match action {
                    Action::Cancel => {
                        self.mission_jump_override = None;
                        self.mission_jump_input = None;
                        self.mission_jump_error = None;
                        return Ok(());
                    }
                    Action::Character(character)
                        if matches!(&*mission, MissionDraft::Probe(_))
                            && character.is_ascii_digit() =>
                    {
                        let input = self.mission_jump_input.get_or_insert_default();
                        if input.len() < 20 {
                            input.push(character);
                        }
                        self.mission_jump_error = None;
                    }
                    Action::Backspace if matches!(&*mission, MissionDraft::Probe(_)) => {
                        if self.mission_jump_input.is_none()
                            && let Some(value) = self.mission_jump_override
                        {
                            self.mission_jump_input = Some(value.to_string());
                        }
                        if let Some(input) = &mut self.mission_jump_input {
                            input.pop();
                        }
                        self.mission_jump_error = None;
                    }
                    Action::Delete if matches!(&*mission, MissionDraft::Probe(_)) => {
                        self.clear_probe_jump_override(&mut mission)?;
                    }
                    Action::Navigate(Direction::Left) => {
                        self.adjust_mission(&mut mission, false)?
                    }
                    Action::Navigate(Direction::Right) => {
                        self.adjust_mission(&mut mission, true)?
                    }
                    Action::Navigate(Direction::Up) => {
                        self.cycle_mission_target(&mut mission, -1)?;
                    }
                    Action::Navigate(Direction::Down) => {
                        self.cycle_mission_target(&mut mission, 1)?;
                    }
                    Action::Confirm => {
                        if let MissionDraft::Probe(probe) = &mut *mission {
                            if let Some(entered) = self.mission_jump_input.as_deref() {
                                let jump_limit = match entered.parse::<u64>() {
                                    Ok(value) => value,
                                    Err(_) => {
                                        self.mission_jump_error =
                                            Some("Enter a valid jump distance".into());
                                        self.modal = Some(Modal::Mission(mission));
                                        return Ok(());
                                    }
                                };
                                if !(probe.minimum_jump_limit..=probe.maximum_jump_limit)
                                    .contains(&jump_limit)
                                {
                                    self.mission_jump_error = Some(format!(
                                        "Jump distance must be {}..={}",
                                        probe.minimum_jump_limit, probe.maximum_jump_limit
                                    ));
                                    self.modal = Some(Modal::Mission(mission));
                                    return Ok(());
                                }
                                let outcome =
                                    self.session.as_mut().expect("playing session").dispatch(
                                        SessionIntent::AssessProbeLaunch {
                                            source_id: probe.source_id.clone(),
                                            ship_id: probe.ship_id.clone(),
                                            target_id: probe.target_id.clone(),
                                            jump_limit,
                                        },
                                    )?;
                                if let SessionOutcome::ProbeAssessment(value) = outcome {
                                    *probe = value;
                                }
                                self.mission_jump_override = Some(jump_limit);
                                self.mission_jump_input = None;
                                self.mission_jump_error = None;
                                self.modal = Some(Modal::Mission(mission));
                                return Ok(());
                            }
                            if matches!(probe.availability, ActionAvailability::Unavailable { .. })
                            {
                                self.modal = Some(Modal::Mission(mission));
                                return Ok(());
                            }
                        }
                        let confirmation = match &*mission {
                            MissionDraft::Probe(_) => Confirmation::Probe,
                            MissionDraft::Expedition(_) => Confirmation::Expedition,
                        };
                        self.mission_draft = Some(*mission);
                        self.mission_jump_input = None;
                        self.mission_jump_error = None;
                        self.modal = Some(Modal::Confirm(confirmation));
                        return Ok(());
                    }
                    _ => {}
                }
                self.modal = Some(Modal::Mission(mission));
            }
            Modal::Editor(_) => unreachable!(),
        }
        Ok(())
    }

    fn handle_confirmation(
        &mut self,
        confirmation: Confirmation,
        action: Action,
    ) -> Result<(), ApplicationError> {
        if confirmation == Confirmation::Construction
            && let Some(draft) = &mut self.construction
        {
            match action {
                Action::Navigate(Direction::Up) if !draft.options.is_empty() => {
                    draft.selected = offset(draft.selected, -1, draft.options.len());
                    self.modal = Some(Modal::Confirm(confirmation));
                    return Ok(());
                }
                Action::Navigate(Direction::Down) if !draft.options.is_empty() => {
                    draft.selected = offset(draft.selected, 1, draft.options.len());
                    self.modal = Some(Modal::Confirm(confirmation));
                    return Ok(());
                }
                _ => {}
            }
        }
        if action == Action::Cancel {
            if confirmation == Confirmation::Construction {
                self.construction = None;
            }
            if matches!(confirmation, Confirmation::Probe | Confirmation::Expedition)
                && let Some(mission) = self.mission_draft.take()
            {
                self.modal = Some(Modal::Mission(Box::new(mission)));
            }
            return Ok(());
        }
        if action != Action::Confirm {
            self.modal = Some(Modal::Confirm(confirmation));
            return Ok(());
        }
        match confirmation {
            Confirmation::Quit => self.should_quit = true,
            Confirmation::Development {
                system_id,
                body_id,
                slot_id,
                enabled,
                ..
            } => {
                let _ = self.dispatch(SessionIntent::SetDevelopmentOperationalEnabled {
                    system_id,
                    body_id,
                    slot_id,
                    enabled,
                })?;
            }
            Confirmation::Habitat {
                system_id,
                body_id,
                slot_id,
                enabled,
                ..
            } => {
                let _ = self.dispatch(SessionIntent::SetHabitatGenerationEnabled {
                    system_id,
                    body_id,
                    slot_id,
                    enabled,
                })?;
            }
            Confirmation::Construction => self.commit_construction()?,
            Confirmation::Probe | Confirmation::Expedition => self.confirm_mission()?,
        }
        Ok(())
    }

    fn handle_playing(&mut self, action: Action, now: Duration) -> Result<(), ApplicationError> {
        match action {
            Action::AdvanceOne => {
                let _ = self.dispatch(SessionIntent::AdvanceOneTick)?;
            }
            Action::AdvanceMany => self.open_editor(EditorKind::Batch),
            Action::Character('r')
                if self
                    .playing_view
                    .as_ref()
                    .and_then(|view| view.systems.get(self.selected_system))
                    .is_some_and(|system| system.chart_position.is_some()) =>
            {
                self.open_editor(EditorKind::Alias)
            }
            Action::Navigate(Direction::Up) if self.screen == Screen::Dashboard => {
                self.select_system(-1)
            }
            Action::Navigate(Direction::Down) if self.screen == Screen::Dashboard => {
                self.select_system(1)
            }
            Action::Confirm if self.screen == Screen::Dashboard => {
                if self.selected_local().is_some() {
                    self.screen = Screen::Local;
                    self.select_first_local_slot();
                } else {
                    self.screen = Screen::SystemDetails;
                }
            }
            Action::Confirm
                if self.screen == Screen::SystemDetails && self.selected_local().is_some() =>
            {
                self.screen = Screen::Local;
                self.select_first_local_slot();
            }
            Action::Cancel if self.screen != Screen::Dashboard => self.screen = Screen::Dashboard,
            Action::Navigate(Direction::Up) if self.screen == Screen::Local => {
                self.move_local_slot(-1)
            }
            Action::Navigate(Direction::Down) if self.screen == Screen::Local => {
                self.move_local_slot(1)
            }
            Action::Character('b') if self.screen == Screen::Local => self.begin_construction(),
            Action::Character('e') if self.screen == Screen::Local => {
                self.request_development_toggle()
            }
            Action::Character('g') if self.screen == Screen::Local => self.request_habitat_toggle(),
            Action::Character('p') if self.screen == Screen::Local => self.ship_action(true)?,
            Action::Character('x') if self.screen == Screen::Local => self.ship_action(false)?,
            Action::Character('c') if self.screen == Screen::Local => {
                self.cancel_first_project()?
            }
            Action::Character('o') => self.screen = Screen::Operations,
            _ => {
                let _ = now;
            }
        }
        Ok(())
    }

    fn select_system(&mut self, delta: isize) {
        let len = self
            .playing_view
            .as_ref()
            .map_or(0, |view| view.systems.len());
        if len > 0 {
            self.selected_system = offset(self.selected_system, delta, len);
        }
    }

    fn selected_local(&self) -> Option<&game_app::LocalSystemView> {
        let system = self.selected_system_id()?;
        self.playing_view
            .as_ref()?
            .local_systems
            .iter()
            .find(|local| &local.system_id == system)
    }

    fn selected_body_view(&self) -> Option<&BodyView> {
        self.selected_local()?.bodies.get(self.selected_body)
    }

    fn select_first_local_slot(&mut self) {
        let first = self.selected_local().and_then(|local| {
            local
                .bodies
                .iter()
                .enumerate()
                .find_map(|(body, value)| (!value.slots.is_empty()).then_some((body, 0)))
        });
        if let Some((body, slot)) = first {
            self.selected_body = body;
            self.selected_slot = slot;
        } else {
            self.selected_body = 0;
            self.selected_slot = 0;
        }
    }

    fn move_local_slot(&mut self, delta: isize) {
        let positions = self.selected_local().map_or_else(Vec::new, |local| {
            local
                .bodies
                .iter()
                .enumerate()
                .flat_map(|(body_index, body)| {
                    (0..body.slots.len()).map(move |slot_index| (body_index, slot_index))
                })
                .collect::<Vec<_>>()
        });
        if positions.is_empty() {
            return;
        }
        let current = positions
            .iter()
            .position(|position| *position == (self.selected_body, self.selected_slot))
            .unwrap_or(0);
        let (body, slot) = positions[offset(current, delta, positions.len())];
        self.selected_body = body;
        self.selected_slot = slot;
    }

    fn begin_construction(&mut self) {
        let Some(system_id) = self.selected_system_id().cloned() else {
            return;
        };
        let Some(body) = self.selected_body_view() else {
            return;
        };
        let Some(slot) = body.slots.get(self.selected_slot) else {
            return;
        };
        if slot.development.is_none() && !slot.construction_options.is_empty() {
            self.construction = Some(ConstructionDraft {
                system_id,
                body_id: body.body_id.clone(),
                slot_id: slot.slot_id.clone(),
                options: slot.construction_options.clone(),
                selected: 0,
            });
            self.modal = Some(Modal::Confirm(Confirmation::Construction));
        }
    }

    fn commit_construction(&mut self) -> Result<(), ApplicationError> {
        let Some(draft) = self.construction.clone() else {
            return Ok(());
        };
        let Some(option) = draft.options.get(draft.selected) else {
            return Ok(());
        };
        let outcome = self.dispatch(SessionIntent::EnqueueConstruction {
            system_id: draft.system_id,
            body_id: draft.body_id,
            slot_id: draft.slot_id,
            role: option.role,
            extractor_resource_id: option.extractor_resource_id.clone(),
        })?;
        if let Some(outcome) = outcome {
            if outcome.accepted {
                self.construction = None;
            } else {
                if outcome.draft_disposition == Some(DraftDisposition::InvalidateRoot) {
                    self.construction = None;
                }
                self.modal = Some(Modal::Rejection(outcome));
            }
        }
        Ok(())
    }

    fn request_development_toggle(&mut self) {
        let Some(system_id) = self.selected_system_id().cloned() else {
            return;
        };
        let Some(body) = self.selected_body_view() else {
            return;
        };
        let Some(slot) = body.slots.get(self.selected_slot) else {
            return;
        };
        let Some(development) = &slot.development else {
            return;
        };
        if matches!(development.toggle, ActionAvailability::Available) {
            self.modal = Some(Modal::Confirm(Confirmation::Development {
                system_id,
                body_id: body.body_id.clone(),
                slot_id: slot.slot_id.clone(),
                label: format!("{:?}", development.role),
                enabled: !development.enabled,
            }));
        }
    }

    fn request_habitat_toggle(&mut self) {
        let Some(system_id) = self.selected_system_id().cloned() else {
            return;
        };
        let Some(body) = self.selected_body_view() else {
            return;
        };
        let Some(slot) = body.slots.get(self.selected_slot) else {
            return;
        };
        if let Some(habitat) = &slot.habitat
            && matches!(habitat.toggle, ActionAvailability::Available)
        {
            self.modal = Some(Modal::Confirm(Confirmation::Habitat {
                system_id,
                body_id: body.body_id.clone(),
                slot_id: slot.slot_id.clone(),
                enabled: !habitat.generation_enabled,
                progress: habitat.generation_progress,
            }));
        }
    }

    fn cancel_first_project(&mut self) -> Result<(), ApplicationError> {
        let project = self
            .selected_body_view()
            .and_then(|body| body.slots.get(self.selected_slot))
            .and_then(|slot| slot.shipyard_queue.iter().find(|row| row.cancellable))
            .map(|row| row.project_id.clone());
        if let Some(project_id) = project {
            let _ = self.dispatch(SessionIntent::CancelShipProject { project_id })?;
        }
        Ok(())
    }

    fn selected_ship_action(&self, probe: bool) -> Option<&ShipActionView> {
        let slot = self.selected_body_view()?.slots.get(self.selected_slot)?;
        Some(if probe {
            &slot.probe_action
        } else {
            &slot.expedition_action
        })
    }

    fn ship_action(&mut self, probe: bool) -> Result<(), ApplicationError> {
        let action = self.selected_ship_action(probe).cloned();
        match action {
            Some(ShipActionView::Launch { ship_id }) => self.begin_mission(probe, &ship_id),
            Some(ShipActionView::Enqueue) => self.enqueue_ship(if probe {
                ShipProjectKind::Probe
            } else {
                ShipProjectKind::Expedition
            }),
            Some(ShipActionView::Unavailable { .. }) | None => Ok(()),
        }
    }

    fn enqueue_ship(&mut self, kind: ShipProjectKind) -> Result<(), ApplicationError> {
        let Some(system_id) = self.selected_system_id().cloned() else {
            return Ok(());
        };
        let Some(body) = self.selected_body_view() else {
            return Ok(());
        };
        let Some(slot) = body.slots.get(self.selected_slot) else {
            return Ok(());
        };
        let _ = self.dispatch(SessionIntent::EnqueueShipProject {
            system_id,
            shipyard_body_id: body.body_id.clone(),
            shipyard_slot_id: slot.slot_id.clone(),
            kind,
        })?;
        Ok(())
    }

    fn begin_mission(
        &mut self,
        probe: bool,
        ship_id: &game_app::ShipId,
    ) -> Result<(), ApplicationError> {
        let Some(local) = self.selected_local() else {
            return Ok(());
        };
        let source = local.system_id.clone();
        let target = self
            .playing_view
            .as_ref()
            .and_then(|view| view.systems.iter().find(|row| row.system_id != source))
            .map(|row| row.system_id.clone());
        let Some(target) = target else {
            return Ok(());
        };
        let intent = if probe {
            SessionIntent::AssessProbeLaunch {
                source_id: source,
                ship_id: ship_id.clone(),
                target_id: target,
                jump_limit: self
                    .playing_view
                    .as_ref()
                    .map_or(1, |view| view.probe_maximum_jump_limit),
            }
        } else {
            SessionIntent::AssessExpeditionLaunch {
                source_id: source,
                ship_id: ship_id.clone(),
                target_id: target,
                reservations: None,
            }
        };
        let outcome = self
            .session
            .as_mut()
            .expect("playing session")
            .dispatch(intent)?;
        self.modal = match outcome {
            SessionOutcome::ProbeAssessment(value) => {
                self.mission_jump_override = None;
                self.mission_jump_input = None;
                self.mission_jump_error = None;
                Some(Modal::Mission(Box::new(MissionDraft::Probe(value))))
            }
            SessionOutcome::ExpeditionAssessment(value) => {
                self.mission_jump_input = None;
                self.mission_jump_error = None;
                Some(Modal::Mission(Box::new(MissionDraft::Expedition(value))))
            }
            _ => None,
        };
        Ok(())
    }

    fn clear_probe_jump_override(
        &mut self,
        mission: &mut MissionDraft,
    ) -> Result<(), ApplicationError> {
        let MissionDraft::Probe(probe) = mission else {
            return Ok(());
        };
        let outcome = self.session.as_mut().expect("playing session").dispatch(
            SessionIntent::AssessProbeLaunch {
                source_id: probe.source_id.clone(),
                ship_id: probe.ship_id.clone(),
                target_id: probe.target_id.clone(),
                jump_limit: probe.maximum_jump_limit,
            },
        )?;
        if let SessionOutcome::ProbeAssessment(value) = outcome {
            *probe = value;
        }
        self.mission_jump_override = None;
        self.mission_jump_input = None;
        self.mission_jump_error = None;
        Ok(())
    }

    fn adjust_mission(
        &mut self,
        mission: &mut MissionDraft,
        increase: bool,
    ) -> Result<(), ApplicationError> {
        match mission {
            MissionDraft::Probe(_) => {}
            MissionDraft::Expedition(expedition) if expedition.reservation_choices.len() >= 2 => {
                let choices = &expedition.reservation_choices;
                let current = expedition
                    .reservations
                    .as_ref()
                    .and_then(|selected| {
                        choices
                            .iter()
                            .position(|choice| choice == &selected.habitat)
                    })
                    .unwrap_or(if increase { choices.len() - 1 } else { 1 });
                let next = if increase {
                    (current + 1) % choices.len()
                } else {
                    (current + choices.len() - 1) % choices.len()
                };
                let reservations = ExpeditionReservations {
                    habitat: choices[next].clone(),
                    collector: choices[(next + 1) % choices.len()].clone(),
                };
                let outcome = self.session.as_mut().expect("playing session").dispatch(
                    SessionIntent::AssessExpeditionLaunch {
                        source_id: expedition.source_id.clone(),
                        ship_id: expedition.ship_id.clone(),
                        target_id: expedition.target_id.clone(),
                        reservations: Some(reservations),
                    },
                )?;
                if let SessionOutcome::ExpeditionAssessment(value) = outcome {
                    *expedition = value;
                }
            }
            MissionDraft::Expedition(_) => {}
        }
        Ok(())
    }

    fn cycle_mission_target(
        &mut self,
        mission: &mut MissionDraft,
        delta: isize,
    ) -> Result<(), ApplicationError> {
        let (source, current) = match mission {
            MissionDraft::Probe(value) => (&value.source_id, &value.target_id),
            MissionDraft::Expedition(value) => (&value.source_id, &value.target_id),
        };
        let targets = self.playing_view.as_ref().map_or_else(Vec::new, |view| {
            view.systems
                .iter()
                .filter(|row| &row.system_id != source)
                .map(|row| row.system_id.clone())
                .collect()
        });
        if targets.is_empty() {
            return Ok(());
        }
        let current_index = targets
            .iter()
            .position(|target| target == current)
            .unwrap_or(0);
        let target_id = targets[offset(current_index, delta, targets.len())].clone();
        let intent = match mission {
            MissionDraft::Probe(value) => SessionIntent::AssessProbeLaunch {
                source_id: value.source_id.clone(),
                ship_id: value.ship_id.clone(),
                target_id,
                jump_limit: self
                    .mission_jump_override
                    .unwrap_or(value.maximum_jump_limit),
            },
            MissionDraft::Expedition(value) => SessionIntent::AssessExpeditionLaunch {
                source_id: value.source_id.clone(),
                ship_id: value.ship_id.clone(),
                target_id,
                reservations: None,
            },
        };
        match self
            .session
            .as_mut()
            .expect("playing session")
            .dispatch(intent)?
        {
            SessionOutcome::ProbeAssessment(value) => *mission = MissionDraft::Probe(value),
            SessionOutcome::ExpeditionAssessment(value) => {
                *mission = MissionDraft::Expedition(value);
            }
            _ => {}
        }
        Ok(())
    }

    /// Commits the mission currently shown by a mission modal. This explicit API
    /// is also useful to executable adapters that provide richer target pickers.
    pub fn confirm_mission(&mut self) -> Result<(), ApplicationError> {
        let mission = match self.mission_draft.take() {
            Some(mission) => mission,
            None => {
                let Some(Modal::Mission(mission)) = self.modal.take() else {
                    return Ok(());
                };
                *mission
            }
        };
        let retained_mission = mission.clone();
        let intent = match mission {
            MissionDraft::Probe(value) => SessionIntent::LaunchProbe {
                source_id: value.source_id,
                ship_id: value.ship_id,
                target_id: value.target_id,
                jump_limit: value.requested_jump_limit,
            },
            MissionDraft::Expedition(value) => {
                let reservations = value.reservations;
                SessionIntent::LaunchExpedition {
                    source_id: value.source_id,
                    ship_id: value.ship_id,
                    target_id: value.target_id,
                    reservations,
                }
            }
        };
        if let Some(outcome) = self.dispatch(intent)?
            && !outcome.accepted
        {
            if outcome.draft_disposition == Some(DraftDisposition::Retain) {
                self.mission_draft = Some(retained_mission);
            }
            self.modal = Some(Modal::Rejection(outcome));
        }
        Ok(())
    }

    fn dispatch(
        &mut self,
        intent: SessionIntent,
    ) -> Result<Option<ApplicationOutcome>, ApplicationError> {
        let outcome = self
            .session
            .as_mut()
            .expect("session exists while playing")
            .dispatch(intent)?;
        match outcome {
            SessionOutcome::Applied { outcome, view } => {
                self.replace_playing_view(view);
                self.notice = Some(outcome.clone());
                Ok(Some(outcome))
            }
            SessionOutcome::Tick(step) => {
                self.replace_playing_view(step.view);
                Ok(None)
            }
            SessionOutcome::ProbeLaunched { outcome, view, .. }
            | SessionOutcome::ExpeditionLaunched { outcome, view, .. } => {
                self.replace_playing_view(view);
                self.notice = Some(outcome.clone());
                Ok(Some(outcome))
            }
            SessionOutcome::Rejected(outcome) => {
                self.notice = Some(outcome.clone());
                Ok(Some(outcome))
            }
            SessionOutcome::ProbeAssessment(_) | SessionOutcome::ExpeditionAssessment(_) => {
                Ok(None)
            }
        }
    }

    pub(crate) fn replace_playing_view(&mut self, view: PlayingView) {
        let selected_id = self.selected_system_id().cloned();
        let previous_index = self.selected_system;
        self.playing_view = Some(view);
        let systems = &self
            .playing_view
            .as_ref()
            .expect("view was replaced")
            .systems;
        self.selected_system = selected_id
            .as_ref()
            .and_then(|selected| {
                systems
                    .iter()
                    .position(|system| &system.system_id == selected)
            })
            .unwrap_or_else(|| {
                if systems.is_empty() {
                    0
                } else {
                    previous_index.min(systems.len() - 1)
                }
            });
    }

    pub fn advance_due(&mut self, now: Duration) -> Result<(), ApplicationError> {
        if !matches!(self.modal, Some(Modal::Batch(_))) {
            return Ok(());
        }
        let Some(Modal::Batch(mut batch)) = self.modal.take() else {
            unreachable!("modal kind checked above")
        };
        if batch.status == BatchStatus::Running && now >= batch.next_due {
            self.step_batch(&mut batch, now, false)?;
        }
        self.modal = Some(Modal::Batch(batch));
        Ok(())
    }

    fn step_batch(
        &mut self,
        batch: &mut TickBatch,
        now: Duration,
        paused_step: bool,
    ) -> Result<(), ApplicationError> {
        let outcome = self
            .session
            .as_mut()
            .expect("batch requires session")
            .dispatch(SessionIntent::AdvanceOneTick)?;
        match outcome {
            SessionOutcome::Tick(step) => {
                self.replace_playing_view(step.view.clone());
                batch.history.push(step);
                batch.selected_history = batch.history.len() - 1;
                if batch.history.len() >= usize::from(batch.requested) {
                    batch.status = BatchStatus::Complete;
                } else if !paused_step {
                    batch.next_due = now.saturating_add(batch.period());
                }
            }
            SessionOutcome::Rejected(outcome) => {
                self.notice = Some(outcome.clone());
                batch.rejection = Some(outcome);
                batch.status = BatchStatus::Rejected;
            }
            _ => {}
        }
        Ok(())
    }

    #[must_use]
    pub fn next_wake(&self, now: Duration) -> Duration {
        if let Some(Modal::Batch(batch)) = &self.modal
            && batch.status == BatchStatus::Running
        {
            return batch.next_due.saturating_sub(now);
        }
        Duration::from_millis(100)
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.terminal_size = (width, height);
        let undersized = width < MIN_WIDTH || height < MIN_HEIGHT;
        if undersized
            && !self.undersized
            && let Some(Modal::Batch(batch)) = &mut self.modal
            && matches!(batch.status, BatchStatus::Running | BatchStatus::Paused)
        {
            batch.status = BatchStatus::Stopped;
        }
        self.undersized = undersized;
    }

    #[must_use]
    pub fn startup_failure_text(&self) -> Option<String> {
        match self.startup_view.as_ref()?.failure.as_ref()? {
            StartupFailure::Content(values) => Some(values.first().map_or_else(
                || "Content could not be loaded".into(),
                |value| format!("{}: {}", value.field, value.message),
            )),
            StartupFailure::Generation(message) | StartupFailure::InvalidStart(message) => {
                Some(message.clone())
            }
        }
    }
}

fn offset(index: usize, delta: isize, len: usize) -> usize {
    if delta < 0 {
        index.checked_sub(delta.unsigned_abs()).unwrap_or(len - 1)
    } else {
        (index + delta as usize) % len
    }
}
