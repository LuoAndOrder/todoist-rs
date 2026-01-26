# td - Todoist CLI Specification

## Overview

`td` is a Rust command-line interface for the Todoist API, designed for both human users and AI agents. It provides full coverage of the Todoist v1 API with local caching, rich output formatting, and excellent developer experience.

## Goals

1. **Complete API Coverage** - Expose all Todoist v1 API capabilities via CLI commands
2. **AI Agent Friendly** - Auto-detect TTY for JSON output, strict exit codes, machine-parseable errors
3. **Fast & Reliable** - <100ms warm start target, local caching with sync, auto-retry on rate limits
4. **Sync-First Architecture** - Use Sync API for efficient incremental updates and batched operations
5. **Excellent UX** - Color-coded output, intuitive commands, shell completions, helpful error messages

## Non-Goals

- Interactive TUI (text user interface)
- Team/workspace collaboration features (personal use focus)
- Offline-first operation (cache is read-only fallback, not offline editing)
- GUI wrapper or web interface

---

## Architecture

### Project Structure

```
todoist-rs/
├── Cargo.toml                 # Workspace definition
├── crates/
│   ├── td/                    # Binary crate - CLI application
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── commands/      # Command implementations
│   │       ├── output/        # Formatting (table, JSON)
│   │       └── config.rs      # Configuration handling
│   │
│   ├── todoist-api/           # Library crate - API client
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs      # HTTP client wrapper
│   │       ├── models/        # API data types
│   │       ├── endpoints/     # API endpoint implementations
│   │       └── error.rs       # Error types
│   │
│   └── todoist-cache/         # Library crate - Local cache
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── store.rs       # Cache storage
│           └── filter.rs      # Local filter parsing
│
├── tests/                     # Integration tests
└── specs/                     # This specification
```

### Technology Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust 1.93+ (Edition 2021) | Performance, safety, excellent CLI ecosystem |
| CLI Framework | clap (derive) | Industry standard, auto-generated help/completions |
| Async Runtime | tokio | Industry standard, required by reqwest |
| HTTP Client | reqwest | Mature, async, easy to use |
| Serialization | serde + serde_json | Standard Rust JSON handling |
| Config Format | TOML | Human-readable, Rust-native |
| Terminal Colors | owo-colors or colored | ANSI color support |
| Date/Time | chrono | Date parsing and formatting |
| Keyring | keyring | Cross-platform secret storage |

---

## Sync-First Architecture

### Why Sync API?

The Todoist v1 API provides both REST endpoints and a Sync endpoint (`POST /api/v1/sync`). This CLI uses the **Sync API as the primary mechanism** because it aligns perfectly with local caching:

| Feature | Sync API | REST API |
|---------|----------|----------|
| Incremental updates | ✓ Via sync_token | ✗ Full refetch |
| Batch operations | ✓ Multiple commands | ✗ One request per op |
| All resources at once | ✓ Single request | ✗ Multiple endpoints |
| Temp ID references | ✓ Cross-command refs | ✗ Sequential only |
| Command idempotency | ✓ UUID-based | ✗ Requires X-Request-Id |

