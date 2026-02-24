# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-02-24

### Added
- Task assignment support for shared projects (`--assign` / `--unassign` on `td add` and `td edit`)
- `--assigned-to` filter flag on `td list` (supports `me`, `others`, or a collaborator name)
- `td collaborators` command to list collaborators on a shared project
- Filter DSL support for `assigned to:`, `assigned by:`, `assigned`, and `!assigned` expressions
- Assignee display in task list (`[@name]`) and task details (`td show`)
- Cache collaborators and collaborator states from Sync API
- Fuzzy collaborator resolution by name or email with ambiguity detection
- `--assign me` resolves to the current user
- CLI E2E test for the full assignment workflow

## [0.1.4] - 2026-02-22

### Fixed
- Execute mutation commands with current sync token instead of stale token
- Parse user timezone from `tz_info` object to match Todoist API format

### Changed
- Shift default E2E suite to lean CLI-focused workflows
- Consolidate extended E2E tests to reduce rate limit pressure

## [0.1.3] - 2026-02-02

### Added
- License and security checking with cargo-deny in CI
- Third-party license attribution file (THIRD_PARTY_LICENSES.html)
- E2E tests to CI pipeline (runs on main branch only)
- Project guidelines documentation (AGENTS.md)

### Changed
- Use `SyncCommandType` enum instead of string literals for type safety
- Extract retry logic from client.rs for better maintainability
- Extract sync_manager lookups to separate module
- Make `TodoistClientBuilder::build()` return `Result` for better error handling

### Fixed
- Align due date e2e tests with actual Todoist API behavior
- Remove `unwrap()` and `unreachable!()` from lookups.rs

### Performance
- Add async I/O methods for cache persistence
- Pre-allocate collections to reduce reallocations
- Add HashMap indexes and optimize merge operations

## [0.1.2] - 2026-01-15

### Added
- Homebrew installation support

## [0.1.1] - 2026-01-10

### Added
- Initial release
- `td add` command for creating tasks
- `td list` command with filtering support
- Filter parser with date keyword support
- Local cache with SyncManager for offline reads
- Quick add endpoint support
- Rate limiting with auto-retry and exponential backoff
- Secure credential storage via system keyring

[0.2.0]: https://github.com/LuoAndOrder/todoist-rs/compare/v0.1.4...v0.2.0
[0.1.4]: https://github.com/LuoAndOrder/todoist-rs/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/LuoAndOrder/todoist-rs/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/LuoAndOrder/todoist-rs/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/LuoAndOrder/todoist-rs/releases/tag/v0.1.1
