use crate::config::Config;
use crate::snapshots::{self, SnapshotInfo};
use chrono::TimeZone;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        self,
        event::{self, Event, KeyCode, KeyEventKind},
    },
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::io;
use std::path::PathBuf;

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

#[derive(PartialEq, Clone, Copy)]
enum Focus {
    Repos,
    Snapshots,
    Files,
}

struct TuiState {
    repos: Vec<PathBuf>,
    repo_state: ListState,
    snapshots: Vec<SnapshotInfo>,
    snap_state: ListState,
    files: Vec<(char, String)>,
    files_state: ListState,
    selected_files: std::collections::HashSet<String>,
    focus: Focus,
    in_repo_select: bool,
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
            files: Vec::new(),
            files_state: ListState::default(),
            selected_files: std::collections::HashSet::new(),
            focus: Focus::Repos,
            in_repo_select: true,
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

    fn selected_file_idx(&self) -> Option<usize> {
        self.files_state.selected()
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
                self.reload_files();
                return;
            }
        }
        self.snapshots = Vec::new();
        self.snap_state.select(None);
        self.reload_files();
    }

    fn reload_files(&mut self) {
        self.selected_files.clear();
        self.files_state.select(None);
        if let Some(repo_idx) = self.selected_repo_idx() {
            if let Some(snap_idx) = self.selected_snapshot_idx() {
                if repo_idx < self.repos.len() && snap_idx < self.snapshots.len() {
                    let repo_path = &self.repos[repo_idx];
                    let commit_hash = &self.snapshots[snap_idx].commit_hash;
                    self.files =
                        snapshots::get_snapshot_files(repo_path, commit_hash).unwrap_or_default();
                    return;
                }
            }
        }
        self.files = Vec::new();
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
        self.reload_files();
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
        self.reload_files();
    }

    fn next_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let i = match self.files_state.selected() {
            Some(i) => {
                if i >= self.files.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.files_state.select(Some(i));
    }

    fn prev_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let i = match self.files_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.files.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.files_state.select(Some(i));
    }

    fn toggle_selected_file(&mut self) {
        if let Some(idx) = self.selected_file_idx() {
            if idx < self.files.len() {
                let path = self.files[idx].1.clone();
                if self.selected_files.contains(&path) {
                    self.selected_files.remove(&path);
                } else {
                    self.selected_files.insert(path);
                }
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn run_interactive(
) -> Result<Option<(PathBuf, String, Option<Vec<String>>)>, Box<dyn std::error::Error>> {
    let mut repos: Vec<PathBuf> = Config::load().git_repos().collect();
    repos.sort();

    if repos.is_empty() {
        println!("No watched repositories found. Add one with `endur watch <path>`.");
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
            let header = Paragraph::new("Endur Interactive Snapshot Restore")
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
                    Constraint::Percentage(40), // Left Pane
                    Constraint::Percentage(60), // Right Pane
                ])
                .split(chunks[1]);

            if state.in_repo_select {
                // Repositories list (Active)
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

                // Backups list (Preview)
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
                            .border_style(Style::default().fg(Color::DarkGray))
                            .title(" Backups (Preview) "),
                    )
                    .highlight_style(
                        Style::default()
                            .bg(Color::Rgb(40, 40, 40))
                            .fg(Color::Gray),
                    )
                    .highlight_symbol("   ");
                f.render_stateful_widget(snap_list, main_chunks[1], &mut state.snap_state);

            } else {
                // Backups list (Active)
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
                f.render_stateful_widget(snap_list, main_chunks[0], &mut state.snap_state);

                // Changed Files (Preview or Active)
                let file_border_color = if state.focus == Focus::Files {
                    Color::Green
                } else {
                    Color::DarkGray
                };
                let file_title = if state.focus == Focus::Files {
                    " Changed Files (Active) "
                } else {
                    " Changed Files (Preview) "
                };
                let file_items: Vec<ListItem> = if state.files.is_empty() {
                    vec![ListItem::new("No files changed in this backup")]
                } else {
                    state
                        .files
                        .iter()
                        .map(|(status, path)| {
                            let check = if state.selected_files.contains(path) {
                                "[x]"
                            } else {
                                "[ ]"
                            };
                            let status_style = match status {
                                'A' => Style::default().fg(Color::Green),
                                'D' => Style::default().fg(Color::Red),
                                'M' => Style::default().fg(Color::Yellow),
                                _ => Style::default().fg(Color::Cyan),
                            };
                            use ratatui::text::{Line, Span};
                            let line = Line::from(vec![
                                Span::raw(format!("{check} ")),
                                Span::styled(format!("[{status}]"), status_style),
                                Span::raw(format!(" {path}")),
                            ]);
                            ListItem::new(line)
                        })
                        .collect()
                };
                let file_list = List::new(file_items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(file_border_color))
                            .title(file_title),
                    )
                    .highlight_style(
                        Style::default()
                            .bg(Color::Rgb(40, 40, 40))
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(">> ");
                f.render_stateful_widget(file_list, main_chunks[1], &mut state.files_state);
            }

            // Footer instructions
            let help_text = match state.focus {
                Focus::Repos => " [Enter] Select Repo  |  [↑/↓] Navigate  |  [Esc/q] Exit",
                Focus::Snapshots => " [Enter] Restore Full Snapshot  |  [Esc/Backspace] Back to Repos  |  [Right/Tab] View Files  |  [↑/↓] Navigate",
                Focus::Files => " [Space] Toggle Select File  |  [Enter] Restore Selected Files  |  [Esc/Backspace/Left] Back to Backups  |  [↑/↓] Navigate",
            };
            let footer = Paragraph::new(help_text)
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
                        KeyCode::Char('q') => return Ok(None),
                        KeyCode::Esc => match state.focus {
                            Focus::Repos => return Ok(None),
                            Focus::Snapshots => {
                                state.in_repo_select = true;
                                state.focus = Focus::Repos;
                            }
                            Focus::Files => {
                                state.focus = Focus::Snapshots;
                                state.files_state.select(None);
                            }
                        },
                        KeyCode::Backspace => match state.focus {
                            Focus::Repos => {}
                            Focus::Snapshots => {
                                state.in_repo_select = true;
                                state.focus = Focus::Repos;
                            }
                            Focus::Files => {
                                state.focus = Focus::Snapshots;
                                state.files_state.select(None);
                            }
                        },
                        KeyCode::Up => match state.focus {
                            Focus::Repos => state.prev_repo(),
                            Focus::Snapshots => state.prev_snapshot(),
                            Focus::Files => state.prev_file(),
                        },
                        KeyCode::Down => match state.focus {
                            Focus::Repos => state.next_repo(),
                            Focus::Snapshots => state.next_snapshot(),
                            Focus::Files => state.next_file(),
                        },
                        KeyCode::Left => match state.focus {
                            Focus::Repos => {}
                            Focus::Snapshots => {
                                state.in_repo_select = true;
                                state.focus = Focus::Repos;
                            }
                            Focus::Files => {
                                state.focus = Focus::Snapshots;
                                state.files_state.select(None);
                            }
                        },
                        KeyCode::Right => match state.focus {
                            Focus::Repos => {
                                if !state.snapshots.is_empty() {
                                    state.in_repo_select = false;
                                    state.focus = Focus::Snapshots;
                                }
                            }
                            Focus::Snapshots => {
                                if !state.files.is_empty() {
                                    state.focus = Focus::Files;
                                    state.files_state.select(Some(0));
                                }
                            }
                            Focus::Files => {}
                        },
                        KeyCode::Tab => match state.focus {
                            Focus::Repos => {
                                if !state.snapshots.is_empty() {
                                    state.in_repo_select = false;
                                    state.focus = Focus::Snapshots;
                                }
                            }
                            Focus::Snapshots => {
                                if !state.files.is_empty() {
                                    state.focus = Focus::Files;
                                    state.files_state.select(Some(0));
                                } else {
                                    state.in_repo_select = true;
                                    state.focus = Focus::Repos;
                                }
                            }
                            Focus::Files => {
                                state.focus = Focus::Snapshots;
                                state.files_state.select(None);
                            }
                        },
                        KeyCode::Char(' ') => {
                            if state.focus == Focus::Files {
                                state.toggle_selected_file();
                            }
                        }
                        KeyCode::Enter => match state.focus {
                            Focus::Repos => {
                                if !state.snapshots.is_empty() {
                                    state.in_repo_select = false;
                                    state.focus = Focus::Snapshots;
                                }
                            }
                            Focus::Snapshots => {
                                if let Some(snap_idx) = state.selected_snapshot_idx() {
                                    if let Some(repo_idx) = state.selected_repo_idx() {
                                        let repo = state.repos[repo_idx].clone();
                                        let hash = state.snapshots[snap_idx].commit_hash.clone();
                                        return Ok(Some((repo, hash, None)));
                                    }
                                }
                            }
                            Focus::Files => {
                                if let Some(snap_idx) = state.selected_snapshot_idx() {
                                    if let Some(repo_idx) = state.selected_repo_idx() {
                                        let repo = state.repos[repo_idx].clone();
                                        let hash = state.snapshots[snap_idx].commit_hash.clone();

                                        if !state.selected_files.is_empty() {
                                            let files: Vec<String> =
                                                state.selected_files.iter().cloned().collect();
                                            return Ok(Some((repo, hash, Some(files))));
                                        } else if let Some(file_idx) = state.selected_file_idx() {
                                            if file_idx < state.files.len() {
                                                let file = state.files[file_idx].1.clone();
                                                return Ok(Some((repo, hash, Some(vec![file]))));
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        _ => {}
                    }
                }
            }
        }
    }
}
