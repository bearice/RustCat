# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.4.1] - 2026-07-13

### Fixed

- **Launcher icon not showing on KDE/GNOME.** The `.desktop` entry references
  `Icon=rustcat`, but the `.deb`, `.rpm`, and portable tarball all shipped
  `assets/appIcon.ico` as `usr/share/pixmaps/rustcat.ico`. The freedesktop
  icon-name lookup does not support `.ico`, so launchers fell back to a generic
  icon. All three packaging paths now install the existing 256×256
  `assets/rustcat.png` into
  `usr/share/icons/hicolor/256x256/apps/rustcat.png`, and `install.sh` refreshes
  the icon cache. The AppImage already used the PNG and was unaffected.
- **About menu item did nothing.** The about message used `\\n` (literal
  backslash-n) instead of a real newline, so the dialog rendered an embedded
  `\n` (cross-platform — Windows `MessageBoxW` was affected too). On Linux,
  `show_dialog` used fire-and-forget `spawn()` and returned `Ok` as soon as
  `kdialog` spawned, without checking whether it actually displayed; if
  `kdialog` spawned but exited non-zero (no display / D-Bus session),
  `zenity`/`xmessage` were never tried and the user saw nothing. Switched to
  blocking `status()` so a non-zero exit falls through to the next tool;
  `status()` also reaps the child, so no zombie accumulates.
- **Per-second journal log spam when launched from a desktop entry.** The
  animation thread printed `CPU Usage: ... speed: ...` every second via
  `println!`, which lands in the systemd journal when stdout is not a TTY.
  Added `src/logging.rs` with a `debug!` macro that only emits informational
  logs when stdout is a TTY or `RUSTCAT_DEBUG` / `RUST_LOG` is set (resolved
  once via `OnceLock`). Gated all routine informational prints behind it;
  genuine error `eprintln!` are left unchanged so failures still surface in the
  journal. To see logs when debugging: `RUSTCAT_DEBUG=1 rust_cat` or run
  `rust_cat` directly in a terminal.

## [2.4.0] - 2026-07-07

### Added

- Linux/KDE support via the freedesktop StatusNotifierItem (SNI) protocol over
  D-Bus — the tray cat now runs natively on KDE Plasma (and other SNI-aware
  trays)
- Linux platform module mirroring the Windows/macOS architecture: CPU usage
  from `/proc/stat`, settings persisted to `~/.config/rustcat/settings.conf`,
  autostart via a freedesktop `~/.config/autostart/rustcat.desktop` file
- Native KDE integration: dark/light theme detection via `kreadconfig`,
  dialogs via `kdialog`, system monitor via `plasma-systemmonitor`
- Nix flake (`flake.nix`) using crane, with `packages`, `devShells`, and `apps`
  outputs; runtime helper tools wrapped onto `PATH`
- Distro packaging for Linux: `.deb` (cargo-deb), `.rpm` (cargo-generate-rpm),
  and a portable AppImage (linuxdeploy), all built in CI alongside the
  portable `.tar.gz` bundle
- Linux packages are built for both `x86_64` and `aarch64` (arm64) in CI,
  using native GitHub arm64 runners — no cross-compilation
- `assets/rustcat.desktop` freedesktop entry, `assets/rustcat.png` launcher
  icon, and `build_linux.sh` build script

### Changed

- Scoped the `crt-static` rustflag to Windows only (`.cargo/config.toml`); it
  was intended for MSVC static linking and forced static-glibc linking on
  Linux, which most distros (and Nix) don't provide
- Refactored `ui_update` in `app.rs` to apply to all non-macOS targets
  (Windows + Linux), since the trayicon D-Bus backend is thread-safe
- Added standard crates.io metadata (description, license, repository,
  keywords, categories) to `Cargo.toml`

### Fixed

- Linux builds now link dynamically against glibc instead of failing on
  missing static libc

## [2.3.0] - 2025-07-10

### Added

- Auto theme support for macOS that follows system wallpaper color
- Template icon support for macOS allowing icons to adapt to system theme
  automatically
- I put an easter egg there, can you find it? 😉

### Fixed

- Fixed version extraction in macOS build script causing sed command failures
- Improved version parsing to handle multiple version entries in Cargo.toml

### Technical Improvements

- Consolidated macOS build scripts into single comprehensive build_macos.sh
  script
- Enhanced macOS build process reliability and error handling
- Removed separate build_app_icon.sh and create_dmg.sh scripts in favor of
  unified approach
- Added Auto theme option to macOS theme menu

## [2.2.2] - 2025-07-09

### Fixed

- Fixed random crash caused by race condition in tray icon updates

## [2.2.1] - 2025-07-09

### Changed

- Upgraded objc2 dependencies to 0.6.x for improved macOS compatibility
- Removed unsafe block from macOS implementation

### Performance

- Optimized icon and theme settings caching to reduce system calls
- Improved overall application responsiveness through reduced OS interactions

## [2.2.0] - 2025-07-09

### Added

- Comprehensive macOS support with platform abstraction
- macOS-specific system integration (defaults, LaunchAgents, Activity Monitor)
- Proper macOS NSApplication integration for native behavior
- Automatic dark/light theme detection on macOS

### Changed

- Cross-platform architecture with Windows/macOS abstraction
- Platform-specific settings storage (Windows Registry vs macOS defaults)
- Platform-specific startup behavior (Windows Run key vs macOS LaunchAgents)
- Platform-specific system monitor integration (Task Manager vs Activity
  Monitor)

### Fixed

- Missing event loop in Windows implementation
- Cross-platform compatibility issues

### Technical Improvements

- Added platform-specific conditional compilation
- Improved code organization with platform abstractions
- Enhanced error handling for cross-platform operations

## [2.1.0] - 2025-07-08

### Added

- Icon compression system using gzip for significantly reduced binary size
- Aggressive compiler optimizations for release builds (size-focused)
- Runtime icon decompression with efficient memory management
- Updated application icon with improved design

### Changed

- Build system now compresses all icons into single chunk for maximum efficiency
- Icon loading refactored to use compressed data with offset/size metadata
- Release profile optimized for minimal binary size (opt-level="z", LTO, strip
  symbols)

### Technical Improvements

- Added flate2 dependency for compression support
- Implemented single-chunk compression reduces icon storage overhead
- Memory-efficient decompression keeps data alive for application lifetime
- Build script generates optimized icon metadata at compile time

### Performance

- Significant binary size reduction through icon compression
- Faster application startup through optimized release builds
- Reduced memory fragmentation with single allocation for all icons

## [2.0.0] - 2025-07-08

### Added

- Dynamic menu system for extensible icon management
- String-based icon system with automatic migration
- Graceful thread shutdown functionality
- About dialog with version and Git hash information
- "Run on Start" option for automatic startup
- Proper error handling for unsafe Windows API calls

### Changed

- **BREAKING**: Migrated to windows-rs crate for improved Windows API
  integration
- **BREAKING**: Refactored main.rs into modular structure
- Improved code quality and error handling throughout the application
- Updated error messages to use eprintln! instead of println!

### Fixed

- Right-click context menu not showing in tray icon
- Lifetime issues in App animation thread
- CPU usage panic with proper error handling
- Various compile errors and code formatting issues

### Dependencies

- Updated trayicon from 0.1.3 to 0.2.0
- Updated winreg from 0.10.1 to 0.55.0
- Migrated from older Windows API bindings to windows-rs 0.58

## [1.0.5] - Previous Release

### Added

- Initial stable release with animated cat and parrot icons
- CPU usage monitoring with animation speed correlation
- Windows theme detection (light/dark)
- Registry integration for settings persistence
- Tray icon with context menu
