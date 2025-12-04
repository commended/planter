use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::{
    error::Error,
    fs, io,
    path::PathBuf,
    time::{Duration, Instant},
};
use walkdir::WalkDir;

#[derive(Clone)]
struct FileNode {
    path: PathBuf,
    name: String,
    is_dir: bool,
    depth: usize,
    #[allow(dead_code)]
    size: u64,
    #[allow(dead_code)]
    children_count: usize,
}

struct Stats {
    total_files: usize,
    total_dirs: usize,
    total_size: u64,
    max_depth: usize,
}

struct App {
    nodes: Vec<FileNode>,
    animation_index: usize,
    animation_complete: bool,
    stats: Stats,
    root_path: PathBuf,
    scroll_offset: usize,
    selected_index: Option<usize>,
}

impl App {
    fn new(path: PathBuf) -> Result<Self, Box<dyn Error>> {
        let mut nodes = Vec::new();
        let mut stats = Stats {
            total_files: 0,
            total_dirs: 0,
            total_size: 0,
            max_depth: 0,
        };

        // Walk the directory tree
        for entry in WalkDir::new(&path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            let depth = entry.depth();
            let is_dir = path.is_dir();

            if is_dir {
                stats.total_dirs += 1;
            } else {
                stats.total_files += 1;
            }

            if depth > stats.max_depth {
                stats.max_depth = depth;
            }

            let size = if is_dir {
                0
            } else {
                fs::metadata(path).map(|m| m.len()).unwrap_or(0)
            };
            stats.total_size += size;

            let children_count = if is_dir {
                fs::read_dir(path)
                    .map(|entries| entries.count())
                    .unwrap_or(0)
            } else {
                0
            };

            nodes.push(FileNode {
                path: path.to_path_buf(),
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                is_dir,
                depth,
                size,
                children_count,
            });
        }

        Ok(App {
            nodes,
            animation_index: 0,
            animation_complete: false,
            stats,
            root_path: path,
            scroll_offset: 0,
            selected_index: None,
        })
    }

    fn increment_animation(&mut self) {
        if self.animation_index < self.nodes.len() {
            self.animation_index += 1;
        } else {
            self.animation_complete = true;
        }
    }

    fn handle_mouse_click(&mut self, row: u16, area: Rect) {
        if !self.animation_complete {
            return;
        }

        // Calculate which item was clicked (accounting for borders and scroll)
        if row > area.top() && row < area.bottom() - 1 {
            let clicked_index = (row - area.top() - 1) as usize + self.scroll_offset;
            if clicked_index < self.animation_index && clicked_index < self.nodes.len() {
                let node = &self.nodes[clicked_index];
                if node.is_dir {
                    // Open directory in default file manager
                    let _ = opener::open(&node.path);
                }
                self.selected_index = Some(clicked_index);
            }
        }
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn scroll_down(&mut self, visible_lines: usize) {
        let max_scroll = self.animation_index.saturating_sub(visible_lines);
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <directory_path>", args[0]);
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    if !path.exists() {
        eprintln!("Error: Path '{}' does not exist", path.display());
        std::process::exit(1);
    }
    if !path.is_dir() {
        eprintln!("Error: Path '{}' is not a directory", path.display());
        std::process::exit(1);
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new(path)?;

    // Run app
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    let animation_speed = Duration::from_millis(10); // Speed of animation
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui(f, app))?;

        let timeout = animation_speed.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Up => app.scroll_up(),
                    KeyCode::Down => {
                        let area_height = terminal.size()?.height.saturating_sub(4) as usize;
                        app.scroll_down(area_height);
                    }
                    KeyCode::PageUp => {
                        for _ in 0..10 {
                            app.scroll_up();
                        }
                    }
                    KeyCode::PageDown => {
                        let area_height = terminal.size()?.height.saturating_sub(4) as usize;
                        for _ in 0..10 {
                            app.scroll_down(area_height);
                        }
                    }
                    _ => {}
                }
            } else if let Event::Mouse(mouse) = event::read()? {
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                        .split(area);
                    app.handle_mouse_click(mouse.row, chunks[0]);
                }
            }
        }

        if last_tick.elapsed() >= animation_speed {
            if !app.animation_complete {
                app.increment_animation();
            }
            last_tick = Instant::now();
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());

    // Left panel: Tree view
    render_tree(f, app, chunks[0]);

    // Right panel: Statistics
    render_stats(f, app, chunks[1]);
}

fn render_tree(f: &mut Frame, app: &App, area: Rect) {
    let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
    let visible_nodes: Vec<ListItem> = app
        .nodes
        .iter()
        .take(app.animation_index)
        .skip(app.scroll_offset)
        .take(visible_height)
        .enumerate()
        .map(|(i, node)| {
            let actual_index = i + app.scroll_offset;
            let indent = "  ".repeat(node.depth);
            let icon = if node.is_dir {
                if node.depth == 0 {
                    "🌱"
                } else {
                    "📁"
                }
            } else {
                "📄"
            };

            let display_name = if node.name.is_empty() {
                node.path.to_string_lossy().to_string()
            } else {
                node.name.clone()
            };

            let mut style = Style::default();
            if node.is_dir {
                style = style.fg(Color::Cyan).add_modifier(Modifier::BOLD);
            }

            if app.selected_index == Some(actual_index) {
                style = style.bg(Color::DarkGray);
            }

            let line = Line::from(vec![
                Span::raw(indent),
                Span::styled(format!("{} {}", icon, display_name), style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let title = format!(
        " {} ({}/{}) ",
        if app.animation_complete {
            "🌳 Tree"
        } else {
            "🌱 Growing..."
        },
        app.animation_index,
        app.nodes.len()
    );

    let list = List::new(visible_nodes).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().fg(Color::Green)),
    );

    f.render_widget(list, area);
}

fn render_stats(f: &mut Frame, app: &App, area: Rect) {
    let stats_text = vec![
        Line::from(vec![Span::styled(
            "📊 Statistics",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Path: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(app.root_path.to_string_lossy().to_string()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Directories: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}", app.stats.total_dirs),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled("Files: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{}", app.stats.total_files),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "Total Items: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}", app.stats.total_files + app.stats.total_dirs),
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Total Size: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                humansize::format_size(app.stats.total_size, humansize::BINARY),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Max Depth: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{}", app.stats.max_depth),
                Style::default().fg(Color::Blue),
            ),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Controls:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::raw("↑/↓ - Scroll")]),
        Line::from(vec![Span::raw("PgUp/PgDn - Fast scroll")]),
        if app.animation_complete {
            Line::from(vec![Span::styled(
                "Click folder - Open",
                Style::default().fg(Color::Green),
            )])
        } else {
            Line::from(vec![Span::styled(
                "Wait for animation...",
                Style::default().fg(Color::DarkGray),
            )])
        },
        Line::from(vec![Span::raw("Q/Esc - Quit")]),
    ];

    let paragraph = Paragraph::new(stats_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Info ")
            .style(Style::default().fg(Color::Green)),
    );

    f.render_widget(paragraph, area);
}
