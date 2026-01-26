# E2E Test Specification

Comprehensive end-to-end test suite for todoist-rs, validating all API functionality against the real Todoist API.

## Overview

All E2E tests:
- Require `--features e2e` flag
- Require `TODOIST_TEST_API_TOKEN` in `.env.local`
- Clean up all created resources after each test
- Are idempotent and can run in any order
- Handle rate limiting gracefully (retry on 429)

## Test File Organization

```
crates/todoist-api/tests/
  api_e2e.rs                    # Basic API connectivity (existing)
  task_lifecycle_e2e.rs         # Task CRUD, move, complete, subtasks
  project_e2e.rs                # Project and section operations
  labels_e2e.rs                 # Label CRUD and task labeling
  reminders_e2e.rs              # Reminder operations (Pro)
  comments_e2e.rs               # Task and project comments (Pro)
  quick_add_e2e.rs              # NLP parsing scenarios

crates/todoist-cache/tests/
  cache_e2e.rs                  # Cache and sync behavior (existing)
  filter_e2e.rs                 # Filter parsing and evaluation (existing)
  filter_comprehensive_e2e.rs   # Extended filter coverage
  workflow_e2e.rs               # AI agent workflow simulations
```

---

## 1. Task Lifecycle Tests

**File:** `crates/todoist-api/tests/task_lifecycle_e2e.rs`

### 1.1 Basic CRUD

#### `test_create_task_minimal`
Create a task with only content field.
- Call `item_add` with `{content: "Test task"}`
- Verify task appears in sync response
- Verify task has default priority (1), no due date, no labels
- Clean up: delete task

#### `test_create_task_with_all_fields`
Create a task with all supported fields populated.
- Call `item_add` with:
  - `content`: "Complete task"
  - `description`: "Detailed description here"
  - `priority`: 4 (p1)
  - `due`: `{date: "2025-12-25"}`
  - `labels`: ["test-label"]
- Verify all fields persisted correctly
- Clean up: delete task and label

#### `test_update_task_content`
Modify task content via `item_update`.
- Create task with content "Original"
- Update content to "Modified"
- Sync and verify content changed
- Clean up: delete task

#### `test_update_task_description`
Add and modify task description.
- Create task with no description
- Update to add description "New description"
- Verify description persisted
- Update description to "Changed description"
- Verify change persisted
- Clean up: delete task

#### `test_delete_task`
Delete a task and verify removal.
- Create task
- Delete via `item_delete`
- Sync and verify task no longer present
- Verify task with `is_deleted: true` or absent from response

### 1.2 Task Movement

#### `test_move_task_between_projects`
Move a task from one project to another.
- Create Project A and Project B
- Create task in Project A
- Call `item_move` with `{id: task_id, project_id: project_b_id}`
- Sync and verify task's `project_id` is now Project B
- Clean up: delete task, both projects

#### `test_move_task_to_section`
Move a task into a section within the same project.
- Create project with section "In Progress"
- Create task in project (no section)
- Call `item_move` with `{id: task_id, section_id: section_id}`
- Sync and verify task's `section_id` matches
- Clean up: delete task, section, project

#### `test_move_task_out_of_section`
Move a task from a section back to project root.
- Create project with section
- Create task in section
- Call `item_move` with `{id: task_id, section_id: null}` (or project_id only)
- Sync and verify task's `section_id` is null
- Clean up: delete task, section, project

#### `test_move_task_to_section_in_different_project`
Move a task to a section in a different project.
- Create Project A and Project B with section "Done"
- Create task in Project A
- Call `item_move` with `{id: task_id, section_id: section_in_b_id}`
- Sync and verify task is in Project B's section
- Clean up: delete all

### 1.3 Task Completion

#### `test_complete_task_with_item_close`
Complete a task using `item_close`.
- Create task
- Call `item_close` with `{id: task_id}`
- Sync and verify task has `checked: true`
- Clean up: delete task

#### `test_complete_task_with_item_complete`
Complete a task using `item_complete` with completion date.
- Create task
- Call `item_complete` with `{id: task_id, completed_at: "2025-01-26T12:00:00Z"}`
- Verify `completed_at` timestamp set
- Clean up: delete task

#### `test_uncomplete_task`
Reopen a completed task.
- Create and complete task
- Call `item_uncomplete` with `{id: task_id}`
- Sync and verify `checked: false`
- Clean up: delete task

#### `test_complete_recurring_task`
Complete a recurring task and verify next occurrence.
- Create task with `due: {string: "every day"}`
- Note the original due date
- Call `item_close` (or `item_complete_recurring`)
- Sync and verify:
  - Task still exists (not deleted)
  - Due date advanced to next occurrence
  - `checked: false` (ready for next occurrence)
- Clean up: delete task

### 1.4 Subtasks (Parent-Child Relationships)

#### `test_create_subtask`
Create a task as a child of another task.
- Create parent task
- Create child task with `parent_id: parent_task_id`
- Sync and verify child's `parent_id` matches
- Clean up: delete both tasks

