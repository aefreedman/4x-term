//! Ratatui input/render adapter. This crate never accesses the ECS world.

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::StreamExt;
use game_app::{AppHandle, AppRequest, ApplicationView, RunState};
use ratatui::Frame;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap};
use std::io::stdout;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Focus {
    Systems,
    Market,
    Trade,
    Events,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::Systems => Self::Market,
            Self::Market => Self::Trade,
            Self::Trade => Self::Events,
            Self::Events => Self::Systems,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UiState {
    pub focus: Focus,
    pub system_index: usize,
    pub market_index: usize,
    pub event_scroll: u16,
    pub trade_quantity: u32,
    pub quantity_input: Option<String>,
    pub help_visible: bool,
    pub message: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            focus: Focus::Systems,
            system_index: 0,
            market_index: 0,
            event_scroll: 0,
            trade_quantity: 1,
            quantity_input: None,
            help_visible: false,
            message: String::new(),
        }
    }
}

trait TerminalOps {
    fn enable_raw(&mut self) -> Result<()>;
    fn enter_alternate(&mut self) -> Result<()>;
    fn hide_cursor(&mut self) -> Result<()>;
    fn show_cursor(&mut self) -> Result<()>;
    fn leave_alternate(&mut self) -> Result<()>;
    fn disable_raw(&mut self) -> Result<()>;
}

struct RealTerminal;

impl TerminalOps for RealTerminal {
    fn enable_raw(&mut self) -> Result<()> {
        enable_raw_mode().map_err(Into::into)
    }
    fn enter_alternate(&mut self) -> Result<()> {
        execute!(stdout(), EnterAlternateScreen).map_err(Into::into)
    }
    fn hide_cursor(&mut self) -> Result<()> {
        execute!(stdout(), crossterm::cursor::Hide).map_err(Into::into)
    }
    fn show_cursor(&mut self) -> Result<()> {
        execute!(stdout(), crossterm::cursor::Show).map_err(Into::into)
    }
    fn leave_alternate(&mut self) -> Result<()> {
        execute!(stdout(), LeaveAlternateScreen).map_err(Into::into)
    }
    fn disable_raw(&mut self) -> Result<()> {
        disable_raw_mode().map_err(Into::into)
    }
}

struct TerminalGuard<O: TerminalOps> {
    ops: O,
    raw: bool,
    alternate: bool,
    cursor_hidden: bool,
}

impl<O: TerminalOps> TerminalGuard<O> {
    fn enter(ops: O) -> Result<Self> {
        let mut guard = Self {
            ops,
            raw: false,
            alternate: false,
            cursor_hidden: false,
        };
        guard.ops.enable_raw()?;
        guard.raw = true;
        guard.ops.enter_alternate()?;
        guard.alternate = true;
        guard.ops.hide_cursor()?;
        guard.cursor_hidden = true;
        Ok(guard)
    }
}

impl<O: TerminalOps> Drop for TerminalGuard<O> {
    fn drop(&mut self) {
        if self.cursor_hidden {
            let _ = self.ops.show_cursor();
        }
        if self.alternate {
            let _ = self.ops.leave_alternate();
        }
        if self.raw {
            let _ = self.ops.disable_raw();
        }
    }
}

fn restore_terminal() {
    let mut terminal = RealTerminal;
    let _ = terminal.show_cursor();
    let _ = terminal.leave_alternate();
    let _ = terminal.disable_raw();
}

fn install_terminal_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous(info);
    }));
}

pub async fn run(mut app: AppHandle) -> Result<()> {
    install_terminal_panic_hook();
    let guard = TerminalGuard::enter(RealTerminal)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = ratatui::Terminal::new(backend)?;
    terminal.clear()?;
    let mut events = EventStream::new();
    let mut ui = UiState::default();
    let mut view = app.views.borrow().clone();
    terminal.draw(|frame| render(frame, &view, &ui))?;

    loop {
        tokio::select! {
            input = events.next() => {
                match input {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        if handle_key(key.code, &mut ui, &view, &app).await? { break; }
                        view = app.views.borrow().clone();
                        terminal.draw(|frame| render(frame, &view, &ui))?;
                    }
                    Some(Ok(Event::Resize(_, _))) => { terminal.draw(|frame| render(frame, &view, &ui))?; }
                    Some(Err(error)) => return Err(error.into()),
                    None => break,
                    _ => {}
                }
            }
            changed = app.views.changed() => {
                if changed.is_err() { break; }
                view = app.views.borrow_and_update().clone();
                clamp_selection(&mut ui, &view);
                terminal.draw(|frame| render(frame, &view, &ui))?;
            }
        }
    }
    drop(terminal);
    drop(guard);
    app.shutdown().await?;
    Ok(())
}

