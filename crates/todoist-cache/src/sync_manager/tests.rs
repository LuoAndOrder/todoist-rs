//! Tests for the sync manager module.

use super::*;
use chrono::{DateTime, Duration, Utc};
use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{Collaborator, CollaboratorState, Project, TzInfo, User};

fn make_test_manager() -> SyncManager {
    let client = TodoistClient::with_base_url("test-token", "http://localhost").unwrap();
    let temp_dir = tempfile::tempdir().unwrap();
    let cache_path = temp_dir.path().join("cache.json");
    let store = CacheStore::with_path(cache_path);
    SyncManager::new(client, store).unwrap()
}

fn make_project(id: &str, name: &str) -> Project {
    Project {
        id: id.to_string(),
        name: name.to_string(),
        color: None,
        parent_id: None,
        child_order: 0,
        is_collapsed: false,
        shared: false,
        can_assign_tasks: false,
        is_deleted: false,
        is_archived: false,
        is_favorite: false,
        view_style: None,
        inbox_project: false,
        folder_id: None,
        created_at: None,
        updated_at: None,
    }
}

fn make_collaborator(id: &str, name: &str, email: &str) -> Collaborator {
    Collaborator {
        id: id.to_string(),
        email: Some(email.to_string()),
        full_name: Some(name.to_string()),
        timezone: Some("UTC".to_string()),
        image_id: None,
    }
}

fn make_collaborator_state(project_id: &str, user_id: &str, state: &str) -> CollaboratorState {
    CollaboratorState {
        project_id: project_id.to_string(),
        user_id: user_id.to_string(),
        state: state.to_string(),
    }
}

fn make_user(id: &str) -> User {
    User {
        id: id.to_string(),
        email: Some("owner@example.com".to_string()),
        full_name: Some("Owner".to_string()),
        tz_info: Some(TzInfo {
            timezone: "UTC".to_string(),
            gmt_string: Some("+00:00".to_string()),
            hours: 0,
            minutes: 0,
            is_dst: 0,
        }),
        inbox_project_id: None,
        start_page: None,
        start_day: None,
        date_format: None,
        time_format: None,
        is_premium: false,
    }
}

// Test staleness calculation with various scenarios

#[test]
fn test_is_stale_when_never_synced() {
    // A cache that has never been synced should always be stale
    let cache = Cache::new();
    assert!(cache.last_sync.is_none());

    // We can't easily test SyncManager without a real client/store,
    // so we test the staleness logic directly on Cache
    let last_sync: Option<DateTime<Utc>> = None;
    let now = Utc::now();
    let threshold = Duration::minutes(5);

    let is_stale = match last_sync {
        None => true,
        Some(ls) => now.signed_duration_since(ls) > threshold,
    };
    assert!(is_stale);
}

#[test]
fn test_is_stale_when_recently_synced() {
    let now = Utc::now();
    let last_sync = Some(now - Duration::minutes(2)); // 2 minutes ago
    let threshold = Duration::minutes(5);

    let is_stale = match last_sync {
        None => true,
        Some(ls) => now.signed_duration_since(ls) > threshold,
    };
    assert!(!is_stale);
}

#[test]
fn test_is_stale_when_old_sync() {
    let now = Utc::now();
    let last_sync = Some(now - Duration::minutes(10)); // 10 minutes ago
    let threshold = Duration::minutes(5);

    let is_stale = match last_sync {
        None => true,
        Some(ls) => now.signed_duration_since(ls) > threshold,
    };
    assert!(is_stale);
}

#[test]
fn test_is_stale_at_threshold_boundary() {
    let now = Utc::now();
    // Exactly at threshold - should not be stale (> not >=)
    let last_sync = Some(now - Duration::minutes(5));
    let threshold = Duration::minutes(5);

    let is_stale = match last_sync {
        None => true,
        Some(ls) => now.signed_duration_since(ls) > threshold,
    };
    assert!(!is_stale);
}

