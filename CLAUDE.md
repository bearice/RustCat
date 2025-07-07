# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

RustCat is a Windows tray application that displays an animated cat or parrot icon whose animation speed reflects real-time CPU usage. It's a Rust port of RunCat_for_windows, removing the .NET runtime dependency.

## Build and Development Commands

### Building
```bash
# In WSL, use cargo.exe to build Windows binaries
cargo.exe build --release
```

### Running (Development)
```bash
# In WSL, use cargo.exe to run Windows binaries
cargo.exe run
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

- Windows-only application using Windows API extensively via windows-rs crate
- Uses Windows subsystem (no console window in release builds)
- Integrates with Windows registry for settings and startup behavior
- Requires Windows for tray icon functionality and CPU monitoring
- When developing in WSL, use `cargo.exe` instead of `cargo` to build Windows binaries

## Development Environment Notes

- When running in WSL, use `cargo.exe` to build