#### `test_create_nested_subtasks`
Create multiple levels of subtask nesting.
- Create task A
- Create task B with `parent_id: A`
- Create task C with `parent_id: B`
- Verify hierarchy: A → B → C
- Clean up: delete all

#### `test_move_subtask_to_different_parent`
Change a subtask's parent.
- Create parent A, parent B, and child C under A
- Call `item_move` with `{id: C, parent_id: B}`
- Sync and verify C's parent is now B
- Clean up: delete all

#### `test_promote_subtask_to_task`
Convert a subtask to a top-level task.
- Create parent and child
- Call `item_move` with `{id: child_id, parent_id: null}`
- Sync and verify child's `parent_id` is null
- Clean up: delete both

#### `test_complete_parent_with_subtasks`
Complete a parent task and observe subtask behavior.
- Create parent with 2 subtasks
- Complete parent via `item_close`
- Sync and document behavior (subtasks may also complete)
- Clean up: delete all

#### `test_delete_parent_cascades_to_subtasks`
Delete a parent and verify subtask handling.
- Create parent with 2 subtasks
- Delete parent
- Sync and verify subtasks are also deleted or orphaned
- Clean up: delete any remaining

### 1.5 Task Ordering

#### `test_reorder_tasks_in_project`
Reorder tasks within a project using `item_reorder`.
- Create 3 tasks in a project
- Call `item_reorder` to change order
- Sync and verify `child_order` values reflect new order
- Clean up: delete all

#### `test_update_day_orders`
Update task ordering for Today view.
- Create 3 tasks due today
- Call `item_update_day_orders` with new order
- Sync and verify `day_order` values
- Clean up: delete all

---

## 2. Due Dates and Scheduling

**File:** `crates/todoist-api/tests/task_lifecycle_e2e.rs` (continued)

#### `test_set_due_date_simple`
Set a simple due date without time.
- Create task
- Update with `due: {date: "2025-06-15"}`
- Verify `due.date` is "2025-06-15"
- Verify `due.datetime` is null (no time component)
- Clean up: delete task

#### `test_set_due_date_with_time`
Set a due date with specific time.
- Create task
- Update with `due: {date: "2025-06-15T14:30:00"}`
- Verify `due.datetime` includes time
- Clean up: delete task

#### `test_set_due_date_with_timezone`
Set a due date with timezone.
- Create task
- Update with `due: {date: "2025-06-15T14:30:00", timezone: "America/New_York"}`
- Verify timezone persisted
- Clean up: delete task

#### `test_set_recurring_due_date`
Set a recurring due date using natural language.
- Create task
- Update with `due: {string: "every monday at 9am"}`
- Verify `due.is_recurring` is true
- Verify `due.string` contains recurrence info
- Clean up: delete task

#### `test_remove_due_date`
Clear a task's due date.
- Create task with due date
- Update with `due: null`
- Sync and verify `due` is null
- Clean up: delete task

#### `test_set_deadline`
Set a deadline (distinct from due date).
- Create task with due date
- Set deadline via `deadline` field
- Verify both `due` and `deadline` can coexist
- Clean up: delete task

#### `test_overdue_task`
Verify overdue task behavior.
- Create task with due date in the past
- Sync and verify task exists
- Filter with "overdue" should match this task
- Clean up: delete task

#### `test_due_date_preserved_on_move`
Moving a task preserves its due date.
- Create task with due date in Project A
- Move to Project B
- Verify due date unchanged
- Clean up: delete all

---

## 3. Project Operations

**File:** `crates/todoist-api/tests/project_e2e.rs`

### 3.1 Basic CRUD

#### `test_create_project_minimal`
Create a project with just a name.
- Call `project_add` with `{name: "Test Project"}`
- Sync and verify project exists
- Verify default color, view_style
- Clean up: delete project

#### `test_create_project_with_color`
Create a project with specific color.
- Call `project_add` with `{name: "Colored", color: "red"}`
- Verify color persisted
- Clean up: delete project

#### `test_create_project_with_view_style`
Create a project with board view.
- Call `project_add` with `{name: "Board", view_style: "board"}`
- Verify `view_style` is "board"
- Clean up: delete project

#### `test_update_project_name`
Rename a project.
- Create project "Original Name"
- Call `project_update` with `{id: ..., name: "New Name"}`
- Verify name changed
- Clean up: delete project

#### `test_update_project_color`
Change a project's color.
- Create project with color "red"
- Update to color "blue"
- Verify color changed
- Clean up: delete project

#### `test_delete_project`
Delete a project.
- Create project
- Call `project_delete`
- Sync and verify project gone
- Clean up: none needed

#### `test_delete_project_with_tasks`
Delete a project containing tasks.
- Create project with 3 tasks
- Delete project
- Verify project and all tasks deleted
- Clean up: none needed

### 3.2 Project Hierarchy