### Sync Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                         Initial Sync                             │
│  sync_token='*' + resource_types='["all"]'                       │
│  → Returns all data + new sync_token                             │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                       Cache Populated                            │
│  Store: sync_token, projects, items, labels, sections, etc.      │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                      Incremental Sync                            │
│  sync_token=<stored> + resource_types='["all"]'                  │
│  → Returns only changes + new sync_token                         │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                      Write Operations                            │
│  commands=[{type, uuid, temp_id, args}, ...]                     │
│  → Returns sync_status + temp_id_mapping + new sync_token        │
└─────────────────────────────────────────────────────────────────┘
```

### REST API Usage (Exceptions)

Some operations still use REST endpoints when Sync API doesn't provide equivalent functionality:

| Operation | Endpoint | Reason |
|-----------|----------|--------|
| Quick Add | `POST /api/v1/tasks/quick_add` | NLP parsing server-side |
| File Upload | `POST /api/v1/attachments` | Binary upload |
| Get Completed Tasks | `GET /api/v1/tasks/completed` | Historical data not in sync |

### Command Batching

Write operations are batched into single sync requests when possible:

```rust
// Example: Complete multiple tasks in one request
let commands = vec![
    SyncCommand::item_close("task-id-1"),
    SyncCommand::item_close("task-id-2"),
    SyncCommand::item_close("task-id-3"),
];
client.sync(commands).await?;
```

### Temporary IDs

When creating related resources, use temp_ids to reference them within the same batch:

```rust
// Create project and add task in one request
let project_temp_id = uuid::Uuid::new_v4().to_string();
let commands = vec![
    SyncCommand::project_add("Shopping List", &project_temp_id),
    SyncCommand::item_add("Buy milk", &project_temp_id), // References temp_id
];
```

---

## Configuration

### File Locations (XDG Spec)

| File | Path | Purpose |
|------|------|---------|
| Config | `~/.config/td/config.toml` | User settings and API token |
| Cache | `~/.cache/td/cache.json` | Cached Todoist data |

### Config File Format

```toml
# ~/.config/td/config.toml

# API token (can also use TODOIST_TOKEN env var or keyring)
token = "your-api-token-here"

# Token storage method: "config", "keyring", or "env"
token_storage = "config"

# Output preferences
[output]
color = true              # Enable colors (respects NO_COLOR env)
date_format = "relative"  # "relative", "iso", "short"

# Cache settings
[cache]
enabled = true
```

### Token Resolution Order

1. `--token` command line flag (highest priority)
2. `TODOIST_TOKEN` environment variable
3. OS keyring (if `token_storage = "keyring"`)
4. Config file `token` field

### First-Run Experience

When no token is found:
1. Prompt user: "Enter your Todoist API token (from https://todoist.com/app/settings/integrations/developer):"
2. Ask storage preference: config file or OS keyring
3. Write config and perform initial sync

---

## Command Structure

### Global Flags

```
-h, --help           Show help information
-V, --version        Show version
-v, --verbose        Verbose output
-q, --quiet          Minimal output (errors only)
--json               Force JSON output
--no-color           Disable colors
--token <TOKEN>      Override API token
```

### Commands Overview

| Command | Alias | Description |
|---------|-------|-------------|
| `td list` | `td l` | List tasks |
| `td add` | `td a` | Add a task |
| `td show` | `td s` | Show task details |
| `td edit` | `td e` | Edit a task |
| `td done` | `td d` | Complete a task |
| `td reopen` | | Reopen a completed task |
| `td delete` | `td rm` | Delete a task |
| `td today` | `td t` | Show today's tasks |
| `td quick` | `td q` | Quick add with natural language |
| `td sync` | | Sync local cache |
| `td projects` | `td p` | List/manage projects |
| `td labels` | `td lb` | List/manage labels |
| `td sections` | | List/manage sections |
| `td comments` | | List/manage comments |
| `td reminders` | | List/manage reminders |
| `td filters` | | List/manage saved filters |
| `td config` | | View/edit configuration |
| `td completions` | | Generate shell completions |

---

## Command Details

### Task Commands

#### `td list` - List Tasks

```
td list [OPTIONS]

Options:
  -f, --filter <FILTER>    Filter expression (e.g., "today & p1")
  -p, --project <NAME>     Filter by project name or ID
  -l, --label <NAME>       Filter by label
  -P, --priority <1-4>     Filter by priority (1=highest)
  --section <NAME>         Filter by section
  --overdue                Show only overdue tasks
  --no-due                 Show only tasks without due date
  --limit <N>              Limit results (default: 50)
  --all                    Show all tasks (no limit)
  --cursor <TOKEN>         Pagination cursor for programmatic use
  --sort <FIELD>           Sort by: due, priority, created, project
  --reverse                Reverse sort order