async fn handle_key(
    code: KeyCode,
    ui: &mut UiState,
    view: &ApplicationView,
    app: &AppHandle,
) -> Result<bool> {
    if let Some(input) = &mut ui.quantity_input {
        match code {
            KeyCode::Char(digit) if digit.is_ascii_digit() && input.len() < 9 => input.push(digit),
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Enter => {
                ui.trade_quantity = input.parse::<u32>().unwrap_or(1).max(1);
                ui.quantity_input = None;
            }
            KeyCode::Esc => ui.quantity_input = None,
            _ => {}
        }
        return Ok(false);
    }
    match code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Tab => ui.focus = ui.focus.next(),
        KeyCode::Char('?') => ui.help_visible = !ui.help_visible,
        KeyCode::Char('n') => ui.quantity_input = Some(String::new()),
        KeyCode::Up | KeyCode::Char('k') => move_selection(ui, view, -1),
        KeyCode::Down | KeyCode::Char('j') => move_selection(ui, view, 1),
        KeyCode::Char(' ') => {
            let state = if view.run_state == RunState::Paused {
                RunState::Running
            } else {
                RunState::Paused
            };
            app.request(AppRequest::SetRunState(state)).await?;
        }
        KeyCode::Char('s') => {
            app.request(AppRequest::Step).await?;
        }
        KeyCode::Char('r') => {
            app.request(AppRequest::SetTickRate(view.tick_rate.next()))
                .await?;
        }
        KeyCode::Char('b') if !view.market.is_empty() => {
            let good = view.market[ui.market_index].good_id.clone();
            if let Err(error) = app
                .request(AppRequest::Buy {
                    good,
                    quantity: ui.trade_quantity,
                })
                .await
            {
                ui.message = error.to_string();
            }
        }
        KeyCode::Char('x') if !view.market.is_empty() => {
            let good = view.market[ui.market_index].good_id.clone();
            if let Err(error) = app
                .request(AppRequest::Sell {
                    good,
                    quantity: ui.trade_quantity,
                })
                .await
            {
                ui.message = error.to_string();
            }
        }
        KeyCode::Enter if !view.systems.is_empty() => {
            let destination = view.systems[ui.system_index].id.clone();
            if let Err(error) = app.request(AppRequest::BeginTravel { destination }).await {
                ui.message = error.to_string();
            }
        }
        _ => {}
    }
    if matches!(ui.focus, Focus::Systems) && !view.systems.is_empty() {
        app.request(AppRequest::SelectSystem(
            view.systems[ui.system_index].id.clone(),
        ))
        .await?;
    }
    Ok(false)
}

fn move_selection(ui: &mut UiState, view: &ApplicationView, delta: isize) {
    match ui.focus {
        Focus::Systems => ui.system_index = shifted(ui.system_index, view.systems.len(), delta),
        Focus::Market | Focus::Trade => {
            ui.market_index = shifted(ui.market_index, view.market.len(), delta)
        }
        Focus::Events => ui.event_scroll = ui.event_scroll.saturating_add_signed(delta as i16),
    }
}

fn shifted(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        0
    } else {
        current.saturating_add_signed(delta).min(len - 1)
    }
}

fn clamp_selection(ui: &mut UiState, view: &ApplicationView) {
    ui.system_index = ui.system_index.min(view.systems.len().saturating_sub(1));
    ui.market_index = ui.market_index.min(view.market.len().saturating_sub(1));
}

