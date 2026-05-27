use std::io;
use std::path::PathBuf;
use chrono::TimeZone;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        self,
        event::{self, Event, KeyCode, KeyEventKind},
    },
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, BorderType, List, ListItem, ListState, Paragraph},
    Terminal,
};
use crate::config::Config;
use crate::snapshots::{self, SnapshotInfo};

struct TerminalGuard;

impl TerminalGuard {
    fn create() -> Result<Self, Box<dyn std::error::Error>> {
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(
            stdout,
            crossterm::terminal::EnterAlternateScreen,
            crossterm::cursor::Hide
        )?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
    }
}

#[derive(PartialEq)]
enum Focus {
    Repos,
    Snapshots,
}

struct TuiState {
    repos: Vec<PathBuf>,
    repo_state: ListState,
    snapshots: Vec<SnapshotInfo>,
    snap_state: ListState,
    focus: Focus,
}

impl TuiState {
    fn new(repos: Vec<PathBuf>) -> Self {
        let mut repo_state = ListState::default();
        if !repos.is_empty() {
            repo_state.select(Some(0));
        }

        let mut state = Self {
            repos,
            repo_state,
            snapshots: Vec::new(),
            snap_state: ListState::default(),
            focus: Focus::Repos,
        };
        state.reload_snapshots();
        state
    }

    fn selected_repo_idx(&self) -> Option<usize> {
        self.repo_state.selected()
    }

    fn selected_snapshot_idx(&self) -> Option<usize> {
        self.snap_state.selected()
    }

    fn reload_snapshots(&mut self) {
        if let Some(idx) = self.selected_repo_idx() {
            if idx < self.repos.len() {
                let path = &self.repos[idx];
                self.snapshots = snapshots::list_snapshots(path).unwrap_or_default();
                if !self.snapshots.is_empty() {
                    self.snap_state.select(Some(0));
                } else {
                    self.snap_state.select(None);
                }
                return;
            }
        }
        self.snapshots = Vec::new();
        self.snap_state.select(None);
    }

    fn next_repo(&mut self) {
        if self.repos.is_empty() {
            return;
        }
        let i = match self.repo_state.selected() {
            Some(i) => {
                if i >= self.repos.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.repo_state.select(Some(i));
        self.reload_snapshots();
    }

    fn prev_repo(&mut self) {
        if self.repos.is_empty() {
            return;
        }
        let i = match self.repo_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.repos.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.repo_state.select(Some(i));
        self.reload_snapshots();
    }

    fn next_snapshot(&mut self) {
        if self.snapshots.is_empty() {
            return;
        }
        let i = match self.snap_state.selected() {
            Some(i) => {
                if i >= self.snapshots.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.snap_state.select(Some(i));
    }

    fn prev_snapshot(&mut self) {
        if self.snapshots.is_empty() {
            return;
        }
        let i = match self.snap_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.snapshots.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.snap_state.select(Some(i));
    }
}

pub fn run_interactive() -> Result<Option<(PathBuf, String)>, Box<dyn std::error::Error>> {
    let mut repos: Vec<PathBuf> = Config::load().git_repos().collect();
    repos.sort();

    if repos.is_empty() {
        println!("No watched repositories found. Add one with `dura watch <path>`.");
        return Ok(None);
    }

    let _guard = TerminalGuard::create()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut state = TuiState::new(repos);

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Header
                    Constraint::Min(10),   // Main panels
                    Constraint::Length(3), // Footer
                ])
                .split(f.area());

            // Header
            let header = Paragraph::new("Dura Interactive Snapshot Restore")
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title(" Application "),
                );
            f.render_widget(header, chunks[0]);

            // Main body
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(40), // Left: Repos
                    Constraint::Percentage(60), // Right: Snapshots
                ])
                .split(chunks[1]);

            // Repos list styling
            let repo_border_color = if state.focus == Focus::Repos {
                Color::Green
            } else {
                Color::DarkGray
            };
            let repo_items: Vec<ListItem> = state
                .repos
                .iter()
                .map(|p| {
                    let display_name = p.to_str().unwrap_or("Invalid path");
                    ListItem::new(display_name)
                })
                .collect();
            let repo_list = List::new(repo_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(repo_border_color))
                        .title(" Repositories "),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::Rgb(40, 40, 40))
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");
            f.render_stateful_widget(repo_list, main_chunks[0], &mut state.repo_state);

            // Snapshots list styling
            let snap_border_color = if state.focus == Focus::Snapshots {
                Color::Green
            } else {
                Color::DarkGray
            };
            let snap_items: Vec<ListItem> = if state.snapshots.is_empty() {
                vec![ListItem::new("No snapshots found for this repository")]
            } else {
                state
                    .snapshots
                    .iter()
                    .map(|s| {
                        let datetime = chrono::Local
                            .timestamp_opt(s.timestamp, 0)
                            .single()
                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "Unknown".to_string());
                        let display = format!(
                            "{} - {} ({} files)",
                            &s.commit_hash[..12],
                            datetime,
                            s.files_changed
                        );
                        ListItem::new(display)
                    })
                    .collect()
            };
            let snap_list = List::new(snap_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(snap_border_color))
                        .title(" Backups "),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::Rgb(40, 40, 40))
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");
            f.render_stateful_widget(snap_list, main_chunks[1], &mut state.snap_state);

            // Footer instructions
            let footer = Paragraph::new(
                " [←/→] Switch Panel  |  [↑/↓] Navigate  |  [Enter] Restore Snapshot  |  [Esc/q] Exit",
            )
            .style(Style::default().fg(Color::Gray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Shortcuts "),
            );
            f.render_widget(footer, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
                        KeyCode::Up => {
                            match state.focus {
                                Focus::Repos => state.prev_repo(),
                                Focus::Snapshots => state.prev_snapshot(),
                            }
                        }
                        KeyCode::Down => {
                            match state.focus {
                                Focus::Repos => state.next_repo(),
                                Focus::Snapshots => state.next_snapshot(),
                            }
                        }
                        KeyCode::Left => {
                            state.focus = Focus::Repos;
                        }
                        KeyCode::Right => {
                            if !state.snapshots.is_empty() {
                                state.focus = Focus::Snapshots;
                            }
                        }
                        KeyCode::Enter => {
                            if state.focus == Focus::Snapshots {
                                if let Some(snap_idx) = state.selected_snapshot_idx() {
                                    if let Some(repo_idx) = state.selected_repo_idx() {
                                        let repo = state.repos[repo_idx].clone();
                                        let hash = state.snapshots[snap_idx].commit_hash.clone();
                                        return Ok(Some((repo, hash)));
                                    }
                                }
                            } else if !state.snapshots.is_empty() {
                                // If they hit enter on repos and there are snapshots, switch focus to snapshots
                                state.focus = Focus::Snapshots;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
