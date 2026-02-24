# Spec: Assign Task to User

## Overview

Add the ability to assign tasks to collaborators in shared Todoist projects via the `td` CLI. This feature spans the full stack: caching collaborators, resolving users by name/email, assigning via `--assign` flags, displaying assignees in task output, filtering by assignee, and a new `td collaborators` command.

**Ship as a single release** — all pieces land together.

---

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Error on non-shared project | **Hard error** | Fail with: _"Task is in a personal project — share the project first to assign tasks."_ Don't silently swallow. |
| User identification | **Name or email, fuzzy** | Match against `full_name` or `email`. Ambiguities error with suggestions (non-interactive, scriptable). |
| Command surface | **Flags on edit/add** | `--assign <user>` and `--unassign` on `td edit` and `td add`. No standalone command. Consistent with existing patterns. |
| Display in list | **Show when assigned** | Append `[@Alice]` (or `[@me]`) after labels, only when task has an assignee. |
| Self-display | **Show "me"** | `[@me]` for tasks assigned to the current user, full name for others. |
| Filtering | **CLI flag + filter DSL** | `--assigned-to <user>` flag on `td list`, plus full Todoist-compatible filter syntax. |
| Filter syntax | **Match Todoist exactly** | `assigned to: <name>`, `assigned by: <name>`, `assigned to: me`, `assigned to: others`. |
| Cache strategy | **Cache with sync** | Store collaborators + collaborator_states in the cache file. Updated on every sync like items/projects. |
| Ambiguity handling | **Error with suggestions** | _"Multiple collaborators match 'mar': Alice Smith, Alicia Chen. Please be more specific."_ |
| Collaborators command | **Yes, project-scoped** | `td collaborators --project <name>`. Table with name, email, role/state. |

---

## Scope

### Crate: `todoist-api-rs`

No model changes needed — `Item` already has `responsible_uid` and `assigned_by_uid`; `Collaborator` and `CollaboratorState` structs already exist.

