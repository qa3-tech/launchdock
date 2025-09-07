# LaunchDock

A fast, cross-platform application launcher that helps you quickly find and launch applications from your system's standard application directories.

## Features

- **Cross-Platform**: Works seamlessly on Windows, Linux, and macOS
- **Daemon Architecture**: Lightweight background service for instant response
- **Global Hotkey**: Meta+Escape to quickly show launcher from anywhere
- **Quick Launch**: Fast application discovery and launching
- **System Integration**: Automatically scans standard application folders
- **Simple CLI**: Easy-to-use command-line interface
- **Show/Hide UI**: Toggle launcher visibility as needed
- **Comprehensive Logging**: Automatic activity logging with easy log management

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/qa3-tech/launchdock.git
cd launchdock

# Build and install
cargo build --release
cargo install --path .
```

### Pre-built Binaries

Download the latest release for your platform from the [releases page](https://github.com/qa3-tech/launchdock/releases).

## Usage

LaunchDock operates as a background daemon that you can control with simple commands:

### Starting the Daemon

```bash
# Start LaunchDock in the background
launchdock start
```

### Basic Commands

```bash
# Show the launcher interface
launchdock show

# Hide the launcher interface
launchdock hide

# Check daemon status
launchdock status

# Stop the daemon
launchdock stop

# View recent logs
launchdock logs

# Clear log file
launchdock logs clear
```

### Global Hotkey

Once the daemon is running, press **Meta+Escape** (Windows/Cmd+Escape on macOS, Super+Escape on Linux) to quickly show the launcher from anywhere.

### Example Workflow

```bash
# Start LaunchDock
$ launchdock start
Starting LaunchDock daemon...

# Check if it's running
$ launchdock status
Daemon: Running
UI: Hidden

# Show the launcher
$ launchdock show
Launcher shown

# Check what's been happening
$ launchdock logs
[2025-01-17 14:30:15] INFO: LaunchDock daemon started
[2025-01-17 14:30:16] INFO: Found 47 applications
[2025-01-17 14:31:22] INFO: Launcher shown
[2025-01-17 14:31:45] INFO: Launching: Firefox

# When done, stop the daemon
$ launchdock stop
LaunchDock stopped
```

## Command Reference

| Command | Description |
|---------|-------------|
| `start` | Start the LaunchDock daemon |
| `stop` | Stop the running daemon |
| `show` | Display the launcher interface |
| `hide` | Hide the launcher interface |
| `status` | Show current daemon and UI status |
| `logs` | View or manage application logs |

### Logs Subcommands

| Subcommand | Description |
|------------|-------------|
| `logs` | Show last 50 log entries (default) |
| `logs show -l <N>` | Show last N log entries |
| `logs clear` | Clear current log file |

## Logging

LaunchDock automatically logs all activities to help with debugging and monitoring. Logs include daemon startup/shutdown, application discovery, launches, and any errors that occur.

### Log File Location

Logs are stored in platform-appropriate directories:

- **Windows**: `%APPDATA%\launchdock\launchdock.log`
- **macOS**: `~/Library/Logs/launchdock/launchdock.log`
- **Linux**: `~/.local/share/launchdock/launchdock.log`

### Managing Logs

```bash
# View recent logs
launchdock logs

# View more log entries
launchdock logs show --lines 100

# Clear log file (recommended when over 5MB)
launchdock logs clear
```

Logs automatically warn when the file exceeds 5MB. You can clear them manually or use external tools like `grep` or `awk` for analysis:

```bash
# Filter for errors only  
launchdock logs | grep ERROR

# Count total log entries
launchdock logs | wc -l

# Search for specific application launches
launchdock logs | grep "Launching:"
```

## Platform Support

### Windows
- Scans `C:\Program Files\`, `C:\Program Files (x86)\`, and Start Menu shortcuts
- Supports `.exe`, `.msi`, and `.appx` applications

### Linux
- Scans `/usr/share/applications/`, `/usr/local/share/applications/`, and `~/.local/share/applications/`
- Supports `.desktop` files and AppImages

### macOS
- Scans `/Applications/`, `/System/Applications/`, and `~/Applications/`
- Supports `.app` bundles and `.dmg` files

## Building from Source

### Prerequisites

- Rust 1.86 or later
- Cargo package manager

### Build Instructions

```bash
# Clone the repository
git clone https://github.com/qa3-tech/launchdock.git
cd launchdock

# Build in debug mode
cargo build

# Build optimized release
cargo build --release

# Run tests
cargo test

# Install locally
cargo install --path .
```

### Development

```bash
# Run in development mode
cargo run -- start

# Run with logging
RUST_LOG=debug cargo run -- start

# Format code
cargo fmt

# Lint code
cargo clippy
```

## Architecture

LaunchDock uses a client-server architecture:

- **Main Process**: Handles CLI commands and communicates with daemon
- **Daemon**: Background service that manages application discovery and UI
- **UI Module**: Cross-platform interface for application selection
- **Model**: Application data structures and business logic

## Configuration

LaunchDock automatically discovers applications from standard system directories. No manual configuration is required for basic usage.

## Troubleshooting

### Daemon Won't Start
```bash
# Check if already running
launchdock status

# Check logs for errors
launchdock logs

# Try stopping first
launchdock stop
launchdock start
```

### Applications Not Appearing
- Ensure applications are installed in standard directories
- Restart the daemon to refresh the application list: `launchdock stop && launchdock start`
- Check system permissions for application directories
- View logs to see what directories are being scanned: `launchdock logs`

### Performance Issues
- Check if log file is too large: `launchdock logs` (warnings appear if >5MB)
- Clear logs if needed: `launchdock logs clear`
- Check logs for repeated errors: `launchdock logs | grep ERROR`

### Platform-Specific Issues

**Windows**: Run as administrator if scanning system directories fails  
**Linux**: Ensure XDG desktop files are properly installed  
**macOS**: Grant necessary permissions in System Preferences > Security & Privacy

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

### Development Setup

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

**LaunchDock** - Launch applications faster, across all platforms.