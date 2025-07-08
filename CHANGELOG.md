# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - 2025-07-08

### Added
- Dynamic menu system for extensible icon management
- String-based icon system with automatic migration
- Graceful thread shutdown functionality
- About dialog with version and Git hash information
- "Run on Start" option for automatic startup
- Proper error handling for unsafe Windows API calls

### Changed
- **BREAKING**: Migrated to windows-rs crate for improved Windows API integration
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