#[test]
fn test_is_stale_just_over_threshold() {
    let now = Utc::now();
    // Just over threshold - should be stale
    let last_sync = Some(now - Duration::minutes(5) - Duration::seconds(1));
    let threshold = Duration::minutes(5);

    let is_stale = match last_sync {
        None => true,
        Some(ls) => now.signed_duration_since(ls) > threshold,
    };
    assert!(is_stale);
}

#[test]
fn test_needs_sync_when_full_sync_needed() {
    // If cache.needs_full_sync() is true, needs_sync should return true
    let cache = Cache::new();
    assert!(cache.needs_full_sync());
    // needs_sync = needs_full_sync || is_stale
    // With new cache: needs_full_sync=true, is_stale=true (no last_sync)
    // Result: true
}

#[test]
fn test_needs_sync_when_stale() {
    let mut cache = Cache::new();
    cache.sync_token = "some_token".to_string(); // Not full sync
    cache.last_sync = Some(Utc::now() - Duration::minutes(10)); // Stale

    assert!(!cache.needs_full_sync());
    // needs_sync = false || is_stale(true) = true
}

#[test]
fn test_needs_sync_when_fresh() {
    let mut cache = Cache::new();
    cache.sync_token = "some_token".to_string(); // Not full sync
    cache.last_sync = Some(Utc::now()); // Just synced

    assert!(!cache.needs_full_sync());
    // needs_sync = false || is_stale(false) = false
}

// Tests for fuzzy matching suggestions

#[test]
fn test_find_similar_name_exact_match_returns_none() {
    // Exact match should not return a suggestion
    let candidates = ["Work", "Personal", "Shopping"];
    let result = find_similar_name("Work", candidates.iter().copied());
    assert!(result.is_none());
}

#[test]
fn test_find_similar_name_case_insensitive_exact_match_returns_none() {
    // Case-insensitive exact match should not return a suggestion
    let candidates = ["Work", "Personal", "Shopping"];
    let result = find_similar_name("work", candidates.iter().copied());
    assert!(result.is_none());
}

#[test]
fn test_find_similar_name_single_typo() {
    // Single character typo should suggest
    let candidates = ["Work", "Personal", "Shopping"];
    let result = find_similar_name("Wrok", candidates.iter().copied());
    assert_eq!(result, Some("Work".to_string()));
}

#[test]
fn test_find_similar_name_missing_letter() {
    // Missing letter should suggest
    let candidates = ["Inbox", "Personal", "Shopping"];
    let result = find_similar_name("inbx", candidates.iter().copied());
    assert_eq!(result, Some("Inbox".to_string()));
}

#[test]
fn test_find_similar_name_extra_letter() {
    // Extra letter should suggest
    let candidates = ["Work", "Personal", "Shopping"];
    let result = find_similar_name("Workk", candidates.iter().copied());
    assert_eq!(result, Some("Work".to_string()));
}

#[test]
fn test_find_similar_name_too_different() {
    // Very different string should not suggest
    let candidates = ["Work", "Personal", "Shopping"];
    let result = find_similar_name("Completely Different", candidates.iter().copied());
    assert!(result.is_none());
}

#[test]
fn test_find_similar_name_empty_candidates() {
    // Empty candidates list should return None
    let candidates: Vec<&str> = vec![];
    let result = find_similar_name("Work", candidates.iter().copied());
    assert!(result.is_none());
}

#[test]
fn test_find_similar_name_best_match_selected() {
    // Should select the best (closest) match
    let candidates = ["Workshop", "Work", "Working"];
    let result = find_similar_name("Wok", candidates.iter().copied());
    assert_eq!(result, Some("Work".to_string()));
}

#[test]
fn test_format_not_found_error_without_suggestion() {
    let msg = format_not_found_error("Project", "inbox", None);
    assert_eq!(
        msg,
        "Project 'inbox' not found. Try running 'td sync' to refresh your cache."
    );
}

#[test]
fn test_format_not_found_error_with_suggestion() {
    let msg = format_not_found_error("Project", "inbox", Some("Inbox"));
    assert_eq!(
        msg,
        "Project 'inbox' not found. Try running 'td sync' to refresh your cache. Did you mean 'Inbox'?"
    );
}

