use std::collections::VecDeque;
use std::io::{self, Stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Wrap},
};

// ── Agent status ───────────────────────────────────────────────────────

/// Lifecycle status of a single swarm agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatus {
    Pending,
    Running,
    Success,
    Error(String),
}

impl AgentStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Pending => "⏳ Pending",
            Self::Running => "▶ Running",
            Self::Success => "✔ Success",
            Self::Error(_) => "✖ Error",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Pending => Color::DarkGray,
            Self::Running => Color::Cyan,
            Self::Success => Color::Green,
            Self::Error(_) => Color::Red,
        }
    }
}

// ── Agent state ────────────────────────────────────────────────────────

const MAX_LOG_LINES: usize = 256;

/// State for one concurrent agent in the swarm.
#[derive(Debug, Clone)]
pub struct AgentState {
    pub url: String,
    pub goal: String,
    pub status: AgentStatus,
    /// Bounded ring-buffer of recent SSE log messages.
    pub logs: VecDeque<String>,
    /// Heuristic progress percentage (0–100).
    pub progress: u16,
}

impl AgentState {
    pub fn new(url: impl Into<String>, goal: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            goal: goal.into(),
            status: AgentStatus::Pending,
            logs: VecDeque::with_capacity(MAX_LOG_LINES),
            progress: 0,
        }
    }

    /// Push a log line, evicting the oldest entry if the buffer is full.
    pub fn push_log(&mut self, line: impl Into<String>) {
        if self.logs.len() == MAX_LOG_LINES {
            self.logs.pop_front();
        }
        self.logs.push_back(line.into());
    }
}

// ── App (top-level UI state) ───────────────────────────────────────────

/// Root application state for the Krill Swarm TUI.
pub struct App {
    pub agents: Vec<AgentState>,
    /// Index of the currently highlighted agent pane.
    pub selected: usize,
    /// Set to `true` when the user requests quit (q / Esc).
    pub should_quit: bool,
}

impl App {
    pub fn new(agents: Vec<AgentState>) -> Self {
        Self {
            agents,
            selected: 0,
            should_quit: false,
        }
    }

