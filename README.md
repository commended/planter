# planter

A CLI tool written in Rust that animates the growth of a folder tree structure in your terminal, with real-time statistics displayed alongside.

## Features

- 🌱 **Animated Tree Growth**: Watch your directory structure grow from root to leaves
- 📊 **Live Statistics**: Real-time display of files, directories, total size, and depth
- 🖱️ **Interactive**: Click on folders to open them in your default file manager (after animation completes)
- ⌨️ **Keyboard Navigation**: Scroll through large directory trees with arrow keys and page up/down
- 🎨 **Beautiful UI**: Color-coded tree view with icons and styled statistics panel

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
- **PgUp/PgDn**: Fast scroll (10 lines at a time)
- **Mouse Click**: Click on a folder to open it in your default file manager (only works after animation completes)
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
2. Displays an animated "growth" of the tree from root to leaves
3. Shows real-time statistics including:
   - Total directories and files
   - Total size (in human-readable format)
   - Maximum depth
   - Current path
4. After animation completes, click any folder to open it in your system's default file manager

## Requirements

- Rust 1.70 or higher
- Terminal with mouse support for click interactions

## License

MIT License - see LICENSE file for details