```

**Output (table mode):**
```
ID       Pri  Due          Project     Labels      Content
a1b2c3   p1   Today 3pm    Work        @urgent     Review PR #123
d4e5f6   p2   Tomorrow     Personal                Buy groceries
g7h8i9   p4   No date      Inbox                   Read article
```

**Output (JSON mode):**
```json
{
  "tasks": [...],
  "cursor": "next_page_token",
  "has_more": true
}
```

#### `td add` - Add Task

```
td add <CONTENT> [OPTIONS]

Arguments:
  <CONTENT>                Task content/title

Options:
  -p, --project <NAME>     Target project (default: Inbox)
  -P, --priority <1-4>     Priority level (1=highest, 4=lowest)
  -d, --due <DATE>         Due date (natural language or ISO)
  -l, --label <NAME>       Add label (repeatable)
  --section <NAME>         Target section within project
  --parent <ID>            Parent task ID (creates subtask)
  --description <TEXT>     Task description/notes
```

**Example:**
```bash
td add "Review PR #123" -p Work -P 1 -d tomorrow -l urgent
```

#### `td show` - Show Task Details

```
td show <TASK_ID>

Options:
  --comments               Include comments
  --reminders              Include reminders
```

**Output:**
```
Task: Review PR #123
ID: a1b2c3d4e5f6
Project: Work
Section: In Progress
Priority: p1 (highest)
Due: Tomorrow at 3:00 PM
Labels: @urgent, @review
Created: 2026-01-20 10:30
Description:
  Check the implementation of the new auth flow.
  Focus on security concerns.

Subtasks (2):
  - [x] Review auth middleware
  - [ ] Check token validation
```

#### `td edit` - Edit Task

```
td edit <TASK_ID> [OPTIONS]

Options:
  -c, --content <TEXT>     Update content
  -p, --project <NAME>     Move to project
  -P, --priority <1-4>     Change priority
  -d, --due <DATE>         Change due date
  --no-due                 Remove due date
  -l, --label <NAME>       Set labels (replaces existing)
  --add-label <NAME>       Add label
  --remove-label <NAME>    Remove label
  --section <NAME>         Move to section
  --description <TEXT>     Update description
```

#### `td done` - Complete Task

```
td done <TASK_ID>... [OPTIONS]

Arguments:
  <TASK_ID>...             One or more task IDs

Options:
  --all-occurrences        Complete all future occurrences (recurring tasks)
  -f, --force              Skip confirmation for multiple tasks
```

**Example:**
```bash
td done a1b2c3              # Complete single task
td done a1b2c3 d4e5f6       # Complete multiple tasks
td done a1b2c3 --all-occurrences  # Complete recurring task entirely
```

#### `td reopen` - Reopen Task

```
td reopen <TASK_ID>...
```

#### `td delete` - Delete Task

```
td delete <TASK_ID>... [OPTIONS]

Options:
  -f, --force              Skip confirmation prompt
```

**Without --force:**
```
Are you sure you want to delete "Review PR #123"? [y/N]
```

#### `td today` - Today's Agenda

```
td today [OPTIONS]

Options:
  --include-overdue        Include overdue tasks (default: true)
  --include-upcoming <N>   Include tasks due within N days
```

**Output:**
```
Today's Tasks (3 tasks)

OVERDUE
  p1  Yesterday    Review PR #123                    Work

DUE TODAY
  p2  Today 2pm    Team standup                      Work
  p3  Today        Buy groceries                     Personal

UPCOMING (next 3 days)
  p4  Tomorrow     Dentist appointment               Personal
```

#### `td quick` - Quick Add (Natural Language)

```
td quick <TEXT>

Arguments:
  <TEXT>                   Natural language task description
