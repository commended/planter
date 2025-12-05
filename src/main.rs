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

// Icon constants
const ICON_ROOT: &str = ""; // nf-fa-seedling
const ICON_FOLDER: &str = ""; // nf-fa-folder
const ICON_FILE: &str = ""; // file icon
const ICON_TREE_COMPLETE: &str = ""; // nf-fa-tree
const ICON_SPINNER: &str = ""; // nf-fa-spinner

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
    preview_contents: Vec<PreviewItem>,
    preview_scroll_offset: usize,
    last_click_time: Option<Instant>,
    last_click_index: Option<usize>,
}

#[derive(Clone)]
struct PreviewItem {
    name: String,
    is_dir: bool,
    size: u64,
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

        let mut app = App {
            nodes,
            animation_depth: 0,
            animation_complete: false,
            stats,
            root_path: path,
            scroll_offset: 0,
            selected_index: None,
            animation_frame: 0,
            preview_contents: Vec::new(),
            preview_scroll_offset: 0,
            last_click_time: None,
            last_click_index: None,
        };
        
        // Select the first folder by default
        if !app.nodes.is_empty() {
            app.selected_index = Some(0);
            app.update_preview(0);
        }
        
        Ok(app)
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
    
    fn is_double_click(&self, idx: usize, now: Instant) -> bool {
        if let (Some(last_time), Some(last_idx)) = (self.last_click_time, self.last_click_index) {
            last_idx == idx && now.duration_since(last_time) < Duration::from_millis(500)
        } else {
            false
        }
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
                
                // Find the actual index in the nodes vector
                let mut actual_index = None;
                for (idx, n) in self.nodes.iter().enumerate() {
                    if n.path == node.path {
                        actual_index = Some(idx);
                        break;
                    }
                }
                
                if let Some(idx) = actual_index {
                    let now = Instant::now();
                    let is_double_click = self.is_double_click(idx, now);
                    
                    if is_double_click {
                        // Second click on same item - open it
                        if node.is_dir {
                            let _ = opener::open(&node.path);
                        }
                        // Reset click tracking after opening
                        self.last_click_time = None;
                        self.last_click_index = None;
                    } else {
                        // First click - select it
                        self.selected_index = Some(idx);
                        self.update_preview(idx);
                        self.last_click_time = Some(now);
                        self.last_click_index = Some(idx);
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
    
    fn get_visible_node_indices(&self) -> Vec<usize> {
        self.nodes.iter()
            .enumerate()
            .filter(|(_, n)| self.is_node_visible(n))
            .map(|(idx, _)| idx)
            .collect()
    }
    
    fn select_previous(&mut self) {
        let visible_nodes = self.get_visible_node_indices();
        
        if visible_nodes.is_empty() {
            return;
        }
        
        if let Some(current) = self.selected_index {
            // Find current position in visible nodes
            if let Some(pos) = visible_nodes.iter().position(|&idx| idx == current) {
                if pos > 0 {
                    // Move to previous visible node
                    let new_idx = visible_nodes[pos - 1];
                    self.selected_index = Some(new_idx);
                    self.update_preview(new_idx);
                }
            }
        }
    }
    
    fn select_next(&mut self) {
        let visible_nodes = self.get_visible_node_indices();
        
        if visible_nodes.is_empty() {
            return;
        }
        
        if let Some(current) = self.selected_index {
            // Find current position in visible nodes
            if let Some(pos) = visible_nodes.iter().position(|&idx| idx == current) {
                if pos < visible_nodes.len() - 1 {
                    // Move to next visible node
                    let new_idx = visible_nodes[pos + 1];
                    self.selected_index = Some(new_idx);
                    self.update_preview(new_idx);
                }
            }
        }
    }
    
    fn ensure_selected_visible(&mut self, visible_lines: usize) {
        if let Some(selected_idx) = self.selected_index {
            let visible_nodes = self.get_visible_node_indices();
            
            if let Some(pos) = visible_nodes.iter().position(|&idx| idx == selected_idx) {
                // Scroll up if selected is above visible area
                if pos < self.scroll_offset {
                    self.scroll_offset = pos;
                }
                // Scroll down if selected is below visible area
                else if pos >= self.scroll_offset + visible_lines {
                    self.scroll_offset = pos.saturating_sub(visible_lines - 1);
                }
            }
        }
    }

    fn update_preview(&mut self, node_index: usize) {
        // Clear preview first
        self.preview_contents.clear();
        self.preview_scroll_offset = 0;
        
        if node_index >= self.nodes.len() {
            return;
        }
        
        let node_path = &self.nodes[node_index].path;
        
        if let Ok(entries) = fs::read_dir(node_path) {
            let mut items: Vec<PreviewItem> = entries
                .filter_map(|entry| entry.ok())
                .map(|entry| {
                    let path = entry.path();
                    let is_dir = path.is_dir();
                    let size = if !is_dir {
                        fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
                    } else {
                        0
                    };
                    PreviewItem {
                        name: entry.file_name().to_string_lossy().to_string(),
                        is_dir,
                        size,
                    }
                })
                .collect();
            
            // Sort directories first, then files, alphabetically within each group
            items.sort_by(|a, b| {
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.cmp(&b.name),
                }
            });
            
            self.preview_contents = items;
        }
    }

    fn scroll_preview_up(&mut self) {
        if self.preview_scroll_offset > 0 {
            self.preview_scroll_offset -= 1;
        }
    }

    fn scroll_preview_down(&mut self, lines: usize) {
        if self.preview_contents.is_empty() {
            return;
        }
        let max_offset = self.preview_contents.len().saturating_sub(1);
        self.preview_scroll_offset = (self.preview_scroll_offset + lines).min(max_offset);
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
                let area_height = terminal.size()?.height.saturating_sub(4) as usize;
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Up => {
                        app.scroll_up();
                    }
                    KeyCode::Down => {
                        app.scroll_down(area_height);
                    }
                    KeyCode::Left => app.scroll_preview_up(),
                    KeyCode::Right => app.scroll_preview_down(1),
                    KeyCode::PageUp => {
                        for _ in 0..10 {
                            app.scroll_up();
                        }
                    }
                    KeyCode::PageDown => {
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
                        .margin(0)
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
        .margin(0)
        .split(f.area());

    // Left panel: Tree view
    render_tree(f, app, chunks[0]);

    // Right panel: Split into stats and preview
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .margin(0)
        .split(chunks[1]);

    // Top right: Statistics
    render_stats(f, app, right_chunks[0]);
    
    // Bottom right: Folder contents preview
    render_preview(f, app, right_chunks[1]);
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
                        tree_prefix.push_str("│ ");
                    } else {
                        tree_prefix.push_str("  ");
                    }
                }
                
                // Determine connector for current node
                let base_connector = if node.is_last_child {
                    "╰─ " // Last child uses corner
                } else {
                    "├─ " // Not last child uses tee
                };
                
                // Animation effect: show growing roots
                if !app.animation_complete && node.depth == app.animation_depth {
                    let prefix = if node.is_last_child { "╰" } else { "├" };
                    match app.animation_frame % 3 {
                        0 => tree_prefix.push_str(&format!("{}", prefix)),
                        1 => tree_prefix.push_str(&format!("{}─", prefix)),
                        _ => tree_prefix.push_str(base_connector),
                    }
                } else {
                    tree_prefix.push_str(base_connector);
                }
            }
            
            // Use Nerd Font icons instead of emojis
            let icon = if node.depth == 0 {
                ICON_ROOT
            } else {
                ICON_FOLDER
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
            ICON_TREE_COMPLETE
        } else {
            ICON_SPINNER
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
        Line::from(vec![
            Span::styled(" Folders: ", Style::default().add_modifier(Modifier::BOLD)),
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
                " Total Size: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                humansize::format_size(app.stats.total_size, humansize::BINARY),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Max Depth: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{}", app.stats.max_depth),
                Style::default().fg(Color::Blue),
            ),
        ]),
        Line::from(vec![Span::styled(
            " Controls:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::raw(" ↑/↓ - Navigate selection")]),
        Line::from(vec![Span::raw(" ←/→ - Scroll preview")]),
        if app.animation_complete {
            Line::from(vec![Span::styled(
                " Click - Select/Open",
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

fn render_preview(f: &mut Frame, app: &App, area: Rect) {
    let visible_height = area.height.saturating_sub(2) as usize;
    
    let preview_items: Vec<ListItem> = app.preview_contents
        .iter()
        .skip(app.preview_scroll_offset)
        .take(visible_height)
        .map(|item| {
            let icon = if item.is_dir {
                ICON_FOLDER
            } else {
                ICON_FILE
            };
            
            let size_str = if item.is_dir {
                String::new()
            } else {
                format!(" ({})", humansize::format_size(item.size, humansize::BINARY))
            };
            
            let style = if item.is_dir {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            
            let line = Line::from(vec![
                Span::styled(format!(" {} {}{}", icon, item.name, size_str), style),
            ]);
            
            ListItem::new(line)
        })
        .collect();
    
    let title = if let Some(idx) = app.selected_index {
        if let Some(node) = app.nodes.get(idx) {
            format!(" {} {} ({} items) ", ICON_FOLDER, node.name, app.preview_contents.len())
        } else {
            format!(" {} Folder Contents ", ICON_FOLDER)
        }
    } else {
        format!(" {} Folder Contents (Click to select) ", ICON_FOLDER)
    };
    
    let list = List::new(preview_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().fg(Color::Green)),
    );
    
    f.render_widget(list, area);
}
