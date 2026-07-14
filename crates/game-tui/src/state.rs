//! Pure, TUI-local interaction state.

use game_app::{ContentId, Energy, PopulationTrend, PresentationEvent};
use std::cmp::Ordering;

/// The player's current top-level activity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Activity {
    #[default]
    Systems,
    Trade,
    Governance,
    Intelligence,
}

/// The layout supported by the current terminal cell grid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutClass {
    Unsupported,
    Compact,
    Regular,
}

/// Classifies a terminal using cell dimensions, never pixel dimensions.
pub const fn classify_layout(width: u16, height: u16) -> LayoutClass {
    if width < 80 || height < 30 {
        LayoutClass::Unsupported
    } else if width >= 160 && height >= 45 {
        LayoutClass::Regular
    } else {
        LayoutClass::Compact
    }
}

/// A sortable, presentation-only system summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemOrderItem {
    pub id: ContentId,
    pub name: String,
    pub risk: u8,
    pub runway_ticks: u32,
    pub energy_fill_percent: u32,
    pub population: u64,
    pub population_trend: PopulationTrend,
    pub route_ticks: Option<u32>,
    pub energy_stock: Energy,
}

/// Columns available for system ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemSortKey {
    Name,
    Risk,
    Runway,
    EnergyFillPercent,
    Population,
    RouteTicks,
}

impl SystemSortKey {
    pub const ALL: [Self; 6] = [
        Self::Name,
        Self::Risk,
        Self::Runway,
        Self::EnergyFillPercent,
        Self::Population,
        Self::RouteTicks,
    ];

    /// Advances to the next visible Systems table column.
    pub const fn next(self) -> Self {
        match self {
            Self::Name => Self::Risk,
            Self::Risk => Self::Runway,
            Self::Runway => Self::EnergyFillPercent,
            Self::EnergyFillPercent => Self::Population,
            Self::Population => Self::RouteTicks,
            Self::RouteTicks => Self::Name,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Risk => "Risk",
            Self::Runway => "Runway",
            Self::EnergyFillPercent => "Energy",
            Self::Population => "Population",
            Self::RouteTicks => "Route",
        }
    }
}

/// Ordering direction for a [`SystemSortKey`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

impl SortDirection {
    fn apply(self, ordering: Ordering) -> Ordering {
        match self {
            Self::Ascending => ordering,
            Self::Descending => ordering.reverse(),
        }
    }

    pub const fn toggled(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Ascending => "↑",
            Self::Descending => "↓",
        }
    }
}

/// Returns a deterministic order without mutating the source rows.
///
/// `ContentId` is always the final tie-breaker so live snapshot ordering cannot
/// affect the presentation order. Route-less systems sort after routed systems.
pub fn order_systems(
    systems: &[SystemOrderItem],
    key: SystemSortKey,
    direction: SortDirection,
) -> Vec<SystemOrderItem> {
    let mut ordered = systems.to_vec();
    ordered.sort_by(|left, right| {
        let primary = match key {
            SystemSortKey::Name => left.name.cmp(&right.name),
            SystemSortKey::Risk => left.risk.cmp(&right.risk),
            SystemSortKey::Runway => left.runway_ticks.cmp(&right.runway_ticks),
            SystemSortKey::EnergyFillPercent => {
                left.energy_fill_percent.cmp(&right.energy_fill_percent)
            }
            SystemSortKey::Population => left.population.cmp(&right.population),
            SystemSortKey::RouteTicks => {
                route_ticks_cmp(left.route_ticks, right.route_ticks, direction)
            }
        };
        let primary = if key == SystemSortKey::RouteTicks {
            primary
        } else {
            direction.apply(primary)
        };
        primary.then_with(|| left.id.cmp(&right.id))
    });
    ordered
}

fn route_ticks_cmp(left: Option<u32>, right: Option<u32>, direction: SortDirection) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => direction.apply(left.cmp(&right)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// The input owner currently above the activity root.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InputLayer {
    #[default]
    Root,
    Quantity,
    Help,
    Detail,
}

