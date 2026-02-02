# AGENTS.md - Project Guidelines for coding agents

## Project Overview

`td` is a fast, offline-capable command-line interface for Todoist written in Rust. It uses a local cache for instant reads and syncs with the Todoist API on demand.

## Crate Structure

This is a Cargo workspace with three crates:

```
crates/
├── todoist-api/    # Low-level Todoist Sync API client
├── todoist-cache/  # Local cache, sync management, filter parser
└── td/             # CLI application
```

| Crate | Published As | Description |
|-------|--------------|-------------|
| `todoist-api` | `todoist-api-rs` | HTTP client with retry/rate-limit handling, Sync API types |
| `todoist-cache` | `todoist-cache-rs` | Cache storage, SyncManager, filter expression engine |
| `td` | `todoist-cli-rs` | CLI binary using clap, credential management |

## Building

```bash
cargo build              # Debug build
cargo build --release    # Release build
```

The CLI binary is at `target/release/td`.

## Testing

### Unit Tests

```bash
cargo test               # Run all unit tests
cargo test -p todoist-api-rs    # Test specific crate
cargo test -p todoist-cache-rs
cargo test -p todoist-cli-rs
```

### End-to-End Tests

E2E tests run against the real Todoist API and require authentication.

#### Setup

1. Create `.env.local` in the workspace root:
   ```
   TODOIST_TEST_API_TOKEN=your_api_token_here
   ```

2. Get your API token from [Todoist Settings > Integrations > Developer](https://todoist.com/app/settings/integrations/developer)

#### Running E2E Tests

```bash
# Run all e2e tests (MUST use single thread due to shared account + rate limits)
cargo test --features e2e -- --test-threads=1

# Run specific e2e test file
cargo test --features e2e -p todoist-api-rs --test labels_e2e -- --test-threads=1
cargo test --features e2e -p todoist-api-rs --test task_lifecycle_e2e -- --test-threads=1
cargo test --features e2e -p todoist-cache-rs --test cache_e2e -- --test-threads=1

# Run a single test
cargo test --features e2e -p todoist-api-rs --test labels_e2e test_create_label -- --test-threads=1
```

#### Why Single-Threaded?

E2E tests **must** run with `--test-threads=1` because:

1. **Rate limits**: Todoist API allows 100 full syncs / 15 min. Each test initializes a `TestContext` that does a full sync.
2. **Shared account**: All tests operate on the same Todoist account. Parallel execution causes race conditions.
3. **Test isolation**: Tests assume stable account state during execution.

#### E2E Test Files

**todoist-api crate:**
- `api_e2e.rs` - Basic API operations (sync, quick add)
- `task_lifecycle_e2e.rs` - Task CRUD, movement, completion
- `project_e2e.rs` - Project/section CRUD, hierarchy, archiving
- `labels_e2e.rs` - Label CRUD, task labeling
- `comments_e2e.rs` - Task/project comments
- `reminders_e2e.rs` - Reminder operations (requires Todoist Pro)
- `edge_cases_e2e.rs` - Unicode, boundaries, rapid operations

**todoist-cache crate:**
- `cache_e2e.rs` - Cache operations with real API
- `filter_e2e.rs` - Filter evaluation against real data
- `filter_comprehensive_e2e.rs` - Comprehensive filter tests
- `workflow_e2e.rs` - Multi-step workflow scenarios

## Code Conventions

### Command Types

Use `SyncCommandType` enum instead of string literals:

```rust
// Good
SyncCommand::new(SyncCommandType::ItemAdd, args)
SyncCommand::with_temp_id(SyncCommandType::ProjectAdd, &temp_id, args)

// Bad - don't use string literals
SyncCommand::new("item_add", args)
```

### E2E Test Patterns

Use unique names with UUIDs to avoid conflicts from leftover test data:

```rust
let uuid = uuid::Uuid::new_v4().to_string()[..8].to_string();
let label_name = format!("e2e-test-my-label-{}", uuid);
```

### Commit Style

Follow conventional commits:
```
type(scope): description

fix(tests): update e2e tests to use SyncCommandType enum
feat(api): add ItemComplete variant to SyncCommandType
refactor(cache): extract merge logic to separate module
perf(api): pre-allocate collections to reduce reallocations
```

## Key APIs

### SyncCommand

```rust
use todoist_api_rs::sync::{SyncCommand, SyncCommandType};

// Create command with auto-generated UUID
let cmd = SyncCommand::new(SyncCommandType::ItemAdd, serde_json::json!({
    "content": "Task content",
    "project_id": project_id
}));

// Create command with temp_id for referencing in same batch
let temp_id = uuid::Uuid::new_v4().to_string();
let cmd = SyncCommand::with_temp_id(SyncCommandType::ItemAdd, &temp_id, args);
```

### TestContext (E2E tests)

```rust
// Initialize - does ONE full sync
let mut ctx = TestContext::new().await?;

// Get inbox project ID
let inbox_id = ctx.inbox_id().to_string();

// Create resources (uses partial sync internally)
let task_id = ctx.create_task("Content", &inbox_id, None).await?;
let label_id = ctx.create_label("label-name").await?;
let project_id = ctx.create_project("Project Name").await?;

// Execute custom commands
let response = ctx.execute(vec![command]).await?;

// Lookup from cache (no API call)
let task = ctx.find_item(&task_id);
let label = ctx.find_label(&label_id);

// Cleanup
ctx.delete_task(&task_id).await?;
ctx.batch_delete(&task_ids, &project_ids, &section_ids, &label_ids).await?;
```

## Useful Commands

```bash
# Check for compile errors without building
cargo check

# Run clippy lints
cargo clippy --all-targets --all-features

# Format code
cargo fmt

# Generate docs
cargo doc --open

# Run specific test with output
cargo test test_name -- --nocapture
```