```

Uses Todoist's server-side natural language processing.

**Example:**
```bash
td quick "Call mom tomorrow at 5pm #Personal p2"
```

### Project Commands

#### `td projects` - List Projects

```
td projects [SUBCOMMAND]

Subcommands:
  list                     List all projects (default)
  add <NAME>               Create new project
  show <ID>                Show project details
  edit <ID>                Edit project
  archive <ID>             Archive project
  unarchive <ID>           Unarchive project
  delete <ID>              Delete project
```

#### `td projects add`

```
td projects add <NAME> [OPTIONS]

Options:
  --color <COLOR>          Project color
  --parent <ID>            Parent project ID
  --favorite               Mark as favorite
```

### Label Commands

#### `td labels`

```
td labels [SUBCOMMAND]

Subcommands:
  list                     List all labels (default)
  add <NAME>               Create new label
  edit <ID>                Edit label
  delete <ID>              Delete label
```

### Section Commands

#### `td sections`

```
td sections [SUBCOMMAND]

Options:
  -p, --project <NAME>     Filter by project

Subcommands:
  list                     List sections (default)
  add <NAME>               Create section (requires --project)
  edit <ID>                Edit section
  delete <ID>              Delete section
```

### Comment Commands

#### `td comments`

```
td comments [SUBCOMMAND]

Options:
  --task <ID>              Comments for task
  --project <ID>           Comments for project

Subcommands:
  list                     List comments (default)
  add <TEXT>               Add comment
  edit <ID>                Edit comment
  delete <ID>              Delete comment
  attach <FILE>            Attach file to comment
  download <ATTACHMENT_ID> Download attachment
```

### Reminder Commands

#### `td reminders`

```
td reminders [SUBCOMMAND]

Options:
  --task <ID>              Reminders for task

Subcommands:
  list                     List reminders (default)
  add                      Create reminder
  delete <ID>              Delete reminder
```

### Utility Commands

#### `td sync`

```
td sync [OPTIONS]

Options:
  --full                   Force full sync (ignore cache)
```

#### `td config`

```
td config [SUBCOMMAND]

Subcommands:
  show                     Show current configuration
  edit                     Open config in $EDITOR
  set <KEY> <VALUE>        Set configuration value
  path                     Print config file path
```

#### `td completions`

```
td completions <SHELL>

Arguments:
  <SHELL>                  bash, zsh, fish, powershell
```

**Example:**
```bash
td completions zsh > ~/.zfunc/_td
```

---

## Output Formatting

### TTY Detection

- **Interactive terminal**: Table format with colors
- **Piped/redirected**: JSON format (machine-readable)
- Override with `--json` or `--no-color` flags

### Table Format

- Clean columns with fixed-width ID prefixes
- Color-coded priorities (p1=red, p2=orange, p3=yellow, p4=blue)
- Due date highlighting (overdue=red, today=yellow, upcoming=default)
- Project names in distinct colors
- Labels prefixed with `@`

### JSON Format

All list commands output:
```json
{
  "tasks": [...],
  "cursor": "pagination_token",
  "has_more": boolean
}
```

Single item commands output the item directly:
```json
{
  "id": "...",
  "content": "...",
  ...
}
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | User error (invalid arguments, missing required fields) |
| 2 | API error (auth failure, not found, validation error) |
| 3 | Network error (connection failed, timeout) |
| 4 | Rate limited (with retry-after information) |
| 5 | Configuration error |

### Error Output

Errors are written to stderr in a consistent format:

**Human-readable:**
```
Error: Task not found

The task ID "abc123" does not exist or has been deleted.
Try 'td list' to see available tasks.
```

**JSON (when --json flag):**
```json
{
  "error": {
    "code": "NOT_FOUND",
    "message": "Task not found",
    "details": "The task ID 'abc123' does not exist",
    "suggestion": "Try 'td list' to see available tasks"
  }
}
```

---

## Filter Syntax

The CLI implements local parsing of Todoist's filter syntax for validation and offline filtering of cached data.

### Supported Filter Expressions