Verify that `SyncCommandType::ItemUpdate` already sends `responsible_uid` through to the API (it should — it's just a JSON arg).

### Crate: `todoist-cache-rs`

#### 1. Cache collaborators

Add to the `Cache` struct:
```rust
pub collaborators: Vec<Collaborator>,
pub collaborator_states: Vec<CollaboratorState>,
```

Update the cache merge logic to process `collaborators` and `collaborator_states` from `SyncResponse` on every sync — same pattern as items, projects, etc.

Add indexes for fast lookup:
- `collaborator_by_id: HashMap<String, usize>` (user_id → index)
- `collaborator_by_project: HashMap<String, Vec<String>>` (project_id → user_ids)

#### 2. Collaborator resolution

Add to `SyncManager` (or a new resolver module):

```rust
pub fn resolve_collaborator(&self, query: &str, project_id: &str) -> Result<&Collaborator>
```

Logic:
1. Get collaborators for the given project (via `collaborator_states` where `state == "active"`)
2. Try exact match on `full_name` (case-insensitive)
3. Try exact match on `email` (case-insensitive)
4. Try prefix/substring match on `full_name` or `email`
5. If 0 matches → error: _"No collaborator matching '\<query\>' in project '\<name\>'"_
6. If 1 match → return it
7. If 2+ matches → error: _"Multiple collaborators match '\<query\>': \<name1\>, \<name2\>. Please be more specific."_

#### 3. Shared project validation

Add helper:
```rust
pub fn is_shared_project(&self, project_id: &str) -> bool
```

A project is shared if it has any active `collaborator_states` entries (more than just the owner). Use this to produce the hard error on assign attempts in personal projects.

#### 4. Filter DSL extensions

Extend the filter AST and parser to support:

| Filter expression | Semantics |
|---|---|
| `assigned to: me` | `responsible_uid == current_user_id` |
| `assigned to: others` | `responsible_uid` is set AND != current_user_id |
| `assigned to: <name>` | `responsible_uid` matches resolved collaborator |
| `assigned by: me` | `assigned_by_uid == current_user_id` |
| `assigned by: others` | `assigned_by_uid` is set AND != current_user_id |
| `assigned by: <name>` | `assigned_by_uid` matches resolved collaborator |
| `assigned` | `responsible_uid` is set (any value) |
| `!assigned` / `no assignee` | `responsible_uid` is null |

The filter evaluator will need access to the collaborator list and the current user ID (available from cached `User` object).

### Crate: `todoist-cli-rs`

#### 5. `td add --assign <user>`

Add `--assign` option to `AddArgs`:
```rust
#[arg(long, value_name = "USER")]
assign: Option<String>,
```

In the add command handler:
1. If `--assign` is provided, validate the target project is shared
2. Resolve the user via `resolve_collaborator()`
3. Include `responsible_uid` in the `item_add` command args

#### 6. `td edit --assign <user>` / `td edit --unassign`

Add to `EditArgs`:
```rust
#[arg(long, value_name = "USER")]
assign: Option<String>,

#[arg(long)]
unassign: bool,
```

In the edit command handler:
1. If `--assign`, validate shared project + resolve user → set `responsible_uid`
2. If `--unassign`, set `responsible_uid` to `null` in the update args
3. `--assign` and `--unassign` are mutually exclusive (clap conflict)

#### 7. `td list --assigned-to <user>`

Add to `ListArgs`:
```rust
#[arg(long, value_name = "USER")]
assigned_to: Option<String>,
```

Special values:
- `me` → filter to `responsible_uid == current_user_id`
- Any other string → resolve across all collaborators, filter to matching user_id

#### 8. Display changes

Update `TaskOutput` to include:
```rust
pub assignee: Option<&'a str>,  // resolved display name, or "me"
```

Rendering: append `[@Alice]` or `[@me]` after labels in the formatted output, only when set.

Update `TaskDetailsOutput` (for `td show`) to include:
- Assignee (full name + email)
- Assigned by (full name)

#### 9. `td collaborators` command

New subcommand:
```rust
Collaborators {
    #[arg(long, short)]
    project: String,  // required, project name or ID
}
```

Output: table with columns `Name`, `Email`, `Status` (active/invited).

Behavior:
1. Resolve project by name/ID
2. Look up `collaborator_states` for that project
3. Join with `collaborators` to get name/email
4. Print table

Error if project is not shared (no collaborator states).

---

## Implementation Order

All in a single release, but implemented in dependency order:

1. **Cache collaborators** — Add fields to `Cache`, update merge logic, add indexes
2. **Collaborator resolution** — `resolve_collaborator()` + shared project check
3. **Display changes** — Add assignee to `TaskOutput`/`TaskDetailsOutput`, render `[@name]`
4. **`td edit --assign/--unassign`** — Wire up the flag, resolve user, send `responsible_uid`
5. **`td add --assign`** — Same pattern as edit
6. **`td list --assigned-to`** — CLI flag filtering
7. **Filter DSL** — AST + parser + evaluator for `assigned to:` / `assigned by:` expressions
8. **`td collaborators`** — New command with table output
9. **Tests** — Unit tests for cache merge, collaborator resolution, filter parser/evaluator, display formatting

---

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| Assign in personal project | Hard error: _"Task is in a personal project — share it first to assign tasks."_ |
| Assign to user not in project | Error: _"No collaborator matching '\<query\>' in project '\<name\>'"_ |
| Ambiguous name match | Error with list of matching names |
| Unassign already-unassigned task | No-op, succeed silently |
| Collaborator leaves project | Next sync removes their `collaborator_state`. Existing tasks keep `responsible_uid` but the name can't resolve — display the raw UID or "Unknown". |
| `--assign` + `--unassign` together | Clap conflict — rejected at parse time |
| Task moved from shared → personal project | Todoist handles this server-side (clears assignment). Our cache updates on next sync. |

---

## Testing

Tests are written alongside each implementation step, not deferred to the end. Follow existing project conventions: `make_*()` factory helpers for test data. E2E tests are out of scope for now — unit tests only.

### Unit Tests — `todoist-cache-rs`

#### Cache merge: collaborators

Location: `crates/todoist-cache/src/cache_tests.rs` (extend existing module)

Follow the existing merge test pattern (`test_full_sync_*`, `test_incremental_sync_*`). Add factory helpers:

```rust
fn make_collaborator(id: &str, name: &str, email: &str) -> Collaborator { ... }
fn make_collaborator_state(project_id: &str, user_id: &str, state: &str) -> CollaboratorState { ... }
```

| Test | Validates |
|------|-----------|
| `test_full_sync_populates_collaborators` | Full sync stores collaborators and collaborator_states in cache |
| `test_incremental_sync_adds_new_collaborator` | Partial sync appends a new collaborator without losing existing ones |
| `test_incremental_sync_updates_collaborator` | Updated collaborator (e.g., name change) replaces the existing entry |
| `test_incremental_sync_removes_deleted_collaborator_state` | `is_deleted: true` on a collaborator_state removes it from cache |
| `test_collaborator_indexes_rebuild` | After merge, `collaborator_by_id` and `collaborator_by_project` indexes are correct |
| `test_cache_serialization_roundtrip_with_collaborators` | Cache with collaborators survives JSON serialize → deserialize |

#### Collaborator resolution

Location: `crates/todoist-cache/src/` (new test module in resolver or sync_manager)

| Test | Validates |
|------|-----------|
| `test_resolve_exact_name_match` | `"Alice Smith"` matches collaborator with that full_name |
| `test_resolve_exact_email_match` | `"alice@example.com"` matches by email |
| `test_resolve_case_insensitive` | `"alice smith"` matches `"Alice Smith"` |
| `test_resolve_partial_name_match` | `"Alice"` matches when only one collaborator has that prefix |
| `test_resolve_no_match_errors` | `"nonexistent"` returns error with message containing project name |
| `test_resolve_ambiguous_match_errors` | `"mar"` matching `"Alice"` and `"Alicia"` returns error listing both names |
| `test_resolve_scoped_to_project` | A collaborator on project A isn't found when resolving for project B |
| `test_resolve_excludes_invited` | Collaborators with `state == "invited"` are not resolved (only `"active"`) |
| `test_is_shared_project_true` | Project with 2+ active collaborator_states returns true |
| `test_is_shared_project_false_personal` | Project with no collaborator_states returns false |
| `test_is_shared_project_false_only_owner` | Project with only the owner's collaborator_state returns false |

#### Filter parser

Location: `crates/todoist-cache/src/filter/tests.rs` (extend existing module)

Follow the existing pattern of `assert_eq!(parse("..."), Ok(expected_ast))`.

| Test | Input | Expected AST |
|------|-------|--------------|
| `test_parse_assigned_to_me` | `"assigned to: me"` | `Filter::AssignedTo(AssignedTarget::Me)` |
| `test_parse_assigned_to_others` | `"assigned to: others"` | `Filter::AssignedTo(AssignedTarget::Others)` |
| `test_parse_assigned_to_name` | `"assigned to: Alice"` | `Filter::AssignedTo(AssignedTarget::User("Alice"))` |
| `test_parse_assigned_by_me` | `"assigned by: me"` | `Filter::AssignedBy(AssignedTarget::Me)` |
| `test_parse_assigned_by_name` | `"assigned by: Alice"` | `Filter::AssignedBy(AssignedTarget::User("Alice"))` |
| `test_parse_assigned` | `"assigned"` | `Filter::Assigned` |
| `test_parse_no_assignee` | `"no assignee"` | `Filter::NoAssignee` |
| `test_parse_assigned_case_insensitive` | `"Assigned To: Me"` | Same as lowercase |
| `test_parse_assigned_combined` | `"assigned to: me & p1"` | `And(AssignedTo(Me), Priority(4))` |
| `test_parse_assigned_to_name_with_spaces` | `"assigned to: Alice Smith"` | `Filter::AssignedTo(AssignedTarget::User("Alice Smith"))` |

#### Filter evaluator

Location: `crates/todoist-cache/src/filter/evaluator_tests.rs` (extend existing module)

Use the existing `make_item()` helper, setting `responsible_uid` and `assigned_by_uid` on test items.

| Test | Setup | Filter | Expected |
|------|-------|--------|----------|
| `test_eval_assigned_to_me` | item with `responsible_uid = "user1"`, current user = `"user1"` | `assigned to: me` | matches |
| `test_eval_assigned_to_me_no_match` | item with `responsible_uid = "user2"`, current user = `"user1"` | `assigned to: me` | no match |
| `test_eval_assigned_to_others` | item with `responsible_uid = "user2"`, current user = `"user1"` | `assigned to: others` | matches |
| `test_eval_assigned_to_others_unassigned` | item with `responsible_uid = None` | `assigned to: others` | no match |
| `test_eval_assigned_to_name` | item `responsible_uid = "user2"`, collaborator "user2" = "Alice" | `assigned to: Alice` | matches |
| `test_eval_assigned` | item with `responsible_uid = "user2"` | `assigned` | matches |
| `test_eval_assigned_unassigned` | item with `responsible_uid = None` | `assigned` | no match |
| `test_eval_no_assignee` | item with `responsible_uid = None` | `no assignee` | matches |
| `test_eval_assigned_by_me` | item with `assigned_by_uid = "user1"`, current user = `"user1"` | `assigned by: me` | matches |
| `test_eval_not_assigned` | item with `responsible_uid = "user1"` | `!assigned` | no match |

### Unit Tests — `todoist-cli-rs`

#### Display formatting

Location: `crates/td/src/output/` (extend existing test module or add inline tests)

| Test | Validates |
|------|-----------|
| `test_task_output_with_assignee` | Task with `assignee: Some("Alice")` renders `[@Alice]` after labels |
| `test_task_output_with_me_assignee` | Task with `assignee: Some("me")` renders `[@me]` |
| `test_task_output_no_assignee` | Task with `assignee: None` renders no `[@...]` |
| `test_task_details_shows_assignee_and_assigner` | `td show` output includes "Assigned to:" and "Assigned by:" lines |

### Test Utilities to Add

| Utility | Location | Purpose |
|---------|----------|---------|
| `make_collaborator(id, name, email)` | `cache_tests.rs` test_helpers | Factory for test Collaborator objects |
| `make_collaborator_state(project_id, user_id, state)` | `cache_tests.rs` test_helpers | Factory for test CollaboratorState objects |

### What NOT to Test

- Todoist API behavior (e.g., does the server actually clear `responsible_uid` when a task moves to a personal project?) — that's Todoist's concern, not ours.
- Collaborator avatar URLs — we don't use `image_id`.
- Notification delivery — server-side.
- E2E tests — deferred to a follow-up. Unit tests provide sufficient coverage for now.

---

## Non-Goals (out of scope)

- **Sharing projects** from the CLI (`td share <project> <email>`) — separate feature
- **Notifications** — Todoist handles push notifications server-side
- **Workspace/team management** — separate from basic assignment
- **Interactive picker** for collaborator selection — keep it non-interactive and scriptable