pub fn render(frame: &mut Frame<'_>, view: &ApplicationView, ui: &UiState) {
    let area = frame.area();
    if area.width < 70 || area.height < 24 {
        frame.render_widget(
            Paragraph::new("Terminal too small\nMinimum recommended size: 70x24\nPress q to quit")
                .block(Block::bordered().title("4x-term")),
            area,
        );
        return;
    }
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(12),
            Constraint::Length(8),
            Constraint::Length(3),
        ])
        .split(area);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(vertical[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(6)])
        .split(top[1]);
    render_systems(frame, top[0], view, ui);
    render_details(frame, right[0], view, ui);
    render_market(frame, right[1], view, ui);
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(vertical[1]);
    render_player(frame, bottom[0], view, ui);
    render_events(frame, bottom[1], view, ui);
    let controls = format!(
        "{} tick {} | Qty {} (n edit) | Space pause/run | s step | r rate | b buy | x sell | Enter travel | ? help | Tab focus | q quit {}",
        if view.run_state == RunState::Paused {
            "PAUSED"
        } else {
            "RUNNING"
        },
        view.tick,
        ui.trade_quantity,
        if ui.message.is_empty() {
            String::new()
        } else {
            format!("| {}", ui.message)
        }
    );
    frame.render_widget(
        Paragraph::new(controls)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title(format!("Controls [{}]", view.tick_rate.label()))),
        vertical[2],
    );
    if let Some(input) = &ui.quantity_input {
        let popup = centered_rect(38, 5, area);
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new(format!("Quantity: {input}_\nEnter confirm · Esc cancel"))
                .block(Block::bordered().title("Trade Quantity")),
            popup,
        );
    } else if ui.help_visible {
        let popup = centered_rect(62, 12, area);
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new("Tab: focus panes\n↑/↓ or j/k: selection\nSpace: pause/run   s: single step   r: rate\nn: enter trade quantity   b: buy   x: sell\nEnter: travel to selected system\n?: close help   q: quit")
                .wrap(Wrap { trim: true })
                .block(Block::bordered().title("Help")),
            popup,
        );
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width.min(area.width),
        height.min(area.height),
    )
}

fn focused(ui: &UiState, focus: Focus) -> Style {
    if ui.focus == focus {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    }
}

