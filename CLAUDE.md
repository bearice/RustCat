# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

RustCat is a cross-platform tray application that displays an animated cat or parrot icon whose animation speed reflects real-time CPU usage. Originally a Windows-only Rust port of RunCat_for_windows, it now supports both Windows and macOS.

## Build and Development Commands

### Building
```bash
# Windows (In WSL, use cargo.exe to build Windows binaries)
cargo.exe build --release

# macOS
cargo build --release
```

### Running (Development)
```bash
# Windows (In WSL, use cargo.exe to run Windows binaries)
cargo.exe run

# macOS
cargo run
```

### Testing
This project does not have automated tests. Testing is done manually by running the application.

## Architecture

### Core Components

- **main.rs**: Main application entry point containing:
  - Tray icon management with menu system
  - Event handling for theme/icon switching
  - CPU usage monitoring thread
  - Icon animation thread
  - Windows registry integration for settings persistence

- **cpu_usage.rs**: CPU usage calculation using Windows API
  - Uses `GetSystemTimes` to calculate CPU usage percentage
  - Provides total CPU time and idle time measurements

- **build.rs**: Build script that:
  - Generates icon resource arrays at build time
  - Embeds Windows resources (app icon)
  - Extracts git hash for version display

### Key Features

- **Animated Icons**: Cat and parrot themes with light/dark variants
- **CPU Monitoring**: Animation speed correlates with CPU usage (higher usage = faster animation)
- **Theme Support**: Automatically detects Windows dark/light theme
- **Registry Integration**:
  - Stores user preferences in `HKEY_CURRENT_USER\Software\RustCat`
  - Supports "Run on Start" functionality via Windows Run registry key
- **Tray Menu**: Right-click menu for theme selection, icon selection, and settings

### Dependencies

- **trayicon**: System tray functionality
- **windows**: Windows API bindings (windows-rs crate)
- **winreg**: Windows registry access
- **winres**: Windows resource embedding (build-time)

### Asset Structure

Icons are organized in `assets/` directory:
- `cat/` and `parrot/` folders contain animation frames
- Each has `light_*` and `dark_*` variants numbered 0-4 (cat) or 0-9 (parrot)
- Build script generates Rust code to embed these as byte arrays

### Platform Specifics

#### Windows
- Uses Windows API extensively via windows-rs crate
- Uses Windows subsystem (no console window in release builds)
- Integrates with Windows registry for settings and startup behavior
- When developing in WSL, use `cargo.exe` instead of `cargo` to build Windows binaries

#### macOS
- Uses macOS NSApplication for proper integration
- Uses macOS defaults command for settings persistence
- Uses macOS LaunchAgents for startup behavior
- Detects system dark/light theme automatically
- Opens Activity Monitor instead of Task Manager

## Development Environment Notes

- When running in WSL, use `cargo.exe` to build

## Release Workflow

When bumping versions and creating releases:

1. **Update version in Cargo.toml**
2. **Update CHANGELOG.md** with detailed release notes including:
   - Added features
   - Changed functionality
   - Fixed bugs
   - Technical improvements
   - Performance enhancements
3. **Commit and tag**:
   ```bash
   git add -A
   git commit -m "Bump version X.Y.Z with [description]"
   git tag -f vX.Y.Z
   git push origin master
   git push origin vX.Y.Z --force
   ```
4. **Create GitHub release**:
   ```bash
   gh release create vX.Y.Z --title "RustCat vX.Y.Z" --notes "[changelog content]"
   ```

This workflow ensures consistent versioning, comprehensive documentation, and proper release management.