#### `test_create_subproject`
Create a project as a child of another.
- Create parent project
- Create child project with `parent_id: parent_id`
- Verify child's `parent_id` set correctly
- Clean up: delete both

#### `test_create_nested_subprojects`
Create multiple levels of project nesting.
- Create Project A
- Create Project B under A
- Create Project C under B
- Verify hierarchy
- Clean up: delete all

#### `test_move_project_under_parent`
Move an existing project to become a subproject.
- Create Project A and Project B (both root level)
- Call `project_move` with `{id: B, parent_id: A}`
- Verify B is now under A
- Clean up: delete both

#### `test_promote_subproject_to_root`
Move a subproject to root level.
- Create parent and child projects
- Call `project_move` with `{id: child, parent_id: null}`
- Verify child is now root level
- Clean up: delete both

#### `test_reorder_projects`
Reorder projects at same level.
- Create 3 root-level projects
- Call `project_reorder` to change order
- Verify `child_order` values
- Clean up: delete all

### 3.3 Project Archive

#### `test_archive_project`
Archive a project.
- Create project with tasks
- Call `project_archive`
- Sync and verify project has `is_archived: true`
- Verify tasks still exist but associated with archived project
- Clean up: unarchive and delete

#### `test_unarchive_project`
Restore an archived project.
- Create and archive project
- Call `project_unarchive`
- Verify `is_archived: false`
- Clean up: delete project

#### `test_archived_project_excluded_from_filters`
Tasks in archived projects don't appear in filters.
- Create project with task due today
- Verify filter "today" includes task
- Archive project
- Verify filter "today" no longer includes task
- Clean up: unarchive and delete

---

## 4. Section Operations

**File:** `crates/todoist-api/tests/project_e2e.rs` (continued)

#### `test_create_section`
Create a section in a project.
- Create project
- Call `section_add` with `{name: "To Do", project_id: ...}`
- Verify section exists with correct project_id
- Clean up: delete section and project

#### `test_create_multiple_sections`
Create multiple sections and verify ordering.
- Create project
- Create sections: "To Do", "In Progress", "Done"
- Verify all exist with correct order
- Clean up: delete all

#### `test_rename_section`
Rename a section.
- Create project with section "Old Name"
- Call `section_update` with new name
- Verify name changed
- Clean up: delete all

#### `test_delete_section`
Delete a section.
- Create project with section
- Delete section
- Verify section gone
- Clean up: delete project

#### `test_delete_section_with_tasks`
Delete a section containing tasks.
- Create project with section containing 2 tasks
- Delete section
- Verify section deleted
- Verify tasks still exist (moved to project root or deleted - document behavior)
- Clean up: delete remaining

#### `test_reorder_sections`
Reorder sections within a project.
- Create project with 3 sections
- Call `section_reorder` to change order
- Verify `section_order` values
- Clean up: delete all

#### `test_move_section_to_different_project`
Move a section from one project to another.
- Create Project A with section, Project B
- Call `section_move` with `{id: section, project_id: B}`
- Verify section now in Project B
- Clean up: delete all

#### `test_archive_section`
Archive a section.
- Create project with section and tasks
- Call `section_archive`
- Verify section has `is_archived: true`
- Clean up: unarchive and delete

#### `test_unarchive_section`
Restore an archived section.
- Create and archive section
- Call `section_unarchive`
- Verify `is_archived: false`
- Clean up: delete all

---

## 5. Label Operations

**File:** `crates/todoist-api/tests/labels_e2e.rs`

### 5.1 Label CRUD

#### `test_create_label`
Create a personal label.
- Call `label_add` with `{name: "test-label"}`
- Sync and verify label exists
- Clean up: delete label

#### `test_create_label_with_color`
Create a label with specific color.
- Call `label_add` with `{name: "colored-label", color: "green"}`
- Verify color persisted
- Clean up: delete label

#### `test_rename_label`
Rename a label.
- Create label "old-name"
- Call `label_update` with new name "new-name"
- Verify name changed
- Verify tasks with label still have it (name updated)
- Clean up: delete label

#### `test_delete_label`
Delete a label.
- Create label
- Add label to a task
- Delete label
- Verify label gone
- Verify task no longer has label
- Clean up: delete task

### 5.2 Task Labeling

#### `test_add_single_label_to_task`
Add one label to a task.
- Create task and label
- Update task with `labels: ["label-name"]`
- Verify task has label
- Clean up: delete both

#### `test_add_multiple_labels_to_task`
Add multiple labels at once.
- Create task and 3 labels
- Update task with all 3 labels
- Verify task has all 3
- Clean up: delete all

#### `test_remove_one_label_from_task`
Remove one label while keeping others.
- Create task with labels ["a", "b", "c"]
- Update task with `labels: ["a", "c"]`
- Verify only "a" and "c" remain
- Clean up: delete all

#### `test_replace_all_labels`
Replace entire label set on a task.
- Create task with labels ["old1", "old2"]
- Update with `labels: ["new1", "new2"]`
- Verify only new labels present
- Clean up: delete all

