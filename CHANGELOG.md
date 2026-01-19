# Changelog

All notable changes to Arta will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Nothing yet

### Changed
- Nothing yet

### Fixed
- Nothing yet

---

## [0.4.0] - 2025-01-19

### Added
- **Container Feature** - Sandboxed execution environments with isolated context
  - `CREATE CONTAINER` - Create a new container with optional settings
  - `SWITCH CONTAINER` - Switch between containers
  - `LIST CONTAINERS` - List all available containers
  - `DESTROY CONTAINER` - Remove a container
  - `EXPORT CONTAINER` - Export container state to a file
  - Container options: `ALLOW ACTIONS`, `READONLY`
- Container-aware REPL with container prompt indicator
- `--container` CLI flag for `run` and `repl` commands
- `containers` CLI subcommand to list all containers
- 3 new container example scripts

### Changed
- Updated CI/CD workflows for automated releases
- Version bumped to 0.4.0

### Fixed
- Fixed GitHub Actions workflow (corrected `dtolnay/rust-toolchain` action name)
- Fixed all clippy warnings for CI compliance
- Applied `cargo fmt` for consistent code style

---

## [0.3.0] - 2025-01-18

### Added
- **LIFE Monitoring** - Real-time reactive monitoring of system resources
  - `LIFE MONITOR BATTERY DO ... END LIFE`
  - `LIFE MONITOR CPU DO ... END LIFE`
  - `LIFE MONITOR MEMORY DO ... END LIFE`
- FOR loops for iterating over query results
- IF/ELSE conditional statements
- Variable system with `LET` declarations
- PRINT command for output
- Script validation before execution
- Context navigation (`ENTER FOLDER`, `ENTER FILE`, `EXIT`)
- `SHOW CONTEXT`, `SHOW VARIABLES`, `SHOW HISTORY` commands

### Changed
- Improved error messages
- Enhanced query output formatting

---

## [0.2.0] - 2025-01-17

### Added
- **Script Support** - Execute `.arta` script files
- **Actions** - `DELETE FILES` and `KILL PROCESS` commands
- `--dry-run` mode for previewing changes
- `--allow-actions` flag for enabling destructive operations
- `EXPLAIN` command for understanding queries
- JSON output format with `--json` flag

### Security
- Actions require explicit `--allow-actions` flag
- WHERE clause required for DELETE operations
- Protected system processes cannot be killed
- Maximum items per operation limit

---

## [0.1.0] - 2025-01-16

### Added
- Initial release
- SQL-like query syntax for system state
- Query targets:
  - `SELECT CPU *` - CPU information
  - `SELECT MEMORY *` - Memory usage
  - `SELECT DISK *` - Disk information
  - `SELECT NETWORK *` - Network interfaces
  - `SELECT SYSTEM *` - System details
  - `SELECT BATTERY *` - Battery status
  - `SELECT PROCESS *` - Process listing
  - `SELECT FILES *` - File listing
  - `SELECT CONTENT *` - File content
- WHERE clause filtering
- Field selection
- Human-readable and JSON output
- Cross-platform support (Linux, macOS, Windows)
- Interactive REPL mode (with `--features repl`)

---

[Unreleased]: https://github.com/Khalidabdi1/arta/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/Khalidabdi1/arta/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/Khalidabdi1/arta/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Khalidabdi1/arta/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Khalidabdi1/arta/releases/tag/v0.1.0