/// All state local to the terminal adapter.
#[derive(Clone, Debug)]
pub struct UiState {
    pub activity: Activity,
    pub input_layer: InputLayer,
    pub selected_system: Option<ContentId>,
    pub system_sort: SystemSortKey,
    pub sort_direction: SortDirection,
    pub system_index: usize,
    pub market_index: usize,
    pub governance_index: usize,
    pub investment_index: usize,
    pub route_proposal: Option<ContentId>,
    pub governance_inspection: Option<ContentId>,
    pub event_anchor: Option<u64>,
    pub event_follow_tail: bool,
    pub newer_events_available: bool,
    pub event_scroll: u16,
    pub trade_quantity: u32,
    pub quantity_input: Option<String>,
    pub help_visible: bool,
    pub message: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            activity: Activity::Systems,
            input_layer: InputLayer::Root,
            selected_system: None,
            system_sort: SystemSortKey::Name,
            sort_direction: SortDirection::Ascending,
            system_index: 0,
            market_index: 0,
            governance_index: 0,
            investment_index: 0,
            route_proposal: None,
            governance_inspection: None,
            event_anchor: None,
            event_follow_tail: true,
            newer_events_available: false,
            event_scroll: 0,
            trade_quantity: 1,
            quantity_input: None,
            help_visible: false,
            message: String::new(),
        }
    }
}

impl UiState {
    /// Preserves the selected stable ID through reordering, or picks the first
    /// remaining row when that ID disappeared.
    pub fn reconcile_system_selection(&mut self, systems: &[SystemOrderItem]) {
        if self
            .selected_system
            .as_ref()
            .is_some_and(|selected| systems.iter().any(|system| &system.id == selected))
        {
            return;
        }
        self.selected_system = systems.first().map(|system| system.id.clone());
    }

    /// Finds the visible row for the stable selected ID.
    pub fn system_row(&self, systems: &[SystemOrderItem]) -> Option<usize> {
        self.selected_system
            .as_ref()
            .and_then(|selected| systems.iter().position(|system| &system.id == selected))
    }

    /// Reconciles the Intelligence cursor against a replaceable bounded event
    /// history. Tail-follow advances only while the cursor is already newest;
    /// otherwise the stable presentation sequence remains the anchor.
    pub fn reconcile_events(&mut self, events: &[PresentationEvent]) {
        let Some(first) = events.first() else {
            self.event_anchor = None;
            self.event_follow_tail = true;
            self.newer_events_available = false;
            self.event_scroll = 0;
            return;
        };
        let last = events.last().expect("non-empty event history");
        if self.event_follow_tail || self.event_anchor.is_none() {
            self.event_anchor = Some(last.sequence);
            self.event_follow_tail = true;
            self.newer_events_available = false;
            self.event_scroll = 0;
            return;
        }

        let previous = self.event_anchor.expect("checked above");
        let anchor_index = events
            .iter()
            .position(|event| event.sequence == previous)
            .unwrap_or_else(|| {
                events
                    .iter()
                    .position(|event| event.sequence >= previous)
                    .unwrap_or(events.len() - 1)
            });
        let anchor = events[anchor_index].sequence.max(first.sequence);
        self.event_anchor = Some(anchor);
        self.event_follow_tail = anchor == last.sequence;
        self.newer_events_available = !self.event_follow_tail;
        self.event_scroll = u16::try_from(events.len() - 1 - anchor_index).unwrap_or(u16::MAX);
    }

    /// Moves the Intelligence cursor by event rows, clamping to the bounded
    /// history and resuming tail-follow when the newest row is reached.
    pub fn scroll_events(&mut self, events: &[PresentationEvent], delta: isize) {
        self.reconcile_events(events);
        if events.is_empty() {
            return;
        }
        let current = self
            .event_anchor
            .and_then(|anchor| events.iter().position(|event| event.sequence == anchor))
            .unwrap_or(events.len() - 1);
        let next = current
            .saturating_add_signed(delta)
            .min(events.len().saturating_sub(1));
        self.event_anchor = Some(events[next].sequence);
        self.event_follow_tail = next == events.len() - 1;
        self.newer_events_available = !self.event_follow_tail;
        self.event_scroll = u16::try_from(events.len() - 1 - next).unwrap_or(u16::MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> ContentId {
        ContentId::new(value).unwrap()
    }

    fn row(id_value: &str, route_ticks: Option<u32>) -> SystemOrderItem {
        SystemOrderItem {
            id: id(id_value),
            name: "Same".into(),
            risk: 1,
            runway_ticks: 1,
            energy_fill_percent: 1,
            population: 1,
            population_trend: PopulationTrend::Stable,
            route_ticks,
            energy_stock: Energy::ZERO,
        }
    }

    #[test]
    fn route_less_systems_remain_last_in_both_directions() {
        let systems = vec![row("core:none", None), row("core:near", Some(1))];
        for direction in [SortDirection::Ascending, SortDirection::Descending] {
            let ordered = order_systems(&systems, SystemSortKey::RouteTicks, direction);
            assert_eq!(ordered.last().unwrap().id, id("core:none"));
        }
    }
}