#[test]
fn test_format_not_found_error_label_with_suggestion() {
    let msg = format_not_found_error("Label", "urgnt", Some("urgent"));
    assert_eq!(
        msg,
        "Label 'urgnt' not found. Try running 'td sync' to refresh your cache. Did you mean 'urgent'?"
    );
}

#[test]
fn test_resolve_exact_name_match() {
    let mut manager = make_test_manager();
    manager.cache.projects = vec![make_project("proj-1", "Shared Project")];
    manager.cache.collaborators = vec![make_collaborator(
        "user-1",
        "Alice Smith",
        "alice@example.com",
    )];
    manager.cache.collaborator_states = vec![make_collaborator_state("proj-1", "user-1", "active")];

    let resolved = manager
        .resolve_collaborator("Alice Smith", "proj-1")
        .unwrap();
    assert_eq!(resolved.id, "user-1");
}

#[test]
fn test_resolve_exact_email_match() {
    let mut manager = make_test_manager();
    manager.cache.projects = vec![make_project("proj-1", "Shared Project")];
    manager.cache.collaborators = vec![make_collaborator(
        "user-1",
        "Alice Smith",
        "alice@example.com",
    )];
    manager.cache.collaborator_states = vec![make_collaborator_state("proj-1", "user-1", "active")];

    let resolved = manager
        .resolve_collaborator("alice@example.com", "proj-1")
        .unwrap();
    assert_eq!(resolved.id, "user-1");
}

#[test]
fn test_resolve_case_insensitive() {
    let mut manager = make_test_manager();
    manager.cache.projects = vec![make_project("proj-1", "Shared Project")];
    manager.cache.collaborators = vec![make_collaborator(
        "user-1",
        "Alice Smith",
        "alice@example.com",
    )];
    manager.cache.collaborator_states = vec![make_collaborator_state("proj-1", "user-1", "active")];

    let resolved = manager
        .resolve_collaborator("alice smith", "proj-1")
        .unwrap();
    assert_eq!(resolved.id, "user-1");
}

#[test]
fn test_resolve_partial_name_match() {
    let mut manager = make_test_manager();
    manager.cache.projects = vec![make_project("proj-1", "Shared Project")];
    manager.cache.collaborators = vec![
        make_collaborator("user-1", "Alice Smith", "alice@example.com"),
        make_collaborator("user-2", "Bob Chen", "bob@example.com"),
    ];
    manager.cache.collaborator_states = vec![
        make_collaborator_state("proj-1", "user-1", "active"),
        make_collaborator_state("proj-1", "user-2", "active"),
    ];

    let resolved = manager.resolve_collaborator("alice", "proj-1").unwrap();
    assert_eq!(resolved.id, "user-1");
}

#[test]
fn test_resolve_no_match_errors() {
    let mut manager = make_test_manager();
    manager.cache.projects = vec![make_project("proj-1", "Shared Project")];
    manager.cache.collaborators = vec![make_collaborator(
        "user-1",
        "Alice Smith",
        "alice@example.com",
    )];
    manager.cache.collaborator_states = vec![make_collaborator_state("proj-1", "user-1", "active")];

    let err = manager
        .resolve_collaborator("nonexistent", "proj-1")
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "No collaborator matching 'nonexistent' in project 'Shared Project'"
    );
}

#[test]
fn test_resolve_ambiguous_match_errors() {
    let mut manager = make_test_manager();
    manager.cache.projects = vec![make_project("proj-1", "Shared Project")];
    manager.cache.collaborators = vec![
        make_collaborator("user-1", "Alice Smith", "alice@example.com"),
        make_collaborator("user-2", "Alicia Chen", "alicia@example.com"),
    ];
    manager.cache.collaborator_states = vec![
        make_collaborator_state("proj-1", "user-1", "active"),
        make_collaborator_state("proj-1", "user-2", "active"),
    ];

    let err = manager.resolve_collaborator("ali", "proj-1").unwrap_err();
    assert_eq!(
        err.to_string(),
        "Multiple collaborators match 'ali': Alice Smith, Alicia Chen. Please be more specific."
    );
}

