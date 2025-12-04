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
    is_last_child: bool,
}

struct Stats {
    total_files: usize,
    total_dirs: usize,
    total_size: u64,
    max_depth: usize,
}

struct App {
    nodes: Vec<FileNode>,
    animation_depth: usize, // Current depth level being animated
    animation_complete: bool,
    stats: Stats,
    root_path: PathBuf,
    scroll_offset: usize,
    selected_index: Option<usize>,
    animation_frame: usize, // For root growth animation
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

        // Walk the directory tree - only collect directories
        for entry in WalkDir::new(&path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            let depth = entry.depth();
            let is_dir = path.is_dir();

            // Count all items for statistics
            if is_dir {
                stats.total_dirs += 1;
            } else {
                stats.total_files += 1;
                // Count file size for total
                let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                stats.total_size += size;
            }

            if depth > stats.max_depth {
                stats.max_depth = depth;
            }

            // Only add directories to nodes (not files)
            if is_dir {
                let children_count = fs::read_dir(path)
                    .map(|entries| entries.count())
                    .unwrap_or(0);

                nodes.push(FileNode {
                    path: path.to_path_buf(),
                    name: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    is_dir,
                    depth,
                    size: 0,
                    children_count,
                    is_last_child: false, // Will be computed below
                });
            }
        }

        // Compute is_last_child for each node
        for i in 0..nodes.len() {
            let current_depth = nodes[i].depth;
            let current_parent = nodes[i].path.parent();
            
            // Check if this is the last child at its level with the same parent
            let mut is_last = true;
            for j in (i + 1)..nodes.len() {
                if nodes[j].depth < current_depth {
                    break; // No more siblings at this depth
                }
                if nodes[j].depth == current_depth {
                    let sibling_parent = nodes[j].path.parent();
                    if sibling_parent == current_parent {
                        is_last = false;
                        break;
                    }
                }
            }
            nodes[i].is_last_child = is_last;
        }