#### `test_clear_all_labels`
Remove all labels from a task.
- Create task with labels
- Update with `labels: []`
- Verify task has no labels
- Clean up: delete all

#### `test_label_case_insensitivity`
Labels are case-insensitive.
- Create label "MyLabel"
- Create task with label "mylabel" (lowercase)
- Verify task has the label (normalized)
- Clean up: delete all

#### `test_add_label_via_item_update`
Add label using item_update command.
- Create task with no labels
- Get current labels, append new one
- Call `item_update` with new labels array
- Verify label added
- Clean up: delete all

---

## 6. Reminder Operations (Pro)

**File:** `crates/todoist-api/tests/reminders_e2e.rs`

#### `test_create_absolute_reminder`
Create a reminder at a specific datetime.
- Create task
- Call `reminder_add` with `{item_id: ..., due: {date: "2025-06-15T09:00:00"}}`
- Sync and verify reminder exists
- Clean up: delete reminder and task

#### `test_create_relative_reminder`
Create a reminder relative to task due time.
- Create task with due datetime
- Call `reminder_add` with `{item_id: ..., minute_offset: 30}` (30 min before)
- Verify reminder created
- Clean up: delete all

#### `test_update_reminder`
Modify an existing reminder.
- Create task and reminder
- Call `reminder_update` to change time
- Verify change persisted
- Clean up: delete all

#### `test_delete_reminder`
Delete a reminder.
- Create task and reminder
- Call `reminder_delete`
- Verify reminder gone, task still exists
- Clean up: delete task

#### `test_multiple_reminders_on_task`
Add multiple reminders to one task.
- Create task
- Add 3 reminders at different times
- Verify all 3 exist
- Clean up: delete all

#### `test_reminder_on_recurring_task`
Reminder behavior with recurring task.
- Create recurring task
- Add reminder
- Complete task (advances recurrence)
- Verify reminder behavior (reset or persisted)
- Clean up: delete all

#### `test_reminder_appears_in_cache`
Reminders sync correctly to cache.
- Create task and reminder via API
- Sync cache
- Verify reminder in `cache.reminders`
- Clean up: delete all

---

## 7. Comment Operations (Pro)

**File:** `crates/todoist-api/tests/comments_e2e.rs`

### 7.1 Task Comments

#### `test_add_task_comment`
Add a comment to a task.
- Create task
- Call `note_add` with `{item_id: ..., content: "This is a comment"}`
- Sync and verify comment exists in `notes`
- Clean up: delete comment and task

#### `test_add_comment_with_formatting`
Add a comment with markdown formatting.
- Create task
- Add comment with `**bold** and *italic* text`
- Verify content preserved
- Clean up: delete all

#### `test_update_task_comment`
Modify an existing comment.
- Create task and comment
- Call `note_update` with new content
- Verify content changed
- Clean up: delete all

#### `test_delete_task_comment`
Delete a comment from a task.
- Create task and comment
- Call `note_delete`
- Verify comment gone, task still exists
- Clean up: delete task

#### `test_multiple_comments_on_task`
Add multiple comments to one task.
- Create task
- Add 3 comments
- Verify all exist and maintain order
- Clean up: delete all

### 7.2 Project Comments

#### `test_add_project_comment`
Add a comment to a project.
- Create project
- Call `project_note_add` with `{project_id: ..., content: "Project note"}`
- Sync and verify in `project_notes`
- Clean up: delete all

#### `test_update_project_comment`
Modify a project comment.
- Create project and comment
- Call `project_note_update`
- Verify change
- Clean up: delete all

#### `test_delete_project_comment`
Delete a project comment.
- Create project and comment
- Call `project_note_delete`
- Verify comment gone
- Clean up: delete project

---

## 8. Quick Add NLP Tests

**File:** `crates/todoist-api/tests/quick_add_e2e.rs`

#### `test_quick_add_plain_text`
Quick add with no NLP markers.
- Quick add "Simple task"
- Verify content is "Simple task", no due date, default priority
- Clean up: delete task

#### `test_quick_add_due_today`
Parse "today" due date.
- Quick add "Buy milk today"
- Verify due date is today
- Verify content is "Buy milk" (today removed)
- Clean up: delete task

#### `test_quick_add_due_tomorrow`
Parse "tomorrow" due date.
- Quick add "Call mom tomorrow"
- Verify due date is tomorrow
- Clean up: delete task

#### `test_quick_add_due_specific_date`
Parse specific date.
- Quick add "Meeting on Dec 25"
- Verify due date is December 25
- Clean up: delete task

#### `test_quick_add_due_next_week`
Parse relative date.
- Quick add "Review next monday"
- Verify due date is next Monday
- Clean up: delete task

#### `test_quick_add_recurring`
Parse recurring due date.
- Quick add "Standup every weekday at 9am"
- Verify `due.is_recurring` is true
- Clean up: delete task