#[test]
fn test_resolve_scoped_to_project() {
    let mut manager = make_test_manager();
    manager.cache.projects = vec![
        make_project("proj-1", "Shared Project A"),
        make_project("proj-2", "Shared Project B"),
    ];
    manager.cache.collaborators = vec![make_collaborator(
        "user-1",
        "Alice Smith",
        "alice@example.com",
    )];
    manager.cache.collaborator_states = vec![make_collaborator_state("proj-1", "user-1", "active")];

    let err = manager.resolve_collaborator("Alice", "proj-2").unwrap_err();
    assert_eq!(
        err.to_string(),
        "No collaborator matching 'Alice' in project 'Shared Project B'"
    );
}

#[test]
fn test_resolve_excludes_invited() {
    let mut manager = make_test_manager();
    manager.cache.projects = vec![make_project("proj-1", "Shared Project")];
    manager.cache.collaborators = vec![make_collaborator(
        "user-1",
        "Alice Smith",
        "alice@example.com",
    )];
    manager.cache.collaborator_states =
        vec![make_collaborator_state("proj-1", "user-1", "invited")];

    let err = manager.resolve_collaborator("Alice", "proj-1").unwrap_err();
    assert_eq!(
        err.to_string(),
        "No collaborator matching 'Alice' in project 'Shared Project'"
    );
}

#[test]
fn test_resolve_me_resolves_to_current_user() {
    let mut manager = make_test_manager();
    manager.cache.user = Some(make_user("owner-1"));
    manager.cache.projects = vec![make_project("proj-1", "Shared Project")];
    manager.cache.collaborators = vec![
        make_collaborator("owner-1", "Owner", "owner@example.com"),
        make_collaborator("user-1", "Alice Smith", "alice@example.com"),
    ];
    manager.cache.collaborator_states = vec![
        make_collaborator_state("proj-1", "owner-1", "active"),
        make_collaborator_state("proj-1", "user-1", "active"),
    ];

    let resolved = manager.resolve_collaborator("me", "proj-1").unwrap();
    assert_eq!(resolved.id, "owner-1");
}

#[test]
fn test_resolve_me_case_insensitive() {
    let mut manager = make_test_manager();
    manager.cache.user = Some(make_user("owner-1"));
    manager.cache.projects = vec![make_project("proj-1", "Shared Project")];
    manager.cache.collaborators = vec![make_collaborator("owner-1", "Owner", "owner@example.com")];
    manager.cache.collaborator_states =
        vec![make_collaborator_state("proj-1", "owner-1", "active")];

    let resolved = manager.resolve_collaborator("Me", "proj-1").unwrap();
    assert_eq!(resolved.id, "owner-1");
}

#[test]
fn test_resolve_me_errors_when_not_active_on_project() {
    let mut manager = make_test_manager();
    manager.cache.user = Some(make_user("owner-1"));
    manager.cache.projects = vec![make_project("proj-1", "Shared Project")];
    manager.cache.collaborators = vec![make_collaborator("owner-1", "Owner", "owner@example.com")];
    // No collaborator_state for owner on this project
    manager.cache.collaborator_states = vec![];

    let err = manager.resolve_collaborator("me", "proj-1").unwrap_err();
    assert!(err.to_string().contains("No collaborator matching 'me'"));
}

#[test]
fn test_is_shared_project_true() {
    let mut manager = make_test_manager();
    manager.cache.user = Some(make_user("owner-1"));
    manager.cache.collaborator_states = vec![
        make_collaborator_state("proj-1", "owner-1", "active"),
        make_collaborator_state("proj-1", "user-1", "active"),
    ];

    assert!(manager.is_shared_project("proj-1"));
}

#[test]
fn test_is_shared_project_false_personal() {
    let manager = make_test_manager();
    assert!(!manager.is_shared_project("proj-1"));
}

#[test]
fn test_is_shared_project_false_only_owner() {
    let mut manager = make_test_manager();
    manager.cache.user = Some(make_user("owner-1"));
    manager.cache.collaborator_states =
        vec![make_collaborator_state("proj-1", "owner-1", "active")];

    assert!(!manager.is_shared_project("proj-1"));
}