| Filter | Description |
|--------|-------------|
| `today` | Due today |
| `tomorrow` | Due tomorrow |
| `overdue` | Past due date |
| `no date` | No due date set |
| `p1`, `p2`, `p3`, `p4` | Priority level |
| `@label` | Has label |
| `#project` | In project |
| `##project` | In project or subprojects |
| `/section` | In section |
| `assigned to: me` | Assigned to current user |
| `created: today` | Created today |

### Boolean Operators

| Operator | Meaning |
|----------|---------|
| `&` | AND |
| `\|` | OR |
| `!` | NOT |
| `()` | Grouping |

**Example:**
```bash
td list --filter "(today | overdue) & p1"
```

---

## Caching

### Cache Structure

The cache stores the complete sync state, mirroring the Sync API response:

```json
{
  "sync_token": "TnYUZEpuzf2FMA9qzyY3j4xky6dXiYejmSO85S5paZ_...",
  "full_sync_date_utc": "2026-01-25T10:30:00Z",
  "last_sync": "2026-01-25T10:30:00Z",
  "user": {...},
  "projects": [...],
  "items": [...],
  "labels": [...],
  "sections": [...],
  "filters": [...],
  "reminders": [...],
  "notes": [...],
  "collaborators": [...],
  "day_orders": {...},
  "completed_info": [...]
}
```

Note: The Sync API calls tasks "items" and comments "notes". The cache uses these names to match the API response directly.

### Cache Behavior

1. **Initial sync** (`sync_token='*'`) - Fetch all data, populate cache
2. **Incremental sync** - Use stored `sync_token` to fetch only changes
3. **Read operations** - Read from cache, optionally refresh if stale (>5 min)
4. **Write operations** - Send commands via Sync API, update cache with response
5. **`td sync`** - Force incremental sync (or `--full` for complete refresh)

### Sync Strategy

```
┌────────────────────────────────────────────────────────────────┐
│                      Read Operations                            │
│                                                                  │
│  Cache exists?  ─────No────→  Initial sync (sync_token='*')     │
│       │                                                          │
│      Yes                                                         │
│       │                                                          │
│  Cache stale?  ─────Yes───→  Incremental sync (stored token)    │
│  (>5 min old)                                                    │
│       │                                                          │
│      No                                                          │
│       ↓                                                          │
│  Return cached data                                              │
└────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────┐
│                      Write Operations                           │
│                                                                  │
│  1. Build command(s) with UUID + optional temp_id               │
│  2. Send to Sync API                                             │
│  3. Check sync_status for success/failure per command            │
│  4. Resolve temp_id_mapping for new resources                    │
│  5. Update cache with new sync_token                             │
│  6. Apply changes to cached data                                 │
└────────────────────────────────────────────────────────────────┘
```

### Cache Invalidation

The Sync API handles invalidation naturally:
- Each sync response includes a new `sync_token`
- Incremental sync returns only changed/deleted resources
- Deleted resources are indicated by `is_deleted: true`

### Offline Handling

If API is unreachable:
- **Read operations** - Fall back to cache with warning (data may be stale)
- **Write operations** - Fail with network error (no offline queue)

---

## Task ID Handling

### Display

Task IDs are shown as 6-character prefixes of the full UUID:
```
a1b2c3   Buy groceries
```

### Input

Users can specify tasks by:
1. Full UUID: `a1b2c3d4-e5f6-7890-abcd-ef1234567890`
2. Unique prefix: `a1b2c3` (or shorter if unique)

If prefix is ambiguous:
```
Error: Ambiguous task ID "a1b"

Multiple tasks match this prefix:
  a1b2c3  Buy groceries
  a1b4d5  Review code

Please use a longer prefix.
```

---

## API Integration

### Base URL

```
https://api.todoist.com/api/v1
```

### Primary Endpoint: Sync API

Most operations use the Sync endpoint:

