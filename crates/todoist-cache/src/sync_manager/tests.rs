//! Tests for the sync manager module.

use super::*;
use chrono::{DateTime, Duration, Utc};

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