#### `test_quick_add_priority_p1`
Parse p1 priority.
- Quick add "Fix critical bug p1"
- Verify priority is 4 (API value for p1)
- Verify content has "p1" removed
- Clean up: delete task

#### `test_quick_add_priority_p2`
Parse p2 priority.
- Quick add "Review PR p2"
- Verify priority is 3
- Clean up: delete task

#### `test_quick_add_label`
Parse label with @.
- Create label "work"
- Quick add "Finish report @work"
- Verify task has "work" label
- Clean up: delete task and label

#### `test_quick_add_multiple_labels`
Parse multiple labels.
- Create labels "urgent" and "work"
- Quick add "Task @urgent @work"
- Verify both labels attached
- Clean up: delete all

#### `test_quick_add_project`
Parse project with #.
- Create project "Shopping"
- Quick add "Buy groceries #Shopping"
- Verify task is in Shopping project
- Clean up: delete all

#### `test_quick_add_section`
Parse section with /.
- Create project with section "Backlog"
- Quick add "New feature /Backlog"
- Verify task is in Backlog section
- Clean up: delete all

#### `test_quick_add_combined`
Parse multiple NLP elements together.
- Create project "Work" with label "urgent"
- Quick add "Submit report tomorrow p2 @urgent #Work"
- Verify:
  - Due date is tomorrow
  - Priority is 3
  - Label is "urgent"
  - Project is "Work"
- Clean up: delete all

#### `test_quick_add_with_description`
Quick add with note/description.
- Quick add "Task" with note "Detailed description"
- Verify description attached
- Clean up: delete task

---

## 9. Filter Evaluation Tests

**File:** `crates/todoist-cache/tests/filter_comprehensive_e2e.rs`

### 9.1 Date Filters

#### `test_filter_today`
Filter matches tasks due today.
- Create task due today, task due tomorrow, task with no date
- Filter "today"
- Verify only today task matches
- Clean up: delete all

#### `test_filter_tomorrow`
Filter matches tasks due tomorrow.
- Create tasks due today, tomorrow, next week
- Filter "tomorrow"
- Verify only tomorrow task matches
- Clean up: delete all

#### `test_filter_overdue`
Filter matches tasks with past due date.
- Create task due yesterday, today, tomorrow
- Filter "overdue"
- Verify only yesterday task matches
- Clean up: delete all

#### `test_filter_no_date`
Filter matches tasks without due date.
- Create task with due date, task without
- Filter "no date"
- Verify only no-date task matches
- Clean up: delete all

#### `test_filter_7_days`
Filter matches tasks due within 7 days.
- Create tasks due: today, in 5 days, in 10 days
- Filter "7 days"
- Verify correct tasks match
- Clean up: delete all

#### `test_filter_specific_date`
Filter matches tasks due on specific date.
- Create tasks due Jan 15, Jan 16
- Filter "Jan 15"
- Verify only Jan 15 task matches
- Clean up: delete all

### 9.2 Priority Filters

#### `test_filter_p1`
Filter matches only p1 tasks.
- Create tasks with p1, p2, p3, p4
- Filter "p1"
- Verify only p1 matches
- Clean up: delete all

#### `test_filter_p1_or_p2`
Filter matches p1 or p2.
- Create tasks with all priorities
- Filter "p1 | p2"
- Verify p1 and p2 match, p3 and p4 don't
- Clean up: delete all

#### `test_filter_p4_default_priority`
Filter matches default priority tasks.
- Create task with no explicit priority
- Filter "p4"
- Verify matches (p4 is default)
- Clean up: delete all

### 9.3 Label Filters

#### `test_filter_single_label`
Filter by one label.
- Create tasks with different labels
- Filter "@work"
- Verify only work-labeled tasks match
- Clean up: delete all

#### `test_filter_no_labels`
Filter tasks without labels.
- Create task with label, task without
- Filter "no labels"
- Verify only unlabeled task matches
- Clean up: delete all

#### `test_filter_multiple_labels_and`
Filter requiring both labels.
- Create tasks: [@a], [@b], [@a, @b]
- Filter "@a & @b"
- Verify only task with both matches
- Clean up: delete all

#### `test_filter_multiple_labels_or`
Filter matching either label.
- Create tasks with @a, @b, @c
- Filter "@a | @b"
- Verify @a and @b match, @c doesn't
- Clean up: delete all

### 9.4 Project Filters

#### `test_filter_project`
Filter by project name.
- Create projects A and B with tasks
- Filter "#A"
- Verify only A's tasks match
- Clean up: delete all

#### `test_filter_project_with_subprojects`
Filter includes subproject tasks.
- Create parent project with subproject
- Add task to each
- Filter "##Parent"
- Verify both tasks match
- Clean up: delete all

#### `test_filter_inbox`
Filter for inbox tasks.
- Create task in inbox, task in project
- Filter "#Inbox"
- Verify only inbox task matches
- Clean up: delete all

### 9.5 Section Filters