        Ok(App {
            nodes,
            animation_depth: 0,
            animation_complete: false,
            stats,
            root_path: path,
            scroll_offset: 0,
            selected_index: None,
            animation_frame: 0,
        })
    }

    fn increment_animation(&mut self) {
        if self.animation_depth <= self.stats.max_depth {
            self.animation_depth += 1;
        } else {
            self.animation_complete = true;
        }
        // Increment frame for smooth animation within current rendering
        if !self.animation_complete {
            self.animation_frame = (self.animation_frame + 1) % 3;
        }
    }

    fn is_node_visible(&self, node: &FileNode) -> bool {
        node.depth <= self.animation_depth
    }

    fn handle_mouse_click(&mut self, row: u16, area: Rect) {
        if !self.animation_complete {
            return;
        }

        // Calculate which item was clicked (accounting for borders and scroll)
        if row > area.top() && row < area.bottom() - 1 {
            let clicked_index = (row - area.top() - 1) as usize + self.scroll_offset;
            let visible_nodes: Vec<_> = self.nodes.iter()
                .filter(|n| self.is_node_visible(n))
                .collect();
            if clicked_index < visible_nodes.len() {
                let node = visible_nodes[clicked_index];
                if node.is_dir {
                    // Open directory in default file manager
                    let _ = opener::open(&node.path);
                }
                // Find the actual index in the nodes vector
                for (idx, n) in self.nodes.iter().enumerate() {
                    if n.path == node.path {
                        self.selected_index = Some(idx);
                        break;
                    }
                }
            }
        }
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn scroll_down(&mut self, visible_lines: usize) {
        let visible_count = self.nodes.iter()
            .filter(|n| self.is_node_visible(n))
            .count();
        let max_scroll = visible_count.saturating_sub(visible_lines);
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
    
    // First, collect all visible nodes with their index in the full list
    let all_visible: Vec<(usize, &FileNode)> = app
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| app.is_node_visible(n))
        .collect();
    
    let visible_nodes: Vec<ListItem> = all_visible
        .iter()
        .enumerate()
        .skip(app.scroll_offset)
        .take(visible_height)
        .map(|(list_idx, (actual_index, node))| {
            // Build tree connectors
            let mut tree_prefix = String::new();
            
            if node.depth > 0 {
                // For each depth level before the current node's depth,
                // determine if we need to show a vertical line
                for ancestor_depth in 1..node.depth {
                    // Get the ancestor path at the checking level (cached)
                    let ancestor_path = node.path.ancestors().nth(node.depth - ancestor_depth);
                    
                    // Check if there's a node after current one at same ancestor level
                    let has_more = all_visible
                        .iter()
                        .skip(list_idx + 1)
                        .any(|(_, future_node)| {
                            if future_node.depth < ancestor_depth {
                                return false;
                            }
                            let future_ancestor_path = future_node.path.ancestors()
                                .nth(future_node.depth - ancestor_depth);
                            
                            ancestor_path == future_ancestor_path
                        });
                    
                    if has_more {
                        tree_prefix.push_str("│   ");
                    } else {
                        tree_prefix.push_str("    ");
                    }
                }
                
                // Determine connector for current node
                let base_connector = if node.is_last_child {
                    "╰── " // Last child uses corner
                } else {
                    "├── " // Not last child uses tee
                };
                
                // Animation effect: show growing roots
                if !app.animation_complete && node.depth == app.animation_depth {
                    let prefix = if node.is_last_child { "╰" } else { "├" };
                    match app.animation_frame % 3 {
                        0 => tree_prefix.push_str(&format!("{}─", prefix)),
                        1 => tree_prefix.push_str(&format!("{}──", prefix)),
                        _ => tree_prefix.push_str(base_connector),
                    }
                } else {
                    tree_prefix.push_str(base_connector);
                }
            }
            
            // Use Nerd Font icons instead of emojis
            let icon = if node.depth == 0 {
                "" // nf-fa-seedling (root folder icon)
            } else {
                "" // nf-fa-folder (folder icon)
            };

            let display_name = if node.name.is_empty() {
                node.path.to_string_lossy().to_string()
            } else {
                node.name.clone()
            };

            let mut style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);

            if app.selected_index == Some(*actual_index) {
                style = style.bg(Color::DarkGray);
            }

            // Color the tree connectors differently
            let connector_style = Style::default().fg(Color::Green);
            let icon_style = style;
            
            let line = Line::from(vec![
                Span::styled(tree_prefix, connector_style),
                Span::styled(format!("{} {}", icon, display_name), icon_style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let visible_count = app.nodes.iter().filter(|n| app.is_node_visible(n)).count();
    let title = format!(
        " {} ({}/{}) - Depth {}/{} ",
        if app.animation_complete {
            "" // nf-fa-tree (completed tree)
        } else {
            "" // nf-fa-spinner (growing)
        },
        visible_count,
        app.nodes.len(),
        app.animation_depth,
        app.stats.max_depth
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
            " Statistics",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Path: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(app.root_path.to_string_lossy().to_string()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " Folders: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}", app.stats.total_dirs),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Files: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{}", app.stats.total_files),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " Total Items: ",
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
                " Total Size: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                humansize::format_size(app.stats.total_size, humansize::BINARY),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Max Depth: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{}", app.stats.max_depth),
                Style::default().fg(Color::Blue),
            ),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![Span::styled(
            " Controls:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::raw(" ↑/↓ - Scroll")]),
        Line::from(vec![Span::raw(" PgUp/PgDn - Fast scroll")]),
        if app.animation_complete {
            Line::from(vec![Span::styled(
                " Click folder - Open",
                Style::default().fg(Color::Green),
            )])
        } else {
            Line::from(vec![Span::styled(
                " Wait for animation...",
                Style::default().fg(Color::DarkGray),
            )])
        },
        Line::from(vec![Span::raw(" Q/Esc - Quit")]),
    ];

    let paragraph = Paragraph::new(stats_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("  Info ")
            .style(Style::default().fg(Color::Green)),
    );

    f.render_widget(paragraph, area);
}
