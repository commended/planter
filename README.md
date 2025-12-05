# planter

A CLI tool written in Rust that animates the horizontal growth of a folder tree structure in your terminal, with real-time statistics displayed alongside.

## Features

-  **Animated Horizontal Tree Growth**: Watch your directory structure grow level by level, from root to deepest folders
-  **Folder-Only View**: Displays only directories for a cleaner, more focused view
-  **Live Statistics**: Real-time display of folders, files, total size, and depth
-  **Folder Contents Preview**: Split-screen view showing the contents of selected folders
-  **Interactive**: Click on folders to preview their contents and open them in your default file manager (after animation completes)
- ⌨️ **Keyboard Navigation**: Scroll through large directory trees with arrow keys and page up/down
-  **Beautiful UI**: Color-coded tree view with Nerd Font icons and styled statistics panel

## Requirements

- Rust 1.70 or higher
- Terminal with mouse support for click interactions
- A Nerd Font installed for proper icon display (recommended: FiraCode Nerd Font, JetBrains Mono Nerd Font, or any Nerd Font)

## Installation

### From Source

```bash
cargo install --path .
```

Or build and run directly:

```bash
cargo build --release
./target/release/planter <directory_path>
```

## Usage

```bash
planter ~/.config
```

Or any directory path:

```bash
planter /path/to/your/folder
```

### Controls

Once the application is running:

- **↑/↓**: Scroll up/down through the tree
- **←/→**: Scroll through folder contents preview
- **PgUp/PgDn**: Fast scroll (10 lines at a time)
- **Mouse Click**: Click on a folder to select it and preview its contents, or open it in your default file manager (only works after animation completes)
- **Q or Esc**: Quit the application

## Example

```bash
# View your home config directory
planter ~/.config

# View your projects folder
planter ~/projects

# View the current directory
planter .
```

## How it Works

1. The tool scans the entire directory structure
2. Displays an animated "growth" of the tree horizontally by depth level
3. Shows only folders (directories), not individual files, for a cleaner view
4. Shows real-time statistics including:
   - Total directories and files
   - Total size (in human-readable format)
   - Maximum depth
   - Current path and animation progress
5. The UI is split into three sections:
   - **Left panel (70%)**: Tree view showing folder hierarchy
   - **Top right panel (15%)**: Statistics and controls
   - **Bottom right panel (15%)**: Preview of selected folder's contents
6. After animation completes, click any folder to preview its contents and open it in your system's default file manager

## Icon Reference

The application uses Nerd Font icons:
-  Root directory (seedling icon)
-  Folders (folder icon)
-  Tree complete (tree icon)
-  Animation in progress (spinner icon)
-  Statistics panel (chart icon)
-  Info panel (info icon)

## License

MIT License - see LICENSE file for details