    /// Process a single terminal event (non-blocking when polled).
    pub fn handle_event(&mut self) -> io::Result<()> {
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                    KeyCode::Tab | KeyCode::Right | KeyCode::Down => {
                        if !self.agents.is_empty() {
                            self.selected = (self.selected + 1) % self.agents.len();
                        }
                    }
                    KeyCode::BackTab | KeyCode::Left | KeyCode::Up => {
                        if !self.agents.is_empty() {
                            self.selected = self
                                .selected
                                .checked_sub(1)
                                .unwrap_or(self.agents.len() - 1);
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

// ── Terminal setup / teardown ──────────────────────────────────────────

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Enter raw mode, switch to the alternate screen, and return a Terminal.
pub fn setup_terminal() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to its original state.
pub fn restore_terminal(terminal: &mut Tui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

// ── Drawing ────────────────────────────────────────────────────────────

/// Render the full swarm dashboard into the provided frame.
pub fn draw_ui(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // 3-pane vertical layout: Header | Grid | Footer
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    // ── Header ─────────────────────────────────────────────────────────
    let header = Paragraph::new(Line::from(vec![
        Span::styled("krill 🦐", Style::default().fg(Color::Cyan).bold()),
        Span::raw(" │ "),
        Span::styled(
            "Agentic Swarm Orchestrator",
            Style::default().fg(Color::White).bold(),
        ),
    ]));
    frame.render_widget(header, chunks[0]);

    // ── Footer / Status Bar ────────────────────────────────────────────
    let running = app
        .agents
        .iter()
        .filter(|a| matches!(a.status, AgentStatus::Running))
        .count();
    let done = app
        .agents
        .iter()
        .filter(|a| matches!(a.status, AgentStatus::Success))
        .count();
    let failed = app
        .agents
        .iter()
        .filter(|a| matches!(a.status, AgentStatus::Error(_)))
        .count();

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            " [q] Quit │ [Tab] Cycle │ [Ctrl+C] Kill ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("  "),
        Span::styled(
            format!(
                "agents: {} │ ▶ {} │ ✔ {} │ ✖ {} ",
                app.agents.len(),
                running,
                done,
                failed,
            ),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .alignment(Alignment::Left);
    frame.render_widget(footer, chunks[2]);

    // ── Master-Detail layout ──────────────────────────────────────────
    let middle = chunks[1];

    if app.agents.is_empty() {
        let msg = Paragraph::new("No agents configured.")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" krill swarm "),
            );
        frame.render_widget(msg, middle);
        return;
    }

    let panels = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(70),
    ])
    .split(middle);

    // ── Left: Swarm Roster + Telemetry ───────────────────────────────────
    let left_chunks = Layout::vertical([
        Constraint::Percentage(70),
        Constraint::Percentage(30),
    ])
    .split(panels[0]);

    render_roster(frame, left_chunks[0], app);

    let selected = &app.agents[app.selected];
    render_telemetry(frame, left_chunks[1], selected);

    // ── Right: Telemetry Detail ────────────────────────────────────────
    render_detail(frame, panels[1], selected);
}

/// Render the left-hand agent roster list.
fn render_roster(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .agents
        .iter()
        .enumerate()
        .map(|(i, agent)| {
            let icon = match agent.status {
                AgentStatus::Pending => "⏳",
                AgentStatus::Running => "▶",
                AgentStatus::Success => "✔",
                AgentStatus::Error(_) => "✖",
            };
            let label = truncate(&agent.url, (area.width as usize).saturating_sub(6));
            let style = if i == app.selected {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default().fg(agent.status.color())
            };
            ListItem::new(Line::from(format!(" {} {}", icon, label))).style(style)
        })
        .collect();

    let roster = List::new(items)
        .highlight_symbol("▸ ")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Swarm Roster "),
        );

    let mut list_state = ListState::default().with_selected(Some(app.selected));
    frame.render_stateful_widget(roster, area, &mut list_state);
}

/// Render the bottom-left target telemetry panel.
fn render_telemetry(frame: &mut Frame, area: Rect, agent: &AgentState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Target Telemetry ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_url_len = (inner.width as usize).saturating_sub(10);
    let state_label = agent.status.label();

    let text = Text::from(vec![
        Line::from(vec![
            Span::styled("Target: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                truncate(&agent.url, max_url_len),
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("State:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(state_label, Style::default().fg(agent.status.color()).bold()),
        ]),
        Line::from(vec![
            Span::styled("Logs:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", agent.logs.len()),
                Style::default().fg(Color::Cyan).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Done:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}%", agent.progress),
                Style::default().fg(Color::Yellow).bold(),
            ),
        ]),
    ]);

    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

/// Render the right-hand detail pane (logs + progress gauge).
fn render_detail(frame: &mut Frame, area: Rect, agent: &AgentState) {
    let detail_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(agent.status.color()))
        .title(format!(" {} — {} ", truncate(&agent.url, 50), agent.status.label()));

    let inner = detail_block.inner(area);
    frame.render_widget(detail_block, area);

    // Split inner: logs on top, gauge at bottom
    let detail_chunks = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .split(inner);

    // Logs
    let visible_lines = detail_chunks[0].height as usize;
    let start = agent.logs.len().saturating_sub(visible_lines);
    let log_text: String = agent
        .logs
        .iter()
        .skip(start)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");

    let log_paragraph = Paragraph::new(log_text)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });
    frame.render_widget(log_paragraph, detail_chunks[0]);

    // Progress gauge
    let gauge_color = match agent.status {
        AgentStatus::Success => Color::Green,
        AgentStatus::Error(_) => Color::Red,
        _ => Color::Yellow,
    };

    let gauge_label = format!("{}%", agent.progress);

    let gauge = Gauge::default()
        .percent(agent.progress)
        .gauge_style(Style::default().fg(gauge_color).bg(Color::DarkGray))
        .label(gauge_label)
        .use_unicode(true);
    frame.render_widget(gauge, detail_chunks[1]);
}

/// Truncate a string to `max_len` characters, appending "…" if shortened.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}
