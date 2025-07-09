# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
- Platform-specific system monitor integration (Task Manager vs Activity Monitor)

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
