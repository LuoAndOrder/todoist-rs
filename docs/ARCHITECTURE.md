# Architecture Documentation

This document provides technical details about the architecture of the `td` CLI and its supporting crates.

## Table of Contents

- [Crate Structure](#crate-structure)
- [Cache Architecture](#cache-architecture)
- [Sync Token Handling](#sync-token-handling)
- [Error Handling](#error-handling)
- [Filter Parser](#filter-parser)

---

## Crate Structure

The project is organized as a Cargo workspace with three crates:

```
todoist-rs/
├── crates/
│   ├── todoist-api/    # Low-level API client
│   ├── todoist-cache/  # Local cache and sync management
│   └── td/             # CLI application
└── Cargo.toml          # Workspace root
```

### `todoist-api`

The API client crate provides:

- **`TodoistClient`** (`client.rs`): HTTP client wrapper with automatic retry and rate limit handling
- **Sync API types** (`sync/`): Request/response models for the Todoist Sync API
- **Quick Add** (`quick_add.rs`): NLP-based task creation endpoint
- **Error types** (`error.rs`): Structured error handling with exit codes

Key features:
- Exponential backoff with `Retry-After` header support
- 30-second request timeout
- Up to 3 retry attempts for rate-limited requests

### `todoist-cache`

The cache crate provides:

- **`Cache`** (`lib.rs`): In-memory representation of Todoist data
- **`CacheStore`** (`store.rs`): Persistent JSON storage with XDG paths
- **`SyncManager`** (`sync_manager.rs`): Orchestrates sync operations
- **Filter parser** (`filter/`): Local filter evaluation

### `td`

The CLI application provides:

- Command-line interface via `clap`
- Output formatting (text, JSON, IDs-only)
- Credential management (keyring, environment, config file)
- Shell completions

---

## Cache Architecture

### Storage Location

The cache uses XDG-compliant paths:

| Platform | Path |
|----------|------|
| Linux    | `~/.cache/td/cache.json` |
| macOS    | `~/Library/Caches/td/cache.json` |
| Windows  | `C:\Users\<User>\AppData\Local\td\cache\cache.json` |

### Cache Structure

```rust
pub struct Cache {
    pub sync_token: String,           // Token for incremental sync
    pub full_sync_date_utc: Option<DateTime<Utc>>,
    pub last_sync: Option<DateTime<Utc>>,
    pub items: Vec<Item>,             // Tasks
    pub projects: Vec<Project>,
    pub labels: Vec<Label>,
    pub sections: Vec<Section>,
    pub notes: Vec<Note>,             // Task comments
    pub project_notes: Vec<ProjectNote>,
    pub reminders: Vec<Reminder>,
    pub filters: Vec<Filter>,         // Saved filters
    pub user: Option<User>,
}
```

### Staleness Detection

The `SyncManager` determines when a sync is needed:

```
Cache is stale if:
  - Never synced (last_sync is None)
  - OR last_sync > 5 minutes ago
```

The 5-minute threshold is configurable via `SyncManager::with_stale_threshold()`.

### Atomic Writes

Cache persistence uses atomic writes to prevent corruption:

1. Write to temporary file (`cache.json.tmp`)
2. Rename to final location (`cache.json`)

This ensures the cache file is never partially written if the process crashes.

---

## Sync Token Handling

### How Sync Tokens Work

The Todoist Sync API uses opaque sync tokens to enable incremental synchronization:

1. **Full sync**: Send `sync_token="*"` to receive all data and a new token
2. **Incremental sync**: Send the stored token to receive only changes since that token was issued
3. **Token refresh**: Each response includes a new token that must be stored

### Sync Token Flow

```
┌──────────────┐                    ┌──────────────┐
│   Client     │                    │  Todoist API │
└──────────────┘                    └──────────────┘
       │                                   │
       │  sync_token="*"                   │
       │  resource_types=["all"]           │
       │──────────────────────────────────>│
       │                                   │
       │  full_sync=true                   │
       │  sync_token="abc123..."           │
       │  items=[...], projects=[...]      │
       │<──────────────────────────────────│
       │                                   │
       │  (store sync_token)               │
       │                                   │
       │  sync_token="abc123..."           │
       │  resource_types=["all"]           │
       │──────────────────────────────────>│
       │                                   │
       │  full_sync=false                  │
       │  sync_token="def456..."           │
       │  items=[changed items only]       │
       │<──────────────────────────────────│
```

### Invalid Token Recovery

If the API rejects a sync token (token expired or corrupted), the client automatically recovers:

```rust
// In SyncManager::sync()
match self.client.sync(request).await {
    Ok(response) => { /* apply response */ }
    Err(e) if e.is_invalid_sync_token() => {
        // Reset and perform full sync
        self.cache.sync_token = "*".to_string();
        let request = SyncRequest::full_sync();
        let response = self.client.sync(request).await?;
        // ...
    }
    Err(e) => Err(e.into()),
}
```

### Mutation Responses

Write operations (add, update, delete) also return sync responses:

1. Send commands via `SyncRequest::with_commands()`
2. API returns affected resources and a new sync token
3. Apply changes via `Cache::apply_mutation_response()`

The mutation response handler always uses incremental merge logic, even if `full_sync: true` is returned, because mutations only contain affected resources.

### Merge Algorithm

For incremental syncs, resources are merged using O(n+m) complexity:

```rust
fn merge_resources<T>(existing: &mut Vec<T>, incoming: &[T], get_id, is_deleted) {
    // 1. Build index: id -> position
    // 2. For each incoming item:
    //    - If deleted: mark for removal
    //    - If exists: update in place
    //    - Otherwise: append
    // 3. Remove deleted items in reverse order
}
```

---

## Error Handling

### Error Hierarchy

```
Error (todoist-api)
├── Api(ApiError)
│   ├── Http { status, message }
│   ├── Auth { message }
│   ├── RateLimit { retry_after }
│   ├── NotFound { resource, id }
│   ├── Validation { field, message }
│   └── Network { message }
├── Http(reqwest::Error)
├── Json(serde_json::Error)
└── Internal(String)

SyncError (todoist-cache)
├── Cache(CacheStoreError)
├── Api(todoist_api::Error)
├── NotFound { resource_type, identifier }
└── SyncTokenInvalid
```

### Exit Codes

The CLI uses standardized exit codes:

| Code | Meaning | Examples |
|------|---------|----------|
| 0 | Success | Command completed |
| 1 | User error | Invalid arguments |
| 2 | API error | Auth failure, validation, not found |
| 3 | Network error | Connection failed, timeout |
| 4 | Rate limited | Too many requests |

### Retry Strategy

Rate-limited requests (HTTP 429) are retried with exponential backoff:

```
Attempt 0: 1 second (or Retry-After header)
Attempt 1: 2 seconds
Attempt 2: 4 seconds
Attempt 3: (max retries exceeded)
```

Backoff is capped at 30 seconds. The `Retry-After` header value takes precedence when present.

### Sync Token Validation

Invalid sync tokens are detected by checking error messages:

```rust
fn is_invalid_sync_token(&self) -> bool {
    match self {
        ApiError::Validation { message, .. } => {
            let msg_lower = message.to_lowercase();
            msg_lower.contains("sync_token")
                || msg_lower.contains("sync token")
                || msg_lower.contains("invalid token")
        }
        _ => false,
    }
}
```

---

## Filter Parser

### Architecture

The filter parser uses a classic lexer-parser-evaluator pipeline:

```
Input String
     │
     ▼
┌─────────┐    ┌─────────┐    ┌───────────┐
│  Lexer  │───>│ Parser  │───>│ Evaluator │───> Filtered Items
└─────────┘    └─────────┘    └───────────┘
   tokens         AST
```

### Components

| Module | Responsibility |
|--------|----------------|
| `lexer.rs` | Tokenizes input into keywords, operators, identifiers |
| `parser.rs` | Builds abstract syntax tree with operator precedence |
| `ast.rs` | Filter enum representing parsed expressions |
| `evaluator.rs` | Evaluates filters against items |
| `error.rs` | Parser error types |

### AST Structure

```rust
pub enum Filter {
    // Date filters
    Today,
    Tomorrow,
    Overdue,
    NoDate,
    SevenDays,
    SpecificDate { month: u32, day: u32 },

    // Priority
    Priority(u8),  // 1-4

    // Labels
    Label(String),
    NoLabels,

    // Project/section
    Project(String),
    ProjectWithSubprojects(String),
    Section(String),

    // Boolean operators
    And(Box<Filter>, Box<Filter>),
    Or(Box<Filter>, Box<Filter>),
    Not(Box<Filter>),
}
```

### Evaluation Context

The evaluator requires context to resolve names to IDs:

```rust
pub struct FilterContext<'a> {
    projects: &'a [Project],
    labels: &'a [Label],
    sections: &'a [Section],
}
```

This allows filters like `#Work` to match against project names.

### Supported Syntax

| Syntax | Description |
|--------|-------------|
| `today` | Tasks due today |
| `tomorrow` | Tasks due tomorrow |
| `overdue` | Tasks past due |
| `no date` | Tasks without due date |
| `7 days` | Tasks due within 7 days |
| `Jan 15` | Tasks due on specific date |
| `p1`-`p4` | Priority levels |
| `@label` | Tasks with label |
| `no labels` | Tasks without labels |
| `#project` | Tasks in project |
| `##project` | Tasks in project + subprojects |
| `/section` | Tasks in section |
| `&` | AND operator |
| `\|` | OR operator |
| `!` | NOT operator |
| `()` | Grouping |

### Example Usage

```rust
use todoist_cache::filter::{FilterParser, FilterEvaluator, FilterContext};

// Parse filter
let filter = FilterParser::parse("today & p1")?;

// Create context with cached data
let context = FilterContext::new(&cache.projects, &cache.labels, &cache.sections);

// Create evaluator
let evaluator = FilterEvaluator::new(&filter, &context);

// Filter items
let high_priority_today = evaluator.filter_items(&cache.items);
```

---

## Design Decisions

### Why Local Cache?

1. **Performance**: Avoids network round-trips for read operations
2. **Offline support**: Read operations work without internet
3. **Reduced API calls**: Incremental sync minimizes data transfer
4. **Filter evaluation**: Complex filters evaluated locally

### Why Sync API vs REST API?

Todoist provides two APIs:

- **REST API**: Simple CRUD, one resource at a time
- **Sync API**: Bulk operations, incremental sync, atomic transactions

We use the Sync API because:
1. Efficient bulk reads with incremental updates
2. Atomic multi-resource operations
3. Better suited for caching architecture

### Why XDG Paths?

XDG Base Directory Specification provides:
1. User expectation alignment on Linux/macOS
2. Clear separation of cache vs config
3. Standard location for CLI tools

### Why Atomic Writes?

File corruption can occur if:
1. Process crashes mid-write
2. Power loss during write
3. Disk full condition

Atomic rename ensures the file is either:
- The old complete version, or
- The new complete version

Never a partial write.