```
POST https://api.todoist.com/api/v1/sync
Content-Type: application/x-www-form-urlencoded
Authorization: Bearer <token>
```

#### Read Operations (Sync)

```
sync_token=<token_or_*>&resource_types=["all"]
```

#### Write Operations (Commands)

```
commands=[{"type":"item_add","uuid":"...","temp_id":"...","args":{...}}]
```

### Authentication

All requests include:
```
Authorization: Bearer <token>
```

### Rate Limiting

- Auto-retry with exponential backoff on 429 responses
- Respect `Retry-After` header
- Maximum 3 retries before failing

### Command Idempotency

Write operations use command UUIDs for idempotency:
- Each command has a unique `uuid` field
- Todoist will not re-execute a command with the same UUID
- Safe to retry failed requests without duplicate operations

### Sync Command Types

| Resource | Add | Update | Delete | Other |
|----------|-----|--------|--------|-------|
| Tasks | `item_add` | `item_update` | `item_delete` | `item_close`, `item_uncomplete`, `item_move` |
| Projects | `project_add` | `project_update` | `project_delete` | `project_archive`, `project_unarchive` |
| Sections | `section_add` | `section_update` | `section_delete` | `section_move`, `section_archive` |
| Labels | `label_add` | `label_update` | `label_delete` | `label_update_orders` |
| Filters | `filter_add` | `filter_update` | `filter_delete` | `filter_update_orders` |
| Reminders | `reminder_add` | `reminder_update` | `reminder_delete` | |
| Comments | `note_add` | `note_update` | `note_delete` | `project_note_add`, etc. |

---

## Date/Time Handling

### Input Formats

Natural language (parsed by Todoist API via quick add):
- "today", "tomorrow", "next monday"
- "in 2 hours", "at 3pm"
- "jan 25", "2026-01-25"

ISO 8601 (for --due flag):
- `2026-01-25`
- `2026-01-25T15:00:00`

### Display Formats

**Relative (default for near dates):**
- "Today 3pm"
- "Tomorrow"
- "Yesterday" (overdue)
- "In 3 days"

**Absolute (for far dates):**
- "Jan 25, 2026"
- "Jan 25, 2026 3:00 PM"

### Timezone

Respects user's timezone setting from Todoist account.

---

## Security

### Token Storage

1. **Config file**: `~/.config/td/config.toml`
   - File permissions enforced: 600 (owner read/write only)
   - Warning if permissions are too open

2. **OS Keyring** (optional):
   - macOS: Keychain
   - Linux: libsecret/GNOME Keyring
   - Windows: Credential Manager

3. **Environment variable**: `TODOIST_TOKEN`

### Sensitive Data

- Tokens never logged, even in verbose mode
- Cache file contains no authentication data
- Config file path printed, not contents, in error messages

---

## Testing Strategy

### Unit Tests

- Filter parser correctness
- Date formatting logic
- ID prefix matching
- Output formatting

### Integration Tests

- API client with mock server (wiremock-rs)
- Cache read/write operations
- Config file handling

### End-to-End Tests (Optional)

- Against real Todoist API with test account
- Requires `TODOIST_TEST_TOKEN` environment variable
- Run with `cargo test --features e2e`

---

## CI/CD

### GitHub Actions Workflow

```yaml
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test
      - run: cargo clippy -- -D warnings
      - run: cargo fmt --check

  release:
    if: startsWith(github.ref, 'refs/tags/')
    needs: test
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
          - target: x86_64-apple-darwin
          - target: aarch64-apple-darwin
          - target: x86_64-pc-windows-msvc
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: softprops/action-gh-release@v1
        with:
          files: target/${{ matrix.target }}/release/td*
```

### Release Process

1. Update version in `Cargo.toml` files
2. Update CHANGELOG.md
3. Create git tag: `git tag v0.1.0`
4. Push tag: `git push origin v0.1.0`
5. GitHub Actions builds and uploads binaries

---

## Distribution