#### `test_filter_section`
Filter by section name.
- Create project with sections A and B
- Add tasks to each
- Filter "/A"
- Verify only section A tasks match
- Clean up: delete all

#### `test_filter_section_in_project`
Filter by section within specific project.
- Create 2 projects each with section "Done"
- Filter "#Project1 & /Done"
- Verify only Project1's Done tasks match
- Clean up: delete all

### 9.6 Complex Filters

#### `test_filter_and_precedence`
AND has higher precedence than OR.
- Create tasks matching various combinations
- Filter "today | p1 & @urgent"
- Verify parsed as "today | (p1 & @urgent)"
- Clean up: delete all

#### `test_filter_parentheses`
Parentheses override precedence.
- Filter "(today | tomorrow) & p1"
- Verify matches: today+p1, tomorrow+p1
- Does NOT match: today+p4, next week+p1
- Clean up: delete all

#### `test_filter_negation_label`
Exclude tasks with label.
- Create tasks with and without @blocked
- Filter "!@blocked"
- Verify @blocked tasks excluded
- Clean up: delete all

#### `test_filter_negation_project`
Exclude tasks in project.
- Create tasks in various projects
- Filter "!#Inbox"
- Verify inbox tasks excluded
- Clean up: delete all

#### `test_filter_complex_real_world`
Complex filter simulating real usage.
- Create diverse set of tasks
- Filter "##Work & (p1 | p2) & !@blocked & (today | overdue)"
- Verify correct matching
- Clean up: delete all

---

## 10. Sync Behavior Tests

**File:** `crates/todoist-cache/tests/cache_e2e.rs` (extend existing)

#### `test_sync_picks_up_task_created_externally`
External changes appear after sync.
- Sync cache
- Create task via direct API call (not through cache)
- Sync cache again
- Verify new task in cache
- Clean up: delete task

#### `test_sync_picks_up_task_deleted_externally`
External deletions reflected in cache.
- Create task, sync to cache
- Delete task via direct API call
- Sync cache
- Verify task removed from cache
- Clean up: none

#### `test_sync_picks_up_task_updated_externally`
External updates reflected in cache.
- Create task with content "Original", sync
- Update to "Modified" via direct API
- Sync cache
- Verify cache has "Modified"
- Clean up: delete task

#### `test_sync_after_bulk_operations`
Bulk create syncs correctly.
- Create 20 tasks in one sync command batch
- Sync cache
- Verify all 20 in cache
- Clean up: delete all

#### `test_sync_token_survives_restart`
Sync token persisted correctly.
- Sync cache, note token
- Drop manager, create new one from same file
- Verify token loaded, incremental sync works
- Clean up: none

#### `test_full_sync_after_invalid_token`
Invalid token triggers full sync.
- Sync cache
- Manually corrupt sync_token in cache file
- Create new manager
- Sync (should do full sync)
- Verify cache repopulated
- Clean up: none

---

## 11. Edge Cases and Stress Tests

**File:** `crates/todoist-api/tests/edge_cases_e2e.rs`

### 11.1 Unicode and Special Characters

#### `test_unicode_in_task_content`
Task content with unicode.
- Create task "Buy Japanese book 日本語の本 📚"
- Sync and verify content preserved exactly
- Clean up: delete task

#### `test_unicode_in_project_name`
Project name with unicode.
- Create project "工作 Projects 🏢"
- Verify name preserved
- Clean up: delete project

#### `test_unicode_in_label_name`
Label name with unicode.
- Create label "重要"
- Add to task
- Verify label works
- Clean up: delete all

#### `test_special_characters_in_content`
Content with quotes, backslashes, newlines.
- Create task with content: `Line 1\nLine 2 with "quotes" and \\backslash`
- Verify preserved
- Clean up: delete task

#### `test_emoji_in_all_fields`
Emoji in task, project, label, description.
- Create task "🎯 Goal" in project "📁 Projects" with label "⭐" and description "📝 Notes"
- Verify all preserved
- Clean up: delete all

### 11.2 Boundary Conditions

#### `test_very_long_task_content`
Task with very long content (1000+ chars).
- Create task with 2000 character content
- Verify truncation behavior or full preservation
- Document API limits
- Clean up: delete task

#### `test_very_long_description`
Task with very long description.
- Create task with 5000 character description
- Verify behavior
- Clean up: delete task

#### `test_empty_project`
Project with no tasks.
- Create project with no tasks
- Sync and filter by project
- Verify empty result (not error)
- Clean up: delete project

#### `test_deeply_nested_subtasks`
5+ levels of subtask nesting.
- Create A → B → C → D → E hierarchy
- Verify all relationships correct
- Clean up: delete all

#### `test_deeply_nested_projects`
5+ levels of project nesting.
- Create nested project hierarchy
- Verify ##Parent filter includes all
- Clean up: delete all

#### `test_task_with_many_labels`
Task with 20+ labels.
- Create 25 labels
- Add all to one task
- Verify all attached
- Clean up: delete all