fn render_systems(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let items = view
        .systems
        .iter()
        .enumerate()
        .map(|(index, system)| {
            let style = if index == ui.system_index {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(system.name.clone()).style(style)
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Systems")
                .border_style(focused(ui, Focus::Systems)),
        ),
        area,
    );
}

fn render_details(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, _ui: &UiState) {
    let system = view
        .systems
        .iter()
        .find(|system| system.id == view.selected_system);
    let mut lines = system.map_or_else(Vec::new, |system| {
        vec![
            Line::from(format!(
                "{} ({:.1}, {:.1}, {:.1})",
                system.name, system.coordinates.0, system.coordinates.1, system.coordinates.2
            )),
            Line::from(format!(
                "Direct: {}",
                system
                    .connections
                    .iter()
                    .map(|connection| format!(
                        "{} ({:.1} · {}t)",
                        connection.system_name, connection.distance, connection.travel_ticks
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        ]
    });
    if let Some(route) = &view.selected_route {
        let mode = if route.current_leg.is_some() {
            format!("Traveling to {}", route.destination_name)
        } else {
            format!("Proposed route to {}", route.destination_name)
        };
        lines.push(Line::from(mode));
        lines.push(Line::from(format!(
            "{} jumps · {:.1} distance · {} ticks{}",
            route.legs.len(),
            route.total_distance,
            route.total_ticks,
            route
                .remaining_ticks
                .map_or(String::new(), |ticks| format!(" · {ticks} remaining"))
        )));
        if let Some(index) = route.current_leg {
            if let Some(leg) = route.legs.get(index) {
                lines.push(Line::from(format!(
                    "Leg {}/{}: {} → {}",
                    index + 1,
                    route.legs.len(),
                    leg.from_name,
                    leg.to_name
                )));
            }
        } else {
            lines.push(Line::from(format_route_chain(route)));
        }
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("System / Route")),
        area,
    );
}

fn format_route_chain(route: &game_app::RouteView) -> String {
    let mut names = route
        .legs
        .first()
        .map(|leg| vec![leg.from_name.clone()])
        .unwrap_or_default();
    names.extend(route.legs.iter().map(|leg| leg.to_name.clone()));
    if names.len() > 6 {
        format!(
            "{} → {} → … → {} → {}",
            names[0],
            names[1],
            names[names.len() - 2],
            names[names.len() - 1]
        )
    } else {
        names.join(" → ")
    }
}

fn render_market(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let rows = view.market.iter().enumerate().map(|(index, row)| {
        Row::new(vec![
            Cell::from(row.name.clone()),
            Cell::from(row.inventory.to_string()),
            Cell::from(row.target.to_string()),
            Cell::from(format!("¤{}", row.buy_quote.0)),
            Cell::from(format!("¤{}", row.sell_quote.0)),
        ])
        .style(if index == ui.market_index {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        })
    });
    let header = Row::new(["Good", "Stock", "Target", "Market buys", "Market sells"])
        .style(Style::default().add_modifier(Modifier::BOLD));
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(36),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Market")
            .border_style(focused(ui, Focus::Market)),
    );
    frame.render_widget(table, area);
}

fn render_player(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let p = &view.player;
    let cargo = if p.cargo.is_empty() {
        "empty".into()
    } else {
        p.cargo
            .iter()
            .map(|item| format!("{} x{}", item.good_name, item.quantity))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let lines = vec![
        Line::from(vec![
            Span::raw(format!(
                "Location: {}{}  ",
                p.location_name,
                if p.traveling { " (traveling)" } else { "" }
            )),
            Span::raw(format!(
                "Funds: ¤{}  Net: ¤{}  Rank: #{}",
                p.currency.0, p.net_worth.0, p.net_worth_rank
            )),
        ]),
        Line::from(format!(
            "Cargo {}/{} (¤{}): {}",
            p.cargo_used, p.cargo_capacity, p.cargo_value.0, cargo
        )),
        Line::from(format!(
            "Sales ¤{} | Profit ¤{} | Trades {} | Economy share {:.1}%",
            p.sales_revenue, p.realized_profit, p.transactions, p.net_worth_share_percent
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Player / Trade")
                .border_style(focused(ui, Focus::Trade)),
        ),
        area,
    );
}

fn render_events(frame: &mut Frame<'_>, area: Rect, view: &ApplicationView, ui: &UiState) {
    let visible = area.height.saturating_sub(2) as usize;
    let start = view
        .events
        .len()
        .saturating_sub(visible + usize::from(ui.event_scroll));
    let end = (start + visible).min(view.events.len());
    let text = view.events[start..end]
        .iter()
        .cloned()
        .map(Line::from)
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Events")
                .border_style(focused(ui, Focus::Events)),
        ),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_app::{
        CargoItemView, ConnectionView, MarketRow, PlayerStatusView, RouteLegView, RouteView,
        SystemListItem, TickRate,
    };
    use game_core::{
        ContentId, GameDefinition, GameSession, GoodCategory, GoodDefinition, Money, Position3,
        SystemDefinition, TraderDefinition,
    };
    use ratatui::backend::TestBackend;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    struct FakeOps {
        calls: Rc<RefCell<Vec<&'static str>>>,
        fail: Option<&'static str>,
    }

    impl FakeOps {
        fn call(&self, name: &'static str) -> Result<()> {
            self.calls.borrow_mut().push(name);
            if self.fail == Some(name) {
                anyhow::bail!("forced {name} failure");
            }
            Ok(())
        }
    }

    impl TerminalOps for FakeOps {
        fn enable_raw(&mut self) -> Result<()> {
            self.call("enable_raw")
        }
        fn enter_alternate(&mut self) -> Result<()> {
            self.call("enter_alternate")
        }
        fn hide_cursor(&mut self) -> Result<()> {
            self.call("hide_cursor")
        }
        fn show_cursor(&mut self) -> Result<()> {
            self.call("show_cursor")
        }
        fn leave_alternate(&mut self) -> Result<()> {
            self.call("leave_alternate")
        }
        fn disable_raw(&mut self) -> Result<()> {
            self.call("disable_raw")
        }
    }

    fn id(value: &str) -> ContentId {
        ContentId::new(value).unwrap()
    }

    fn test_session() -> GameSession {
        let ore = id("core:ore");
        let systems = (0..2)
            .map(|index| SystemDefinition {
                id: id(&format!("core:s{index}")),
                name: format!("System {index}"),
                position: Position3 {
                    x: f64::from(index),
                    y: 0.0,
                    z: 0.0,
                },
                inventory: BTreeMap::from([(ore.clone(), 10)]),
                targets: BTreeMap::from([(ore.clone(), 10)]),
                currency: Money(1_000),
                recipes: vec![],
                sources: vec![],
            })
            .collect();
        GameSession::new(GameDefinition {
            goods: vec![GoodDefinition {
                id: ore,
                name: "Ore".into(),
                category: GoodCategory::Raw,
                base_price: Money(10),
            }],
            recipes: vec![],
            systems,
            traders: vec![TraderDefinition {
                id: id("core:player"),
                name: "Player".into(),
                system: id("core:s0"),
                currency: Money(100),
                cargo_capacity: 10,
                speed: 1.0,
                player: true,
            }],
        })
        .unwrap()
    }

    fn test_view() -> ApplicationView {
        ApplicationView {
            tick: 0,
            run_state: RunState::Paused,
            tick_rate: TickRate::Normal,
            systems: vec![SystemListItem {
                id: id("core:s0"),
                name: "Aster".into(),
                coordinates: (0.0, 0.0, 0.0),
                connections: vec![ConnectionView {
                    system_id: id("core:s1"),
                    system_name: "Brasshaven".into(),
                    distance: 3.5,
                    travel_ticks: 4,
                }],
            }],
            selected_system: id("core:s0"),
            selected_route: None,
            market: vec![MarketRow {
                good_id: id("core:ore"),
                name: "Ore".into(),
                inventory: 10,
                target: 10,
                buy_quote: Money(9),
                sell_quote: Money(11),
            }],
            player: PlayerStatusView {
                location: id("core:s0"),
                location_name: "Aster Reach".into(),
                currency: Money(100),
                cargo: vec![CargoItemView {
                    good_id: id("core:ore"),
                    good_name: "Ferrite Ore".into(),
                    quantity: 2,
                }],
                cargo_used: 2,
                cargo_capacity: 10,
                cargo_value: Money(0),
                net_worth: Money(100),
                purchase_cost: 0,
                sales_revenue: 0,
                realized_profit: 0,
                units_moved: 0,
                transactions: 0,
                net_worth_rank: 1,
                net_worth_share_percent: 100.0,
                sales_share_percent: 0.0,
                traveling: false,
            },
            events: vec![],
        }
    }

    #[test]
    fn partial_terminal_setup_is_cleaned_up() {
        for (failure, expected) in [
            (
                "enter_alternate",
                vec!["enable_raw", "enter_alternate", "disable_raw"],
            ),
            (
                "hide_cursor",
                vec![
                    "enable_raw",
                    "enter_alternate",
                    "hide_cursor",
                    "leave_alternate",
                    "disable_raw",
                ],
            ),
        ] {
            let calls = Rc::new(RefCell::new(Vec::new()));
            let result = TerminalGuard::enter(FakeOps {
                calls: Rc::clone(&calls),
                fail: Some(failure),
            });
            assert!(result.is_err());
            assert_eq!(*calls.borrow(), expected);
        }
    }

    #[test]
    fn complete_terminal_setup_is_cleaned_up_in_reverse_order() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let guard = TerminalGuard::enter(FakeOps {
            calls: Rc::clone(&calls),
            fail: None,
        })
        .unwrap();
        drop(guard);
        assert_eq!(
            *calls.borrow(),
            vec![
                "enable_raw",
                "enter_alternate",
                "hide_cursor",
                "show_cursor",
                "leave_alternate",
                "disable_raw"
            ]
        );
    }

    #[test]
    fn renders_normal_constrained_and_edge_case_views() {
        let base = test_view();
        let mut edge = base.clone();
        edge.systems[0].name =
            "A very long frontier system name that must be clipped safely".into();
        edge.player.currency = Money(i64::MAX);
        edge.player.net_worth = Money(i64::MAX);
        edge.player.traveling = true;
        edge.selected_route = Some(RouteView {
            destination_id: id("core:s1"),
            destination_name: "Brasshaven".into(),
            legs: vec![RouteLegView {
                from_id: id("core:s0"),
                from_name: "Aster Reach".into(),
                to_id: id("core:s1"),
                to_name: "Brasshaven".into(),
                distance: 42.5,
                travel_ticks: 8,
            }],
            current_leg: Some(0),
            total_distance: 42.5,
            total_ticks: 8,
            remaining_ticks: Some(7),
        });
        edge.events = vec!["Rejected: insufficient cargo capacity".into(); 20];

        let help = UiState {
            help_visible: true,
            ..UiState::default()
        };
        let quantity = UiState {
            quantity_input: Some("123".into()),
            ..UiState::default()
        };
        for (width, height, view, ui) in [
            (100, 35, &base, UiState::default()),
            (70, 24, &edge, UiState::default()),
            (100, 35, &edge, help),
            (100, 35, &edge, quantity),
            (40, 10, &base, UiState::default()),
            (100, 12, &base, UiState::default()),
        ] {
            let backend = TestBackend::new(width, height);
            let mut terminal = ratatui::Terminal::new(backend).unwrap();
            terminal.draw(|frame| render(frame, view, &ui)).unwrap();
            let rendered = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            if width < 70 || height < 24 {
                assert!(rendered.contains("Terminal too small"));
            } else {
                for label in [
                    "Systems",
                    "System / Route",
                    "Market",
                    "Player / Trade",
                    "Events",
                    "Controls",
                ] {
                    assert!(rendered.contains(label), "missing {label}");
                }
                assert!(
                    !rendered.contains("core:"),
                    "internal content IDs leaked into the rendered interface"
                );
                if ui.help_visible {
                    assert!(rendered.contains("Help"));
                }
                if ui.quantity_input.is_some() {
                    assert!(rendered.contains("Trade Quantity"));
                }
            }
        }
    }

    #[tokio::test]
    async fn required_keys_map_to_local_state_and_application_requests() {
        let app = game_app::spawn(test_session());
        let mut ui = UiState::default();
        let mut view = app.views.borrow().clone();

        handle_key(KeyCode::Char('s'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.tick, 1);

        handle_key(KeyCode::Char('r'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.tick_rate, TickRate::Fast);
        handle_key(KeyCode::Char(' '), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.run_state, RunState::Running);
        handle_key(KeyCode::Char(' '), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.run_state, RunState::Paused);

        handle_key(KeyCode::Char('n'), &mut ui, &view, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Char('2'), &mut ui, &view, &app)
            .await
            .unwrap();
        handle_key(KeyCode::Enter, &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.trade_quantity, 2);
        handle_key(KeyCode::Char('b'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.player.cargo_used, 2);
        handle_key(KeyCode::Char('x'), &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.player.cargo_used, 0);

        handle_key(KeyCode::Char('?'), &mut ui, &view, &app)
            .await
            .unwrap();
        assert!(ui.help_visible);
        handle_key(KeyCode::Down, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert_eq!(view.selected_system, id("core:s1"));
        assert_eq!(view.tick, 1, "UI navigation must not advance simulation");
        handle_key(KeyCode::Enter, &mut ui, &view, &app)
            .await
            .unwrap();
        view = app.views.borrow().clone();
        assert!(view.player.traveling);

        handle_key(KeyCode::Tab, &mut ui, &view, &app)
            .await
            .unwrap();
        assert_eq!(ui.focus, Focus::Market);
        handle_key(KeyCode::Up, &mut ui, &view, &app).await.unwrap();
        assert!(
            handle_key(KeyCode::Char('q'), &mut ui, &view, &app)
                .await
                .unwrap()
        );
        app.shutdown().await.unwrap();
    }
}