### Initial Release

- GitHub Releases with pre-built binaries
  - Linux (x86_64, aarch64)
  - macOS (x86_64, aarch64/Apple Silicon)
  - Windows (x86_64)

### Future Distribution Channels

- crates.io (`cargo install td`)
- Homebrew formula
- AUR package
- Debian/RPM packages

---

## Dependencies

### Minimum Dependency Set

```toml
[workspace.dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# Async
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
reqwest = { version = "0.12", features = ["json"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Utilities
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2"
directories = "5"  # XDG paths

# Optional
keyring = "3"  # Secret storage
owo-colors = "4"  # Terminal colors
```

---

## Appendix A: Command Quick Reference

```
TASKS
  td list [-f FILTER] [-p PROJECT] [--limit N]    List tasks
  td add CONTENT [-p PROJECT] [-P PRIORITY]       Add task
  td show ID                                      Show task details
  td edit ID [OPTIONS]                            Edit task
  td done ID...                                   Complete task(s)
  td reopen ID...                                 Reopen task(s)
  td delete ID... [-f]                            Delete task(s)
  td today                                        Today's agenda
  td quick TEXT                                   Quick add (NLP)

PROJECTS
  td projects                                     List projects
  td projects add NAME                            Create project
  td projects show ID                             Show project
  td projects edit ID                             Edit project
  td projects archive ID                          Archive project
  td projects delete ID                           Delete project

LABELS
  td labels                                       List labels
  td labels add NAME                              Create label
  td labels delete ID                             Delete label

SECTIONS
  td sections [-p PROJECT]                        List sections
  td sections add NAME -p PROJECT                 Create section

COMMENTS
  td comments --task ID                           List task comments
  td comments add TEXT --task ID                  Add comment
  td comments attach FILE --task ID               Attach file

REMINDERS
  td reminders --task ID                          List reminders
  td reminders add --task ID                      Create reminder

UTILITY
  td sync [--full]                                Sync with Todoist
  td config show                                  Show configuration
  td completions SHELL                            Generate completions
```

---

## Appendix B: Environment Variables

| Variable | Description |
|----------|-------------|
| `TODOIST_TOKEN` | API token (overrides config file) |
| `NO_COLOR` | Disable colors when set |
| `TD_CONFIG` | Override config file path |
| `TD_CACHE` | Override cache file path |

---

## Appendix C: API Endpoint Mapping

### Sync API Operations (Primary)

Most operations use the Sync API endpoint: `POST /api/v1/sync`

| CLI Command | Sync Command(s) |
|-------------|-----------------|
| `td list` | Read from cache (sync if stale) |
| `td add` | `item_add` |
| `td show` | Read from cache |
| `td edit` | `item_update` (and/or `item_move`) |
| `td done` | `item_close` |
| `td reopen` | `item_uncomplete` |
| `td delete` | `item_delete` |
| `td projects list` | Read from cache |
| `td projects add` | `project_add` |
| `td projects edit` | `project_update` |
| `td projects archive` | `project_archive` |
| `td projects delete` | `project_delete` |
| `td labels list` | Read from cache |
| `td labels add` | `label_add` |
| `td labels delete` | `label_delete` |
| `td sections add` | `section_add` |
| `td sections delete` | `section_delete` |
| `td comments add` | `note_add` |
| `td comments delete` | `note_delete` |
| `td reminders add` | `reminder_add` |
| `td reminders delete` | `reminder_delete` |
| `td sync` | Incremental sync (or full with `--full`) |

### REST API Operations (Exceptions)

| CLI Command | REST Endpoint | Reason |
|-------------|---------------|--------|
| `td quick` | `POST /api/v1/tasks/quick_add` | NLP parsing server-side |
| File attachments | `POST /api/v1/attachments` | Binary upload |
| Completed tasks | `GET /api/v1/tasks/completed` | Historical data |

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 0.1.0 | TBD | Initial specification |