#### `test_project_with_many_sections`
Project with 20+ sections.
- Create project with 25 sections
- Verify all exist and ordered
- Clean up: delete all

### 11.3 Rapid Operations

#### `test_rapid_create_delete`
Create and immediately delete.
- Create task
- Immediately delete (same sync batch)
- Verify no errors, task gone
- Clean up: none

#### `test_rapid_update_cycle`
Multiple rapid updates.
- Create task
- Update 10 times in sequence
- Verify final state correct
- Clean up: delete task

#### `test_rate_limit_handling`
Verify 429 handling.
- Make many rapid requests to trigger rate limit
- Verify retry logic works
- Document rate limit behavior
- Clean up: any created resources

---

## 12. AI Agent Workflow Tests

**File:** `crates/todoist-cache/tests/workflow_e2e.rs`

These tests simulate realistic multi-step workflows that an AI agent or automation would perform.

#### `test_workflow_daily_review`
Simulate a daily task review.
1. Sync cache
2. Filter tasks due "today"
3. Complete all tasks
4. Sync and verify all completed
5. Clean up: delete created tasks

#### `test_workflow_project_setup`
Set up a new project with structure.
1. Create project "New Feature"
2. Create sections: "Backlog", "In Progress", "Review", "Done"
3. Create 3 tasks in Backlog
4. Verify structure via sync
5. Clean up: delete all

#### `test_workflow_task_triage`
Triage inbox tasks.
1. Create 5 tasks in Inbox
2. Create target project and labels
3. Move tasks to project
4. Add labels based on content
5. Verify final state
6. Clean up: delete all

#### `test_workflow_bulk_task_creation`
Create many tasks efficiently.
1. Batch create 50 tasks in single sync
2. Verify all created with correct properties
3. Clean up: delete all

#### `test_workflow_search_and_update`
Find and update matching tasks.
1. Create tasks with various priorities
2. Filter for p4 (low priority)
3. Update all to p3
4. Verify changes
5. Clean up: delete all

#### `test_workflow_project_migration`
Move all tasks from one project to another.
1. Create Project A with 10 tasks
2. Create Project B
3. Move all tasks from A to B
4. Verify A empty, B has all tasks
5. Clean up: delete all

#### `test_workflow_recurring_task_cycle`
Manage recurring tasks.
1. Create recurring task "Daily standup every weekday"
2. Complete task
3. Verify next occurrence created
4. Complete again
5. Verify pattern continues
6. Clean up: delete task

#### `test_workflow_label_cleanup`
Rename label and verify cascade.
1. Create label "oldname"
2. Add to 5 tasks
3. Rename to "newname"
4. Verify all tasks have "newname"
5. Clean up: delete all

#### `test_workflow_end_of_day_cleanup`
Complete all tasks for the day.
1. Create 5 tasks due today
2. Complete all
3. Create 3 new tasks for tomorrow
4. Verify today empty, tomorrow has tasks
5. Clean up: delete all

---

## Running the Tests

```bash
# Run all E2E tests
cargo test --features e2e

# Run specific test file
cargo test --package todoist-api --features e2e --test task_lifecycle_e2e

# Run specific test
cargo test --features e2e test_move_task_between_projects

# Run with output
cargo test --features e2e -- --nocapture
```

## Test Data Naming Convention

All test-created resources should use prefixed names for easy identification:
- Projects: `E2E_Test_<description>_<uuid>`
- Labels: `e2e-test-<description>-<uuid>`
- Tasks: `E2E test - <description>`

This allows manual cleanup if tests fail mid-execution.

## Rate Limit Considerations

The Todoist API has strict rate limits:

| Sync Type | Limit | Window |
|-----------|-------|--------|
| **Full sync** | 100 requests | 15 minutes |
| **Partial sync** | 1000 requests | 15 minutes |
| **Commands per request** | 100 max | N/A |

### Rate Limit Mitigation Architecture

The test suite MUST be designed to stay within these limits. The key insight:
**After 1 full sync, all subsequent syncs should be partial (incremental).**

#### TestContext Pattern

All E2E tests should use a shared `TestContext` that maintains sync state:

```rust
/// Shared context for E2E tests that minimizes API calls
pub struct TestContext {
    pub client: TodoistClient,
    sync_token: String,
    pub inbox_id: String,
    // Cached state from last sync
    pub items: Vec<Item>,
    pub projects: Vec<Project>,
    pub sections: Vec<Section>,
    pub labels: Vec<Label>,
}

impl TestContext {
    /// Create new context with ONE full sync
    pub async fn new() -> Self {
        let token = get_test_token().expect("TODOIST_TEST_API_TOKEN required");
        let client = TodoistClient::new(token);

        // ONE full sync at initialization
        let response = client.sync(SyncRequest::full_sync()).await.unwrap();

        Self {
            sync_token: response.sync_token.clone(),
            inbox_id: response.projects.iter()
                .find(|p| p.inbox_project)
                .expect("Should have inbox")
                .id.clone(),
            items: response.items,
            projects: response.projects,
            sections: response.sections,
            labels: response.labels,
            client,
        }
    }

    /// Execute commands and update cached state (partial sync)
    pub async fn execute(&mut self, commands: Vec<SyncCommand>) -> SyncResponse {
        let request = SyncRequest::with_token_and_commands(&self.sync_token, commands);
        let response = self.client.sync(request).await.unwrap();

        // Update sync token for next call
        self.sync_token = response.sync_token.clone();

        // Merge response data into cached state
        self.merge_response(&response);

        response
    }

    /// Partial sync to refresh state (NOT full sync)
    pub async fn refresh(&mut self) -> SyncResponse {
        let request = SyncRequest::with_token(&self.sync_token);
        let response = self.client.sync(request).await.unwrap();
        self.sync_token = response.sync_token.clone();
        self.merge_response(&response);
        response
    }

    /// Find item in cached state (no API call)
    pub fn find_item(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id && !i.is_deleted)
    }

    fn merge_response(&mut self, response: &SyncResponse) {
        // Merge items, projects, sections, labels from response
        // Update existing, add new, mark deleted
        for item in &response.items {
            if let Some(existing) = self.items.iter_mut().find(|i| i.id == item.id) {
                *existing = item.clone();
            } else {
                self.items.push(item.clone());
            }
        }
        // Similar for projects, sections, labels...
    }
}
```

#### Command Batching

Instead of individual API calls, batch related commands:

```rust
// BAD: 3 separate API calls
let task1_id = create_task(&client, "Task 1", &inbox_id, None).await;
let task2_id = create_task(&client, "Task 2", &inbox_id, None).await;
let task3_id = create_task(&client, "Task 3", &inbox_id, None).await;

// GOOD: 1 API call with 3 commands
let temp_ids: Vec<String> = (0..3).map(|_| uuid::Uuid::new_v4().to_string()).collect();
let commands: Vec<SyncCommand> = (0..3)
    .map(|i| SyncCommand::with_temp_id(
        "item_add",
        &temp_ids[i],
        json!({"content": format!("Task {}", i+1), "project_id": inbox_id})
    ))
    .collect();

let response = ctx.execute(commands).await;
let task_ids: Vec<String> = temp_ids.iter()
    .map(|tid| response.real_id(tid).unwrap().clone())
    .collect();
```

#### Verification Without Re-Syncing

Use the command response directly instead of syncing again:

```rust
// BAD: Extra full sync for verification
let response = ctx.execute(vec![create_command]).await;
let task = find_task(&client, &task_id).await; // <-- FULL SYNC!

// GOOD: Use response data or cached state
let response = ctx.execute(vec![create_command]).await;
assert!(!response.has_errors());
let task = ctx.find_item(&task_id).expect("Task should be in cache");
```

#### Test Structure

```rust
#[tokio::test]
async fn test_create_and_move_task() {
    let mut ctx = TestContext::new().await;  // 1 full sync total

    // Create task (partial sync with command)
    let temp_id = uuid::Uuid::new_v4().to_string();
    let response = ctx.execute(vec![
        SyncCommand::with_temp_id("item_add", &temp_id, json!({
            "content": "Test task",
            "project_id": ctx.inbox_id
        }))
    ]).await;

    let task_id = response.real_id(&temp_id).unwrap();

    // Verify from cache (no API call)
    let task = ctx.find_item(task_id).expect("Task should exist");
    assert_eq!(task.content, "Test task");

    // Move task (partial sync with command)
    let project_id = create_test_project(&mut ctx).await;
    ctx.execute(vec![
        SyncCommand::new("item_move", json!({"id": task_id, "project_id": project_id}))
    ]).await;

    // Verify from cache (no API call)
    let task = ctx.find_item(task_id).expect("Task should exist");
    assert_eq!(task.project_id, project_id);

    // Cleanup (batch delete)
    ctx.execute(vec![
        SyncCommand::new("item_delete", json!({"id": task_id})),
        SyncCommand::new("project_delete", json!({"id": project_id})),
    ]).await;
}
```

#### API Call Budget

For a test run with N tests:
- **Without optimization**: ~2N full syncs (easily exceeds 100)
- **With TestContext**: 1 full sync + ~2N partial syncs (stays under limits)

| Tests | Full Syncs (Before) | Full Syncs (After) | Partial Syncs |
|-------|---------------------|--------------------| --------------|
| 25    | 50+                 | 1                  | ~75           |
| 50    | 100+                | 1                  | ~150          |
| 100   | 200+                | 1                  | ~300          |

### Additional Best Practices

1. **Use batch sync commands** - Up to 100 commands per request
2. **Reuse TestContext** - Don't create a new full sync per test
3. **Verify from cache** - Don't re-sync just to check a value
4. **Batch cleanup** - Delete multiple resources in one request
5. **Handle 429 gracefully** - Retry with exponential backoff
6. **Clean up promptly** - Avoid accumulating test data
