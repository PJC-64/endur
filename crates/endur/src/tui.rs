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
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Terminal,
};
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::SystemTime;

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

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Focus {
    Repos,
    Snapshots,
    Files,
}

#[derive(Clone)]
pub struct TuiState {
    pub repos: Vec<PathBuf>,
    pub repo_state: ListState,
    pub snapshots: Vec<SnapshotInfo>,
    pub snap_state: ListState,
    pub files: Vec<(char, String)>,
    pub files_state: ListState,
    pub selected_files: std::collections::HashSet<String>,
    pub focus: Focus,
    pub in_repo_select: bool,
    /// When false (default), only show snapshots after the current HEAD commit.
    pub show_all_snapshots: bool,
}

impl TuiState {
    pub fn new(repos: Vec<PathBuf>) -> Self {
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
            show_all_snapshots: false,
        };
        state.reload_snapshots();
        state
    }

    pub fn selected_repo_idx(&self) -> Option<usize> {
        self.repo_state.selected()
    }

    pub fn selected_snapshot_idx(&self) -> Option<usize> {
        self.snap_state.selected()
    }

    pub fn selected_file_idx(&self) -> Option<usize> {
        self.files_state.selected()
    }

    pub fn reload_snapshots(&mut self) {
        if let Some(idx) = self.selected_repo_idx() {
            if idx < self.repos.len() {
                let path = &self.repos[idx];
                self.snapshots =
                    snapshots::list_snapshots(path, self.show_all_snapshots).unwrap_or_default();
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

    pub fn reload_files(&mut self) {
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

    pub fn next_repo(&mut self) {
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

    pub fn prev_repo(&mut self) {
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

    pub fn next_snapshot(&mut self) {
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

    pub fn prev_snapshot(&mut self) {
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

    pub fn next_file(&mut self) {
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

    pub fn prev_file(&mut self) {
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

    pub fn toggle_selected_file(&mut self) {
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
                            .title(if state.show_all_snapshots {
                                " Backups [All] "
                            } else {
                                " Backups [Since HEAD] "
                            }),
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
                Focus::Snapshots => " [Enter] Restore Full  |  [A] Toggle All/HEAD  |  [Esc] Back  |  [Right/Tab] Files  |  [↑/↓] Nav",
                Focus::Files => " [Space] Toggle Select  |  [Enter] Restore Selected  |  [Esc/Left] Back  |  [↑/↓] Nav",
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
                        KeyCode::Char('a') | KeyCode::Char('A')
                            if state.focus == Focus::Snapshots =>
                        {
                            state.show_all_snapshots = !state.show_all_snapshots;
                            state.reload_snapshots();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

// =============================================================================
// NEW: Async Control Center TUI
// =============================================================================

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum ControlCenterTab {
    Repos,
    Snapshots,
    Logs,
    Metrics,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Tab2Focus {
    Snapshots,
    Files,
    Preview,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum ManagementMode {
    Direct,
    Service,
}

fn contains_git_repo(path: &std::path::Path) -> bool {
    if path.join(".git").is_dir() {
        return true;
    }
    let walk = walkdir::WalkDir::new(path)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok());
    for entry in walk {
        if entry.file_type().is_dir()
            && entry
                .path()
                .file_name()
                .map(|n| n == ".git")
                .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn get_base_root() -> PathBuf {
    let config = Config::load();
    if let Some(ref br) = config.base_root {
        let expanded = if let Some(stripped) = br.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(stripped)
            } else {
                PathBuf::from(br)
            }
        } else {
            PathBuf::from(br)
        };
        if expanded.exists() {
            return expanded;
        }
    }
    if let Some(home) = dirs::home_dir() {
        let dev = home.join("Development");
        if dev.exists() {
            return dev;
        }
        return home;
    }
    PathBuf::from("/")
}

pub struct ControlCenterState {
    pub tab: ControlCenterTab,
    pub repos_state: TuiState,
    pub tab2_focus: Tab2Focus,

    pub daemon_running: bool,
    pub daemon_pid: Option<u32>,
    pub daemon_start_time: Option<SystemTime>,
    pub daemon_version: Option<String>,

    pub management_mode: ManagementMode,
    pub service_installed: bool,
    pub service_running: bool,

    pub file_selector_active: bool,
    pub selector_current_dir: PathBuf,
    pub selector_entries: Vec<PathBuf>,
    pub selector_selected_idx: usize,
    pub selector_list_state: ListState,

    pub logs: Vec<String>,
    pub logs_scroll: u16,
    pub preview_scroll: u16,
    pub metrics_text: String,
    pub metrics_scroll: u16,

    pub input_mode: bool,
    pub input_buffer: String,

    pub message: Option<String>,
    pub message_time: Option<std::time::Instant>,
}

impl ControlCenterState {
    pub fn new(repos_state: TuiState) -> Self {
        let log_path = crate::database::RuntimeLock::get_endur_cache_home().join("endur.log");
        let initial_logs = read_initial_logs(&log_path);

        let daemon_running = crate::database::RuntimeLock::is_active();
        let (daemon_pid, daemon_start_time) = if daemon_running {
            let lock = crate::database::RuntimeLock::load();
            (lock.pid, lock.start_time)
        } else {
            (None, None)
        };

        let service_installed = crate::service::is_installed();
        let service_running = crate::service::is_running().unwrap_or(false);

        let base_root = get_base_root();

        let mut state = Self {
            tab: ControlCenterTab::Repos,
            repos_state,
            tab2_focus: Tab2Focus::Snapshots,
            daemon_running,
            daemon_pid,
            daemon_start_time,
            daemon_version: None,
            management_mode: if service_installed && service_running {
                ManagementMode::Service
            } else {
                ManagementMode::Direct
            },
            service_installed,
            service_running,
            file_selector_active: false,
            selector_current_dir: base_root,
            selector_entries: Vec::new(),
            selector_selected_idx: 0,
            selector_list_state: ListState::default(),
            logs: initial_logs,
            logs_scroll: 0,
            preview_scroll: 0,
            metrics_text: String::new(),
            metrics_scroll: 0,
            input_mode: false,
            input_buffer: String::new(),
            message: Some("Welcome to Endur Control Center!".to_string()),
            message_time: Some(std::time::Instant::now()),
        };
        state.update_metrics();
        state
    }

    pub fn update_metrics(&mut self) {
        let log_path = crate::database::RuntimeLock::get_endur_cache_home().join("endur.log");
        if let Ok(mut file) = std::fs::File::open(&log_path) {
            let mut output = Vec::new();
            if crate::metrics::get_snapshot_metrics(&mut file, &mut output, true, true).is_ok() {
                if let Ok(s) = String::from_utf8(output) {
                    self.metrics_text = s;
                    return;
                }
            }
        }
        self.metrics_text = "No metrics found or failed to read log file.".to_string();
    }

    pub fn show_message(&mut self, msg: String) {
        self.message = Some(msg);
        self.message_time = Some(std::time::Instant::now());
    }

    pub fn update_selector_entries(&mut self) {
        let current = &self.selector_current_dir;
        let mut entries = Vec::new();

        // 1. Option to watch current directory if it is a git repo itself
        if current.join(".git").is_dir() {
            entries.push(current.clone());
        }

        // 2. Option to go to parent directory
        if let Some(parent) = current.parent() {
            entries.push(parent.to_path_buf());
        }

        // 3. Subdirectories containing git repos
        let mut subdirs = Vec::new();
        if let Ok(dir_entries) = std::fs::read_dir(current) {
            for entry in dir_entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with('.') {
                            continue;
                        }
                    }
                    if contains_git_repo(&path) {
                        subdirs.push(path);
                    }
                }
            }
        }
        subdirs.sort();
        entries.extend(subdirs);

        self.selector_entries = entries;
        self.selector_selected_idx = 0;
        self.selector_list_state.select(Some(0));
    }

    pub fn get_current_preview_text(&self) -> String {
        if let Some(repo_idx) = self.repos_state.selected_repo_idx() {
            if let Some(snap_idx) = self.repos_state.selected_snapshot_idx() {
                if repo_idx < self.repos_state.repos.len()
                    && snap_idx < self.repos_state.snapshots.len()
                {
                    let repo_path = &self.repos_state.repos[repo_idx];
                    let commit_hash = &self.repos_state.snapshots[snap_idx].commit_hash;

                    if self.tab2_focus == Tab2Focus::Preview || self.tab2_focus == Tab2Focus::Files
                    {
                        if let Some(file_idx) = self.repos_state.selected_file_idx() {
                            if file_idx < self.repos_state.files.len() {
                                let file_path = &self.repos_state.files[file_idx].1;
                                return get_file_diff(repo_path, commit_hash, file_path);
                            }
                        }
                    }

                    return get_commit_diff(repo_path, commit_hash);
                }
            }
        }
        "No diff preview available".to_string()
    }
}

pub fn format_log_line(line: &str) -> Option<String> {
    let json: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return Some(line.to_string()),
    };

    let time_str = if let Some(time_val) = json.get("time").and_then(|v| v.as_str()) {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(time_val) {
            format!("{} ", dt.format("%Y-%m-%d %H:%M:%S"))
        } else {
            format!("{} ", time_val)
        }
    } else {
        String::new()
    };

    let level_raw = json.get("level").and_then(|v| v.as_str()).unwrap_or("INFO");
    let level_str = if level_raw.contains("Error") || level_raw.contains("ERROR") {
        "ERROR"
    } else if level_raw.contains("Warn") || level_raw.contains("WARN") {
        "WARN"
    } else if level_raw.contains("Debug") || level_raw.contains("DEBUG") {
        "DEBUG"
    } else if level_raw.contains("Trace") || level_raw.contains("TRACE") {
        "TRACE"
    } else {
        "INFO"
    };

    let fields = match json.get("fields").and_then(|v| v.as_object()) {
        Some(f) => f,
        None => {
            let msg = json.get("message").and_then(|v| v.as_str()).unwrap_or("");
            return Some(format!("{}[{}] {}", time_str, level_str, msg));
        }
    };

    let message = fields.get("message").and_then(|v| v.as_str()).unwrap_or("");

    if message == "info_operation" {
        if let Some(operation) = fields.get("operation").and_then(|v| v.as_object()) {
            if let Some(snapshot) = operation.get("Snapshot").and_then(|v| v.as_object()) {
                let repo = snapshot
                    .get("repo")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let error = snapshot.get("error").and_then(|v| v.as_str());
                let op = snapshot.get("op").and_then(|v| v.as_object());
                let latency = snapshot
                    .get("latency")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                if let Some(op_obj) = op {
                    let commit_hash = op_obj
                        .get("commit_hash")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let endur_branch = op_obj
                        .get("endur_branch")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let short_hash = if commit_hash.len() >= 8 {
                        &commit_hash[..8]
                    } else {
                        commit_hash
                    };
                    return Some(format!(
                        "{}[{}] Repository '{}' snapshot captured: commit {}, branch {} (latency: {:.2}s)",
                        time_str, level_str, repo, short_hash, endur_branch, latency
                    ));
                } else if let Some(err_msg) = error {
                    return Some(format!(
                        "{}[{}] Repository '{}' snapshot failed: {} (latency: {:.2}s)",
                        time_str, "ERROR", repo, err_msg, latency
                    ));
                } else {
                    return None;
                }
            } else if let Some(stats) = operation.get("CollectStats").and_then(|v| v.as_object()) {
                let count = stats
                    .get("loop_stats")
                    .and_then(|v| v.as_object())
                    .and_then(|h| h.get("count"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                return Some(format!(
                    "{}[{}] Stats collection: processed {} check loops",
                    time_str, level_str, count
                ));
            }
        }
    }

    if message.starts_with("Checking repo for changes:") {
        return None;
    }

    Some(format!("{}[{}] {}", time_str, level_str, message))
}

fn read_initial_logs(path: &std::path::Path) -> Vec<String> {
    if let Ok(file) = std::fs::File::open(path) {
        let reader = std::io::BufReader::new(file);
        use std::io::BufRead;
        let mut formatted_lines = Vec::new();
        for line in reader.lines().map_while(Result::ok) {
            if let Some(formatted) = format_log_line(&line) {
                formatted_lines.push(formatted);
            }
        }
        let len = formatted_lines.len();
        if len > 100 {
            formatted_lines[len - 100..].to_vec()
        } else {
            formatted_lines
        }
    } else {
        Vec::new()
    }
}

fn get_file_diff(repo_path: &std::path::Path, commit_hash: &str, file_path: &str) -> String {
    if let Ok(repo) = git2::Repository::open(repo_path) {
        if let Ok(oid) = git2::Oid::from_str(commit_hash) {
            if let Ok(commit) = repo.find_commit(oid) {
                let mut diff_text = Vec::new();
                if commit.parent_count() > 0 {
                    if let Ok(parent) = commit.parent(0) {
                        let mut diff_opts = git2::DiffOptions::new();
                        diff_opts.pathspec(file_path);
                        if let Ok(diff) = repo.diff_tree_to_tree(
                            Some(&parent.tree().unwrap()),
                            Some(&commit.tree().unwrap()),
                            Some(&mut diff_opts),
                        ) {
                            let _ = diff.print(git2::DiffFormat::Patch, |_, _, line| {
                                diff_text.extend_from_slice(line.content());
                                true
                            });
                        }
                    }
                } else {
                    if let Ok(diff) =
                        repo.diff_tree_to_tree(None, Some(&commit.tree().unwrap()), None)
                    {
                        let _ = diff.print(git2::DiffFormat::Patch, |_, _, line| {
                            diff_text.extend_from_slice(line.content());
                            true
                        });
                    }
                }
                if diff_text.is_empty() {
                    return "No changes in this file.".to_string();
                }
                return String::from_utf8_lossy(&diff_text).to_string();
            }
        }
    }
    "Failed to load diff.".to_string()
}

fn get_commit_diff(repo_path: &std::path::Path, commit_hash: &str) -> String {
    if let Ok(repo) = git2::Repository::open(repo_path) {
        if let Ok(oid) = git2::Oid::from_str(commit_hash) {
            if let Ok(commit) = repo.find_commit(oid) {
                let mut diff_text = Vec::new();
                diff_text.extend_from_slice(format!("Commit: {}\n", commit.id()).as_bytes());
                if let Some(author) = commit.author().name() {
                    diff_text.extend_from_slice(format!("Author: {}\n", author).as_bytes());
                }
                if let Some(summary) = commit.summary() {
                    diff_text.extend_from_slice(format!("Message: {}\n\n", summary).as_bytes());
                }

                if commit.parent_count() > 0 {
                    if let Ok(parent) = commit.parent(0) {
                        if let Ok(diff) = repo.diff_tree_to_tree(
                            Some(&parent.tree().unwrap()),
                            Some(&commit.tree().unwrap()),
                            None,
                        ) {
                            let _ = diff.print(git2::DiffFormat::Patch, |_, _, line| {
                                diff_text.extend_from_slice(line.content());
                                true
                            });
                        }
                    }
                } else {
                    if let Ok(diff) =
                        repo.diff_tree_to_tree(None, Some(&commit.tree().unwrap()), None)
                    {
                        let _ = diff.print(git2::DiffFormat::Patch, |_, _, line| {
                            diff_text.extend_from_slice(line.content());
                            true
                        });
                    }
                }
                return String::from_utf8_lossy(&diff_text).to_string();
            }
        }
    }
    "Failed to load commit diff.".to_string()
}

pub async fn run_control_center() -> Result<(), Box<dyn std::error::Error>> {
    let mut repos: Vec<PathBuf> = Config::load().git_repos().collect();
    repos.sort();

    let repos_state = TuiState::new(repos);
    let mut state = ControlCenterState::new(repos_state);

    let _guard = TerminalGuard::create()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    // Channels for async tasks
    let (key_tx, mut key_rx) = tokio::sync::mpsc::channel(100);
    let (status_tx, mut status_rx) = tokio::sync::mpsc::channel(10);
    let (log_tx, mut log_rx) = tokio::sync::mpsc::channel(100);

    // 1. Spawning key listener thread
    tokio::spawn(async move {
        loop {
            if event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    if key.kind == KeyEventKind::Press && key_tx.send(key).await.is_err() {
                        break;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    });

    // 2. Spawning daemon status checker task
    tokio::spawn(async move {
        loop {
            let running = crate::database::RuntimeLock::is_active();
            let mut daemon_version = None;
            let lock_info = if running {
                let lock = crate::database::RuntimeLock::load();
                if let Ok(res) = crate::poller::send_uds_command("status").await {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&res) {
                        daemon_version = val
                            .get("version")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                }
                Some((lock.pid, lock.start_time, daemon_version))
            } else {
                None
            };
            let service_installed = crate::service::is_installed();
            let service_running = crate::service::is_running().unwrap_or(false);
            if status_tx
                .send((running, lock_info, service_installed, service_running))
                .await
                .is_err()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });

    // 3. Spawning log file tailing task
    let log_path = crate::database::RuntimeLock::get_endur_cache_home().join("endur.log");
    let log_path_clone = log_path.clone();
    tokio::spawn(async move {
        let mut file_offset = if let Ok(metadata) = std::fs::metadata(&log_path_clone) {
            metadata.len()
        } else {
            0
        };
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if let Ok(metadata) = std::fs::metadata(&log_path_clone) {
                let new_len = metadata.len();
                if new_len > file_offset {
                    if let Ok(mut file) = std::fs::File::open(&log_path_clone) {
                        use std::io::Seek;
                        if file.seek(std::io::SeekFrom::Start(file_offset)).is_ok() {
                            let mut buffer = String::new();
                            if let Ok(bytes_read) = file.read_to_string(&mut buffer) {
                                if bytes_read > 0 {
                                    file_offset += bytes_read as u64;
                                    for line in buffer.lines() {
                                        if !line.is_empty() {
                                            let _ = log_tx.send(line.to_string()).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if new_len < file_offset {
                    file_offset = 0;
                }
            }
        }
    });

    let mut tick_interval = tokio::time::interval(std::time::Duration::from_millis(250));

    loop {
        // Clear message if it's older than 5 seconds
        if let Some(time) = state.message_time {
            if time.elapsed() > std::time::Duration::from_secs(5) {
                state.message = None;
                state.message_time = None;
            }
        }

        // Draw TUI
        terminal.draw(|f| {
            draw_control_center(f, &state);
        })?;

        // Process async updates or inputs
        tokio::select! {
            _ = tick_interval.tick() => {
                // Let the loop trigger a redraw on tick
            }
            // Key events
            Some(key) = key_rx.recv() => {
                if state.file_selector_active {
                    match key.code {
                        KeyCode::Esc => {
                            state.file_selector_active = false;
                        }
                        KeyCode::Up => {
                            if state.selector_selected_idx > 0 {
                                state.selector_selected_idx -= 1;
                                state.selector_list_state.select(Some(state.selector_selected_idx));
                            }
                        }
                        KeyCode::Down => {
                            if !state.selector_entries.is_empty() && state.selector_selected_idx < state.selector_entries.len() - 1 {
                                state.selector_selected_idx += 1;
                                state.selector_list_state.select(Some(state.selector_selected_idx));
                            }
                        }
                        KeyCode::Enter if !state.selector_entries.is_empty() => {
                            let selected_path = state.selector_entries[state.selector_selected_idx].clone();
                            if selected_path == state.selector_current_dir {
                                // Watch current directory!
                                let path_str = selected_path.to_string_lossy().to_string();
                                let mut config = crate::config::Config::load();
                                if !config.repos.contains_key(&path_str) {
                                    if let Err(e) = config.set_watch(path_str.clone(), crate::config::WatchConfig::default()) {
                                        state.show_message(format!("Failed to watch: {}", e));
                                    } else {
                                        config.save();
                                        let _ = crate::poller::send_uds_command("reload").await;
                                        state.show_message(format!("Started watching {}", path_str));
                                        let mut new_repos: Vec<PathBuf> = crate::config::Config::load().git_repos().collect();
                                        new_repos.sort();
                                        state.repos_state.repos = new_repos;
                                        state.file_selector_active = false;
                                    }
                                } else {
                                    state.show_message("Already watching this repository.".to_string());
                                }
                            } else if Some(selected_path.as_path()) == state.selector_current_dir.parent() {
                                // Go to parent directory
                                state.selector_current_dir = selected_path;
                                state.update_selector_entries();
                            } else {
                                // Go to subdirectory
                                state.selector_current_dir = selected_path;
                                state.update_selector_entries();
                            }
                        }
                        KeyCode::Char(' ') if !state.selector_entries.is_empty() => {
                            // Watch highlighted folder (Space bar)
                            let selected_path = state.selector_entries[state.selector_selected_idx].clone();
                            if selected_path.join(".git").is_dir() {
                                let path_str = selected_path.to_string_lossy().to_string();
                                let mut config = crate::config::Config::load();
                                if !config.repos.contains_key(&path_str) {
                                    if let Err(e) = config.set_watch(path_str.clone(), crate::config::WatchConfig::default()) {
                                        state.show_message(format!("Failed to watch: {}", e));
                                    } else {
                                        config.save();
                                        let _ = crate::poller::send_uds_command("reload").await;
                                        state.show_message(format!("Started watching {}", path_str));
                                        let mut new_repos: Vec<PathBuf> = crate::config::Config::load().git_repos().collect();
                                        new_repos.sort();
                                        state.repos_state.repos = new_repos;
                                        state.file_selector_active = false;
                                    }
                                } else {
                                    state.show_message("Already watching this repository.".to_string());
                                }
                            } else {
                                state.show_message("Selected folder is not a Git repository.".to_string());
                            }
                        }
                        _ => {}
                    }
                } else if state.input_mode {
                    match key.code {
                        KeyCode::Esc => {
                            state.input_mode = false;
                            state.input_buffer.clear();
                        }
                        KeyCode::Enter => {
                            let path_str = state.input_buffer.trim().to_string();
                            state.input_mode = false;
                            state.input_buffer.clear();
                            if !path_str.is_empty() {
                                let mut config = crate::config::Config::load();
                                if let Err(e) = config.set_watch(path_str.clone(), crate::config::WatchConfig::default()) {
                                    state.show_message(format!("Failed to watch: {}", e));
                                } else {
                                    config.save();
                                    let _ = crate::poller::send_uds_command("reload").await;
                                    state.show_message(format!("Started watching {}", path_str));
                                    let mut new_repos: Vec<PathBuf> = crate::config::Config::load().git_repos().collect();
                                    new_repos.sort();
                                    state.repos_state.repos = new_repos;
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            state.input_buffer.pop();
                        }
                        KeyCode::Char(c) => {
                            state.input_buffer.push(c);
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('1') => {
                            state.tab = ControlCenterTab::Repos;
                        }
                        KeyCode::Char('2') => {
                            state.tab = ControlCenterTab::Snapshots;
                        }
                        KeyCode::Char('3') => {
                            state.tab = ControlCenterTab::Logs;
                        }
                        KeyCode::Char('4') => {
                            state.tab = ControlCenterTab::Metrics;
                            state.update_metrics();
                        }
                        KeyCode::Tab => {
                            state.tab = match state.tab {
                                ControlCenterTab::Repos => ControlCenterTab::Snapshots,
                                ControlCenterTab::Snapshots => ControlCenterTab::Logs,
                                ControlCenterTab::Logs => ControlCenterTab::Metrics,
                                ControlCenterTab::Metrics => ControlCenterTab::Repos,
                            };
                            if state.tab == ControlCenterTab::Metrics {
                                state.update_metrics();
                            }
                        }
                        // Management mode toggle
                        KeyCode::Char('m') | KeyCode::Char('M') => {
                            state.management_mode = match state.management_mode {
                                ManagementMode::Direct => ManagementMode::Service,
                                ManagementMode::Service => ManagementMode::Direct,
                            };
                            state.show_message(format!(
                                "Switched to {:?} management mode",
                                state.management_mode
                            ));
                        }
                        // Service Installation / Uninstallation
                        KeyCode::Char('i') | KeyCode::Char('I') => {
                            if state.management_mode == ManagementMode::Service {
                                state.show_message("Installing service...".to_string());
                                match crate::service::install() {
                                    Ok(_) => {
                                        state.service_installed = true;
                                        state.service_running = true;
                                        state.show_message("Service installed and started successfully.".to_string());
                                    }
                                    Err(e) => {
                                        state.show_message(format!("Failed to install service: {}", e));
                                    }
                                }
                            }
                        }
                        KeyCode::Char('u') | KeyCode::Char('U') => {
                            if state.management_mode == ManagementMode::Service {
                                state.show_message("Uninstalling service...".to_string());
                                match crate::service::uninstall() {
                                    Ok(_) => {
                                        state.service_installed = false;
                                        state.service_running = false;
                                        state.show_message("Service uninstalled successfully.".to_string());
                                    }
                                    Err(e) => {
                                        state.show_message(format!("Failed to uninstall service: {}", e));
                                    }
                                }
                            }
                        }
                        // Daemon / Service Control
                        KeyCode::Char('s') | KeyCode::Char('S') => {
                            match state.management_mode {
                                ManagementMode::Direct => {
                                    if state.service_installed && state.service_running {
                                        state.show_message("Daemon is running as a system service. Toggle to Service mode [m] to manage.".to_string());
                                    } else {
                                        let current_exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("endur"));
                                        let logfile_path = crate::database::RuntimeLock::get_endur_cache_home().join("endur.log");
                                        let mut cmd = std::process::Command::new(current_exe);
                                        cmd.arg("serve")
                                            .arg("--logfile")
                                            .arg(logfile_path);
                                        #[cfg(unix)]
                                        {
                                            use std::os::unix::process::CommandExt;
                                            unsafe {
                                                cmd.pre_exec(|| {
                                                    extern "C" {
                                                        fn setsid() -> i32;
                                                    }
                                                    setsid();
                                                    Ok(())
                                                });
                                            }
                                        }
                                        #[cfg(windows)]
                                        {
                                            use std::os::windows::process::CommandExt;
                                            cmd.creation_flags(0x00000008 | 0x00000200);
                                        }
                                        let _child = cmd.spawn();
                                        state.show_message("Starting daemon...".to_string());
                                    }
                                }
                                ManagementMode::Service => {
                                    if !state.service_installed {
                                        state.show_message("Service is not installed. Press [i] to install it.".to_string());
                                    } else {
                                        state.show_message("Starting service...".to_string());
                                        match crate::service::start() {
                                            Ok(_) => {
                                                state.service_running = true;
                                                state.show_message("Service started successfully.".to_string());
                                            }
                                            Err(e) => {
                                                state.show_message(format!("Failed to start service: {}", e));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Char('K') => {
                            match state.management_mode {
                                ManagementMode::Direct => {
                                    if state.service_installed && state.service_running {
                                        state.show_message("Daemon is running as a system service. Toggle to Service mode [m] to manage.".to_string());
                                    } else {
                                        let res = crate::poller::send_uds_command("kill").await;
                                        match res {
                                            Ok(msg) => state.show_message(format!("Daemon stopped: {}", msg)),
                                            Err(_) => {
                                                if crate::database::RuntimeLock::is_active() {
                                                    let mut lock = crate::database::RuntimeLock::load();
                                                    lock.pid = None;
                                                    lock.save();
                                                    state.show_message("Daemon stopped via lock file fallback.".to_string());
                                                } else {
                                                    state.show_message("Daemon is not running.".to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                                ManagementMode::Service => {
                                    if !state.service_installed {
                                        state.show_message("Service is not installed.".to_string());
                                    } else {
                                        state.show_message("Stopping service...".to_string());
                                        match crate::service::stop() {
                                            Ok(_) => {
                                                state.service_running = false;
                                                state.show_message("Service stopped successfully.".to_string());
                                            }
                                            Err(e) => {
                                                state.show_message(format!("Failed to stop service: {}", e));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            let res = crate::poller::send_uds_command("reload").await;
                            match res {
                                Ok(msg) => state.show_message(format!("Config reloaded: {}", msg)),
                                Err(e) => state.show_message(format!("Reload failed: {}", e)),
                            }
                        }
                        // Tab Specific inputs
                        _ => {
                            match state.tab {
                                ControlCenterTab::Repos => {
                                    match key.code {
                                        KeyCode::Up => state.repos_state.prev_repo(),
                                        KeyCode::Down => state.repos_state.next_repo(),
                                        KeyCode::Char('a') => {
                                            state.file_selector_active = true;
                                            state.selector_current_dir = get_base_root();
                                            state.update_selector_entries();
                                        }
                                        KeyCode::Char('d') => {
                                            if let Some(idx) = state.repos_state.selected_repo_idx() {
                                                if idx < state.repos_state.repos.len() {
                                                    let path_str = state.repos_state.repos[idx].to_string_lossy().to_string();
                                                    let mut config = crate::config::Config::load();
                                                    if let Err(e) = config.set_unwatch(path_str.clone()) {
                                                        state.show_message(format!("Failed to unwatch: {}", e));
                                                    } else {
                                                        config.save();
                                                        let _ = crate::poller::send_uds_command("reload").await;
                                                        state.show_message(format!("Stopped watching {}", path_str));
                                                        let mut new_repos: Vec<PathBuf> = crate::config::Config::load().git_repos().collect();
                                                        new_repos.sort();
                                                        state.repos_state.repos = new_repos;
                                                        state.repos_state.repo_state.select(Some(0));
                                                        state.repos_state.reload_snapshots();
                                                    }
                                                }
                                            }
                                        }
                                        KeyCode::Char('c') => {
                                            let mut config = crate::config::Config::load();
                                            let mut to_remove = Vec::new();
                                            for repo_path_str in config.repos.keys() {
                                                let path = std::path::Path::new(repo_path_str);
                                                if git2::Repository::open(path).is_err() {
                                                    to_remove.push(repo_path_str.clone());
                                                }
                                            }
                                            if to_remove.is_empty() {
                                                state.show_message("No invalid repositories found.".to_string());
                                            } else {
                                                for repo in &to_remove {
                                                    config.repos.remove(repo);
                                                }
                                                config.save();
                                                let _ = crate::poller::send_uds_command("reload").await;
                                                state.show_message(format!("Cleaned up {} repositories.", to_remove.len()));
                                                let mut new_repos: Vec<PathBuf> = crate::config::Config::load().git_repos().collect();
                                                new_repos.sort();
                                                state.repos_state.repos = new_repos;
                                                state.repos_state.repo_state.select(Some(0));
                                                state.repos_state.reload_snapshots();
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                ControlCenterTab::Snapshots => {
                                    match key.code {
                                        KeyCode::Left => {
                                            state.tab2_focus = match state.tab2_focus {
                                                Tab2Focus::Snapshots => Tab2Focus::Snapshots,
                                                Tab2Focus::Files => Tab2Focus::Snapshots,
                                                Tab2Focus::Preview => Tab2Focus::Files,
                                            };
                                        }
                                        KeyCode::Right => {
                                            state.tab2_focus = match state.tab2_focus {
                                                Tab2Focus::Snapshots => {
                                                    if !state.repos_state.files.is_empty() {
                                                        state.repos_state.files_state.select(Some(0));
                                                        Tab2Focus::Files
                                                    } else {
                                                        Tab2Focus::Snapshots
                                                    }
                                                }
                                                Tab2Focus::Files => Tab2Focus::Preview,
                                                Tab2Focus::Preview => Tab2Focus::Preview,
                                            };
                                        }
                                        KeyCode::Up => {
                                            match state.tab2_focus {
                                                Tab2Focus::Snapshots => state.repos_state.prev_snapshot(),
                                                Tab2Focus::Files => state.repos_state.prev_file(),
                                                Tab2Focus::Preview => {
                                                    if state.preview_scroll > 0 {
                                                        state.preview_scroll -= 1;
                                                    }
                                                }
                                            }
                                        }
                                        KeyCode::Down => {
                                            match state.tab2_focus {
                                                Tab2Focus::Snapshots => state.repos_state.next_snapshot(),
                                                Tab2Focus::Files => state.repos_state.next_file(),
                                                Tab2Focus::Preview => {
                                                    state.preview_scroll += 1;
                                                }
                                            }
                                        }
                                        KeyCode::Char(' ') => {
                                            if state.tab2_focus == Tab2Focus::Files {
                                                state.repos_state.toggle_selected_file();
                                            }
                                        }
                                        KeyCode::Enter => {
                                            if let Some(snap_idx) = state.repos_state.selected_snapshot_idx() {
                                                if let Some(repo_idx) = state.repos_state.selected_repo_idx() {
                                                    let repo = &state.repos_state.repos[repo_idx];
                                                    let hash = &state.repos_state.snapshots[snap_idx].commit_hash;

                                                    let files_to_restore = if state.tab2_focus == Tab2Focus::Files && !state.repos_state.selected_files.is_empty() {
                                                        Some(state.repos_state.selected_files.iter().cloned().collect::<Vec<String>>())
                                                    } else if state.tab2_focus == Tab2Focus::Files {
                                                        state.repos_state.selected_file_idx().map(|idx| vec![state.repos_state.files[idx].1.clone()])
                                                    } else {
                                                        None
                                                    };

                                                    drop(terminal);
                                                    let _ = crossterm::terminal::disable_raw_mode();
                                                    println!("\nRestoring... please wait.");

                                                    match snapshots::restore(repo, hash, files_to_restore.as_deref()) {
                                                        Ok(changes) => {
                                                            if changes.is_empty() {
                                                                println!("No files needed to be restored.");
                                                            } else {
                                                                println!("Successfully restored:");
                                                                for (status, path) in changes {
                                                                    println!("  {} {}", status, path);
                                                                }
                                                            }
                                                        }
                                                        Err(e) => println!("Restore failed: {}", e),
                                                    }

                                                    println!("\nPress Enter to return to TUI...");
                                                    let mut input = String::new();
                                                    let _ = std::io::stdin().read_line(&mut input);

                                                    crossterm::terminal::enable_raw_mode()?;
                                                    terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
                                                    terminal.clear()?;
                                                    state.repos_state.reload_files();
                                                    state.show_message("Restore completed.".to_string());
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                ControlCenterTab::Logs => {
                                    match key.code {
                                        KeyCode::Up => {
                                            if state.logs_scroll > 0 {
                                                state.logs_scroll -= 1;
                                            }
                                        }
                                        KeyCode::Down => {
                                            state.logs_scroll += 1;
                                        }
                                        _ => {}
                                    }
                                }
                                ControlCenterTab::Metrics => {
                                    match key.code {
                                        KeyCode::Up => {
                                            if state.metrics_scroll > 0 {
                                                state.metrics_scroll -= 1;
                                            }
                                        }
                                        KeyCode::Down => {
                                            state.metrics_scroll += 1;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Daemon status updates
            Some((running, lock_info, service_installed, service_running)) = status_rx.recv() => {
                state.daemon_running = running;
                state.service_installed = service_installed;
                state.service_running = service_running;
                if let Some((pid, start_time, version)) = lock_info {
                    state.daemon_pid = pid;
                    state.daemon_start_time = start_time;
                    state.daemon_version = version;
                } else {
                    state.daemon_pid = None;
                    state.daemon_start_time = None;
                    state.daemon_version = None;
                }
            }
            // Live logs tailing
            Some(line) = log_rx.recv() => {
                if let Some(formatted) = format_log_line(&line) {
                    state.logs.push(formatted);
                    if state.logs.len() > 500 {
                        state.logs.remove(0);
                    }
                }
                if state.tab == ControlCenterTab::Metrics {
                    state.update_metrics();
                }
            }
        }
    }

    Ok(())
}

fn draw_control_center(f: &mut ratatui::Frame, state: &ControlCenterState) {
    use ratatui::text::{Line, Span};

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Header (needs 6 lines for border + status, keys, info, and notes)
            Constraint::Length(1), // Tabs
            Constraint::Min(10),   // Active Pane
            Constraint::Length(9), // Footer (logs + messages + help)
        ])
        .split(f.area());

    // 1. Header
    let mut header_lines = Vec::new();

    match state.management_mode {
        ManagementMode::Direct => {
            if state.service_installed && state.service_running {
                // Line 1: Mode & Status
                header_lines.push(Line::from(vec![
                    Span::styled(" Mode: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "Direct Process",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  │  Daemon Status: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "● RUNNING AS SYSTEM SERVICE",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                // Line 2: Note / Warning
                header_lines.push(Line::from(vec![Span::styled(
                    " ℹ️ Running as a system service. Toggle to System Service mode [m] to manage.",
                    Style::default().fg(Color::Cyan),
                )]));
                // Line 3: Keys
                header_lines.push(Line::from(vec![
                    Span::styled(" Keys: ", Style::default().fg(Color::DarkGray)),
                    Span::styled("[m]", Style::default().fg(Color::Yellow)),
                    Span::raw(" Switch Management Mode  "),
                    Span::styled("[r]", Style::default().fg(Color::Yellow)),
                    Span::raw(" Reload Config"),
                ]));
            } else {
                // Not running under system service
                let status_style = if state.daemon_running {
                    let has_mismatch = if let Some(ref dv) = state.daemon_version {
                        dv != env!("CARGO_PKG_VERSION")
                    } else {
                        false
                    };
                    if has_mismatch {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    }
                } else {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                };

                let status_text = if state.daemon_running {
                    let pid_str = state
                        .daemon_pid
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let uptime_str = state
                        .daemon_start_time
                        .and_then(|t| SystemTime::now().duration_since(t).ok())
                        .map(|d| {
                            let secs = d.as_secs();
                            if secs < 60 {
                                format!("{}s", secs)
                            } else if secs < 3600 {
                                format!("{}m", secs / 60)
                            } else {
                                format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
                            }
                        })
                        .unwrap_or_else(|| "unknown".to_string());
                    format!("● RUNNING (PID: {}, Uptime: {})", pid_str, uptime_str)
                } else {
                    "● NOT RUNNING".to_string()
                };

                header_lines.push(Line::from(vec![
                    Span::styled(" Mode: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "Direct Process",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  │  Daemon Status: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(status_text, status_style),
                ]));

                header_lines.push(Line::from(vec![
                    Span::styled(" Keys: ", Style::default().fg(Color::DarkGray)),
                    Span::styled("[s]", Style::default().fg(Color::Yellow)),
                    Span::raw(" Start Daemon  "),
                    Span::styled("[k]", Style::default().fg(Color::Yellow)),
                    Span::raw(" Kill Daemon  "),
                    Span::styled("[r]", Style::default().fg(Color::Yellow)),
                    Span::raw(" Reload Config  "),
                    Span::styled("[m]", Style::default().fg(Color::Yellow)),
                    Span::raw(" Switch to Service Mode"),
                ]));

                // Line 3: Version mismatch warning if any
                let mut warning_line = Line::from("");
                if state.daemon_running {
                    if let Some(ref dv) = state.daemon_version {
                        let cv = env!("CARGO_PKG_VERSION");
                        if dv != cv {
                            warning_line = Line::from(vec![Span::styled(
                                format!(" ⚠️ Version mismatch! Running v{}, expected v{}.", dv, cv),
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                            )]);
                        }
                    }
                }
                header_lines.push(warning_line);
            }
        }
        ManagementMode::Service => {
            let config_status_text = if state.service_installed {
                "INSTALLED"
            } else {
                "NOT INSTALLED"
            };
            let config_status_style = if state.service_installed {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            };

            let service_status_text = if state.service_running {
                "RUNNING"
            } else {
                "STOPPED"
            };
            let service_status_style = if state.service_running {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            };

            header_lines.push(Line::from(vec![
                Span::styled(" Mode: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "System Service",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "  │  Service Config: ",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(config_status_text, config_status_style),
                Span::styled(
                    "  │  Service Status: ",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(service_status_text, service_status_style),
            ]));

            // Build action keys
            let mut keys_spans = vec![Span::styled(
                " Keys: ",
                Style::default().fg(Color::DarkGray),
            )];
            if !state.service_installed {
                keys_spans.push(Span::styled("[i]", Style::default().fg(Color::Yellow)));
                keys_spans.push(Span::raw(" Install Service  "));
            } else {
                keys_spans.push(Span::styled("[i]", Style::default().fg(Color::Yellow)));
                keys_spans.push(Span::raw(" Reinstall Service  "));
                keys_spans.push(Span::styled("[s]", Style::default().fg(Color::Yellow)));
                keys_spans.push(Span::raw(" Start Service  "));
                keys_spans.push(Span::styled("[k]", Style::default().fg(Color::Yellow)));
                keys_spans.push(Span::raw(" Stop Service  "));
                keys_spans.push(Span::styled("[u]", Style::default().fg(Color::Yellow)));
                keys_spans.push(Span::raw(" Uninstall Service  "));
            }
            keys_spans.push(Span::styled("[m]", Style::default().fg(Color::Yellow)));
            keys_spans.push(Span::raw(" Switch to Direct Mode"));

            header_lines.push(Line::from(keys_spans));

            // Line 3: Info & mismatch warning
            let mut info_spans = vec![];
            if state.service_installed && state.service_running {
                let pid_str = state
                    .daemon_pid
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let uptime_str = state
                    .daemon_start_time
                    .and_then(|t| SystemTime::now().duration_since(t).ok())
                    .map(|d| {
                        let secs = d.as_secs();
                        if secs < 60 {
                            format!("{}s", secs)
                        } else if secs < 3600 {
                            format!("{}m", secs / 60)
                        } else {
                            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
                        }
                    })
                    .unwrap_or_else(|| "unknown".to_string());
                let ver_str = state.daemon_version.as_deref().unwrap_or("unknown");

                info_spans.push(Span::styled(" PID: ", Style::default().fg(Color::DarkGray)));
                info_spans.push(Span::raw(format!("{}  │  ", pid_str)));
                info_spans.push(Span::styled(
                    "Uptime: ",
                    Style::default().fg(Color::DarkGray),
                ));
                info_spans.push(Span::raw(format!("{}  │  ", uptime_str)));
                info_spans.push(Span::styled(
                    "Version: ",
                    Style::default().fg(Color::DarkGray),
                ));
                info_spans.push(Span::raw(ver_str.to_string()));

                if let Some(ref dv) = state.daemon_version {
                    let cv = env!("CARGO_PKG_VERSION");
                    if dv != cv {
                        info_spans.push(Span::styled(
                            format!("  ⚠️ Version mismatch! (expected v{})", cv),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ));
                    }
                }
            }
            header_lines.push(Line::from(info_spans));

            // Line 4: Note / Description
            header_lines.push(Line::from(vec![
                Span::styled(" Note: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "Installing/Reinstalling stops & removes any existing service before installing the latest version.",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    let is_active = match state.management_mode {
        ManagementMode::Direct => {
            if state.service_installed && state.service_running {
                true
            } else {
                state.daemon_running
            }
        }
        ManagementMode::Service => state.service_running,
    };
    let has_mismatch = if state.daemon_running {
        if let Some(ref dv) = state.daemon_version {
            dv != env!("CARGO_PKG_VERSION")
        } else {
            false
        }
    } else {
        false
    };
    let border_color = if is_active {
        if has_mismatch {
            Color::Yellow
        } else {
            Color::Cyan
        }
    } else {
        Color::Red
    };
    let border_style = Style::default().fg(border_color);

    let header_widget = Paragraph::new(header_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .title(" Status & Control "),
    );
    f.render_widget(header_widget, chunks[0]);

    // 2. Tabs
    let tab_names = vec![
        " [1] Repositories ",
        " [2] Backups & Restore ",
        " [3] Full System Log ",
        " [4] Metrics ",
    ];
    let active_idx = match state.tab {
        ControlCenterTab::Repos => 0,
        ControlCenterTab::Snapshots => 1,
        ControlCenterTab::Logs => 2,
        ControlCenterTab::Metrics => 3,
    };
    let tabs_widget = Tabs::new(tab_names)
        .select(active_idx)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs_widget, chunks[1]);

    // 3. Active Pane
    match state.tab {
        ControlCenterTab::Repos => {
            let pane_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(chunks[2]);

            // Watched Repositories
            let repo_items: Vec<ListItem> = state
                .repos_state
                .repos
                .iter()
                .map(|p| ListItem::new(p.to_string_lossy().to_string()))
                .collect();
            let mut repos_list_state = state.repos_state.repo_state;
            let repos_list = List::new(repo_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Green))
                        .title(" Watched Repositories "),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::Rgb(40, 40, 40))
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");
            f.render_stateful_widget(repos_list, pane_chunks[0], &mut repos_list_state);

            // Details pane, Add Repo file selector, or legacy input
            if state.file_selector_active {
                // File selector display
                let current_dir_str = state.selector_current_dir.to_string_lossy().to_string();

                let selector_items: Vec<ListItem> = state
                    .selector_entries
                    .iter()
                    .map(|entry| {
                        let display_name = if entry == &state.selector_current_dir {
                            let name = state
                                .selector_current_dir
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "/".to_string());
                            format!("● [Watch Current Directory] {}", name)
                        } else if Some(entry)
                            == state
                                .selector_current_dir
                                .parent()
                                .map(|p| p.to_path_buf())
                                .as_ref()
                        {
                            "↱ .. (Parent Directory)".to_string()
                        } else {
                            let is_repo = entry.join(".git").is_dir();
                            let suffix = if is_repo { " [Git Repo]" } else { "" };
                            let folder_name = entry
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            format!("📁 {}{}", folder_name, suffix)
                        };

                        ListItem::new(display_name)
                    })
                    .collect();

                let mut list_state = state.selector_list_state;
                let list_widget = List::new(selector_items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::Yellow))
                            .title(format!(" File Selector │ Current: {} ", current_dir_str)),
                    )
                    .highlight_style(
                        Style::default()
                            .bg(Color::Rgb(40, 40, 40))
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("▶ ");
                f.render_stateful_widget(list_widget, pane_chunks[1], &mut list_state);
            } else if state.input_mode {
                let input_widget = Paragraph::new(format!("> {}", state.input_buffer)).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Yellow))
                        .title(" Add Repository to Watch (Enter absolute path) "),
                );
                f.render_widget(input_widget, pane_chunks[1]);
            } else {
                let detail_text = if let Some(idx) = state.repos_state.selected_repo_idx() {
                    if idx < state.repos_state.repos.len() {
                        let path = &state.repos_state.repos[idx];
                        let backups_count = state.repos_state.snapshots.len();
                        format!(
                            "Repository Path: {}\n\nTotal Snapshots: {}\n\nPress [d] to stop watching this repository.\nPress [c] to run cleanup on watched paths.",
                            path.display(),
                            backups_count
                        )
                    } else {
                        "No repository selected.".to_string()
                    }
                } else {
                    "No repository selected.".to_string()
                };

                let detail_widget = Paragraph::new(detail_text).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title(" Repository Detail "),
                );
                f.render_widget(detail_widget, pane_chunks[1]);
            }
        }
        ControlCenterTab::Snapshots => {
            let pane_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(30), // Snapshots
                    Constraint::Percentage(30), // Changed files
                    Constraint::Percentage(40), // Diff preview
                ])
                .split(chunks[2]);

            // Snapshots list
            let snap_border_color = if state.tab2_focus == Tab2Focus::Snapshots {
                Color::Green
            } else {
                Color::DarkGray
            };
            let snap_items: Vec<ListItem> = state
                .repos_state
                .snapshots
                .iter()
                .map(|s| {
                    let datetime = chrono::Local
                        .timestamp_opt(s.timestamp, 0)
                        .single()
                        .map(|dt| dt.format("%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    ListItem::new(format!("{} │ {}", &s.commit_hash[..8], datetime))
                })
                .collect();
            let mut snap_list_state = state.repos_state.snap_state;
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
                .highlight_symbol("▶ ");
            f.render_stateful_widget(snap_list, pane_chunks[0], &mut snap_list_state);

            // Changed Files list
            let file_border_color = if state.tab2_focus == Tab2Focus::Files {
                Color::Green
            } else {
                Color::DarkGray
            };
            let file_items: Vec<ListItem> = state
                .repos_state
                .files
                .iter()
                .map(|(status, path)| {
                    let check = if state.repos_state.selected_files.contains(path) {
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
                        Span::raw(format!("{} ", check)),
                        Span::styled(format!("[{}]", status), status_style),
                        Span::raw(format!(" {}", path)),
                    ]);
                    ListItem::new(line)
                })
                .collect();
            let mut file_list_state = state.repos_state.files_state;
            let file_list = List::new(file_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(file_border_color))
                        .title(" Files "),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::Rgb(40, 40, 40))
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");
            f.render_stateful_widget(file_list, pane_chunks[1], &mut file_list_state);

            // Diff Preview
            let preview_border_color = if state.tab2_focus == Tab2Focus::Preview {
                Color::Green
            } else {
                Color::DarkGray
            };
            let diff_text = state.get_current_preview_text();
            let diff_widget = Paragraph::new(diff_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(preview_border_color))
                        .title(" Diff Preview "),
                )
                .scroll((state.preview_scroll, 0))
                .wrap(ratatui::widgets::Wrap { trim: false });
            f.render_widget(diff_widget, pane_chunks[2]);
        }
        ControlCenterTab::Logs => {
            let reversed_logs: Vec<String> = state.logs.iter().rev().cloned().collect();
            let logs_text = reversed_logs.join("\n");
            let logs_widget = Paragraph::new(logs_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title(" Full System Log (endur.log) "),
                )
                .scroll((state.logs_scroll, 0));
            f.render_widget(logs_widget, chunks[2]);
        }
        ControlCenterTab::Metrics => {
            let metrics_widget = Paragraph::new(state.metrics_text.as_str())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Yellow))
                        .title(" Snapshot Metrics Summary (endur metrics) "),
                )
                .scroll((state.metrics_scroll, 0));
            f.render_widget(metrics_widget, chunks[2]);
        }
    }

    // 4. Footer
    let footer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Live logs stream
            Constraint::Length(1), // Actions message
            Constraint::Length(2), // Help shortcuts
        ])
        .split(chunks[3]);

    // Live logs stream
    let live_logs_count = state.logs.len();
    let live_logs_text = if live_logs_count > 4 {
        let mut last_logs: Vec<String> = state.logs[live_logs_count - 4..].to_vec();
        last_logs.reverse();
        last_logs.join("\n")
    } else {
        let mut last_logs = state.logs.clone();
        last_logs.reverse();
        last_logs.join("\n")
    };
    let live_logs_widget = Paragraph::new(live_logs_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Live Logs Stream "),
        );
    f.render_widget(live_logs_widget, footer_chunks[0]);

    // Action status message
    let message_text = state.message.as_deref().unwrap_or("");
    let message_widget = Paragraph::new(message_text).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(message_widget, footer_chunks[1]);

    // Help shortcuts
    let help_text = match state.tab {
        ControlCenterTab::Repos => {
            if state.input_mode {
                " [Esc] Cancel Input  │  [Enter] Submit Path"
            } else {
                " [Tab/1-4] Switch Tab  │  [a] Watch Repo  │  [d] Stop Watching  │  [c] Run Cleanup  │  [q] Exit"
            }
        }
        ControlCenterTab::Snapshots => {
            match state.tab2_focus {
                Tab2Focus::Snapshots => " [Tab/1-4] Switch Tab  │  [Right] View Files  │  [↑/↓] Navigate Backups  │  [q] Exit",
                Tab2Focus::Files => " [Space] Toggle Checkbox  │  [Enter] Restore Checked  │  [Left] Back to Backups  │  [Right] View Diff  │  [q] Exit",
                Tab2Focus::Preview => " [↑/↓] Scroll Diff Text  │  [Left] Back to Files  │  [Tab/1-4] Switch Tab  │  [q] Exit",
            }
        }
        ControlCenterTab::Logs => " [Tab/1-4] Switch Tab  │  [↑/↓] Scroll Logs  │  [q] Exit",
        ControlCenterTab::Metrics => " [Tab/1-4] Switch Tab  │  [↑/↓] Scroll Metrics  │  [q] Exit",
    };
    let help_widget = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(help_widget, footer_chunks[2]);
}
