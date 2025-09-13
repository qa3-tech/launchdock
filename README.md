# LaunchDock

A fast, cross-platform application launcher with fuzzy search that helps you quickly find and launch applications from anywhere on your system.

## Features

- **Cross-Platform**: Works on Windows, Linux, and macOS
- **Daemon Architecture**: Lightweight background service for instant response
- **Smart Fuzzy Search**: Intelligent character matching with proximity-based scoring
- **Custom Hotkeys**: Configure any key combination to show the launcher
- **Auto-Discovery**: Automatically finds applications in standard system directories
- **Clean Interface**: Minimal, distraction-free design
- **Icon Support**: Platform-specific icon loading with smart fallbacks
- **System Integration**: Works with your OS's application management

## Installation

### Prerequisites

Ensure you have Rust 1.89+ installed on your system.

### From Source

```bash
git clone https://github.com/qa3-tech/launchdock.git
cd launchdock
cargo build --release
cargo install --path .
```

### From Binaries

Download the latest release for your platform from the [releases page](https://github.com/qa3-tech/launchdock/releases).

## Setup

### 1. Start the Daemon

Add LaunchDock to your system startup so it runs automatically:

```bash
launchdock start
```

### 2. Configure Hotkeys

Set up your preferred keyboard shortcut in your OS hotkey settings to run: `launchdock show`

**Platform-specific instructions:**

- **Windows**: Settings → System → Keyboard → Advanced keyboard settings → App shortcuts
- **macOS**: System Preferences → Keyboard → Shortcuts → App Shortcuts
- **Linux (GNOME)**: Settings → Keyboard → Custom Shortcuts
- **Linux (KDE)**: System Settings → Shortcuts → Custom Shortcuts

**Suggested hotkey combinations:**

- `Super+Space` (recommended)
- `Ctrl+Alt+Space`
- `Meta+Escape`
- Any combination that works for your workflow

### 3. Verify Setup

```bash
# Check daemon status
launchdock status

# Test your configured hotkey
# Should show the launcher interface
```

## Usage

### Basic Commands

```bash
# Start/stop the daemon
launchdock start
launchdock stop

# Show the launcher
launchdock show

# Check status
launchdock status

# View logs
launchdock logs
launchdock logs clear
```

### Using the Launcher

1. Press your configured hotkey to show the launcher
2. Type to search for applications (fuzzy matching supported)
3. Use arrow keys or number shortcuts (1-7) to select
4. Press Enter to launch, or Escape to close

**Search Examples:**

- Type `fx` to find Firefox
- Type `gv` to find applications like "Gnome Video" or "GoodVibes"
- Type `code` to find VS Code, Visual Studio Code, etc.

## How It Works

LaunchDock uses intelligent fuzzy search that matches characters in order but not necessarily consecutively. The search algorithm considers:

- **Character proximity**: Closer matches rank higher
- **Application name length**: Shorter names get slight preference
- **Early matches**: Matches at the beginning of names score higher
- **Consecutive characters**: Sequential character matches get bonus points

This means typing `psg` will find "Photoshop Graphics" before "Photo Studio Gallery" because the characters are closer together.

## Platform Support

### Linux

- Scans `/usr/share/applications/`, user applications, and desktop entries
- Supports `.desktop` files, AppImages, and executables
- Icon loading from standard theme directories

### macOS

- Scans `/Applications/`, system apps, and user applications
- Supports `.app` bundles with comprehensive icon discovery
- Searches multiple icon naming patterns and formats

## Configuration

LaunchDock works out of the box with zero configuration. It automatically:

- Discovers applications in standard system directories
- Finds and loads application icons
- Generates fallback icons for apps without icons

## Building from Source

### Development Setup

```bash
git clone https://github.com/qa3-tech/launchdock.git
cd launchdock

# Build debug version
cargo build

# Run tests
cargo test

# Build optimized release
cargo build --release

# Install locally
cargo install --path .
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint code
cargo clippy

# Run with debug logging
RUST_LOG=debug cargo run -- start
```

## Troubleshooting

### Daemon Issues

```bash
# Check if running
launchdock status

# View error logs
launchdock logs

# Restart daemon
launchdock stop && launchdock start
```

### Missing Applications

- Ensure apps are installed in standard directories
- Restart daemon to refresh: `launchdock stop && launchdock start`
- Check logs to see scan results: `launchdock logs`
- Verify system permissions for application directories

### Performance

- Clear large log files: `launchdock logs clear`
- Check for repeated errors: `launchdock logs | grep ERROR`

### Platform-Specific

**Linux**: Ensure desktop entries are properly installed
**macOS**: Grant permissions in System Preferences → Security & Privacy if needed

## Architecture

LaunchDock uses a clean client-server architecture:

- **CLI Client**: Handles commands and communicates with daemon
- **Background Daemon**: Manages application discovery and UI lifecycle
- **UI Module**: Cross-platform launcher interface with Iced framework
- **Model Layer**: Application data structures and fuzzy search logic

## Contributing

Contributions are welcome! Please see our dual licensing model below.

### Development Workflow

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Make your changes and add tests
4. Ensure code quality: `cargo fmt && cargo clippy`
5. Commit: `git commit -m 'Add amazing feature'`
6. Push: `git push origin feature/amazing-feature`
7. Open a Pull Request

## License

This project uses a dual licensing model:

- **Open Source**: Licensed under GPL-3.0 for open source projects
- **Commercial**: Separate commercial license required for proprietary use

Contact contact@qa3.tech for commercial licensing inquiries.

See [LICENSE](LICENSE) for complete terms.

---

**LaunchDock** - Launch applications faster, across all platforms.

Copyright © 2025 QA3 Technologies LLC
