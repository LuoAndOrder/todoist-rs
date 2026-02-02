//! Merge logic for applying sync responses to the cache.
//!
//! This module handles both full and incremental sync operations,
//! merging incoming resources with the existing cache state.

use std::collections::HashMap;

use chrono::Utc;
use todoist_api_rs::sync::SyncResponse;

use crate::Cache;

/// Applies a sync response to the cache, merging in changes.
///
/// This function handles both full and incremental sync responses:
/// - Updates the sync token and timestamps
/// - For full sync: replaces all resources with the response data
/// - For incremental sync: merges changes (add/update/delete by ID)
///
/// Resources with `is_deleted: true` are removed from the cache.
pub(crate) fn apply_sync_response(cache: &mut Cache, response: &SyncResponse) {
    let now = Utc::now();

    // Update sync token
    cache.sync_token = response.sync_token.clone();
    cache.last_sync = Some(now);

    // If this is a full sync, update full_sync_date_utc
    if response.full_sync {
        // Use the server-provided timestamp if available, otherwise use current time
        cache.full_sync_date_utc = response
            .full_sync_date_utc
            .as_ref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .or(Some(now));
    }

    if response.full_sync {
        // Full sync: replace all data (filter out deleted items)
        cache.items = response
            .items
            .iter()
            .filter(|i| !i.is_deleted)
            .cloned()
            .collect();
        cache.projects = response
            .projects
            .iter()
            .filter(|p| !p.is_deleted)
            .cloned()
            .collect();
        cache.labels = response
            .labels
            .iter()
            .filter(|l| !l.is_deleted)
            .cloned()
            .collect();
        cache.sections = response
            .sections
            .iter()
            .filter(|s| !s.is_deleted)
            .cloned()
            .collect();
        cache.notes = response
            .notes
            .iter()
            .filter(|n| !n.is_deleted)
            .cloned()
            .collect();
        cache.project_notes = response
            .project_notes
            .iter()
            .filter(|n| !n.is_deleted)
            .cloned()
            .collect();
        cache.reminders = response
            .reminders
            .iter()
            .filter(|r| !r.is_deleted)
            .cloned()
            .collect();
        cache.filters = response
            .filters
            .iter()
            .filter(|f| !f.is_deleted)
            .cloned()
            .collect();
    } else {
        // Incremental sync: merge changes
        merge_resources(
            &mut cache.items,
            &response.items,
            |i| &i.id,
            |i| i.is_deleted,
        );
        merge_resources(
            &mut cache.projects,
            &response.projects,
            |p| &p.id,
            |p| p.is_deleted,
        );
        merge_resources(
            &mut cache.labels,
            &response.labels,
            |l| &l.id,
            |l| l.is_deleted,
        );
        merge_resources(
            &mut cache.sections,
            &response.sections,
            |s| &s.id,
            |s| s.is_deleted,
        );
        merge_resources(
            &mut cache.notes,
            &response.notes,
            |n| &n.id,
            |n| n.is_deleted,
        );
        merge_resources(
            &mut cache.project_notes,
            &response.project_notes,
            |n| &n.id,
            |n| n.is_deleted,
        );
        merge_resources(
            &mut cache.reminders,
            &response.reminders,
            |r| &r.id,
            |r| r.is_deleted,
        );
        merge_resources(
            &mut cache.filters,
            &response.filters,
            |f| &f.id,
            |f| f.is_deleted,
        );
    }

    // User is always replaced if present in response
    if response.user.is_some() {
        cache.user = response.user.clone();
    }

    // Rebuild indexes after applying changes
    cache.rebuild_indexes();
}

/// Applies a mutation response to the cache.
///
/// This function is similar to [`apply_sync_response`] but is specifically
/// designed for write operation (mutation) responses. It:
/// - Updates the sync_token from the response
/// - Updates the last_sync timestamp
/// - Merges any resources returned in the response (add/update/delete by ID)
///
/// Unlike full sync responses, mutation responses always use incremental
/// merge logic since they only contain affected resources.
///
/// Note: The `temp_id_mapping` from the response should be used by the caller
/// to resolve temporary IDs before calling this function, or the caller can
/// use the returned response's `temp_id_mapping` to look up real IDs.
pub(crate) fn apply_mutation_response(cache: &mut Cache, response: &SyncResponse) {
    let now = Utc::now();

    // Update sync token - critical for subsequent syncs
    cache.sync_token = response.sync_token.clone();
    cache.last_sync = Some(now);

    // Merge resources using incremental logic (mutations never do full sync)
    // Even if the response has full_sync: true, we treat it as incremental
    // because we're only applying the affected resources from a mutation
    merge_resources(
        &mut cache.items,
        &response.items,
        |i| &i.id,
        |i| i.is_deleted,
    );
    merge_resources(
        &mut cache.projects,
        &response.projects,
        |p| &p.id,
        |p| p.is_deleted,
    );
    merge_resources(
        &mut cache.labels,
        &response.labels,
        |l| &l.id,
        |l| l.is_deleted,
    );
    merge_resources(
        &mut cache.sections,
        &response.sections,
        |s| &s.id,
        |s| s.is_deleted,
    );
    merge_resources(
        &mut cache.notes,
        &response.notes,
        |n| &n.id,
        |n| n.is_deleted,
    );
    merge_resources(
        &mut cache.project_notes,
        &response.project_notes,
        |n| &n.id,
        |n| n.is_deleted,
    );
    merge_resources(
        &mut cache.reminders,
        &response.reminders,
        |r| &r.id,
        |r| r.is_deleted,
    );
    merge_resources(
        &mut cache.filters,
        &response.filters,
        |f| &f.id,
        |f| f.is_deleted,
    );

    // User is replaced if present in response
    if response.user.is_some() {
        cache.user = response.user.clone();
    }

    // Rebuild indexes after applying changes
    cache.rebuild_indexes();
}

/// Merges a list of resources from a sync response into the cache.
///
/// For each resource in the response:
/// - If `is_deleted` is true: remove from cache
/// - If resource exists in cache: update it
/// - Otherwise: add it
///
/// Uses a two-phase approach to avoid double cloning of ID strings:
/// 1. Build index using borrowed references (no cloning)
/// 2. Categorize incoming items into updates, inserts, and deletions
/// 3. Apply changes to the existing vec
///
/// This reduces memory allocations during merge operations, especially
/// for large caches with 10,000+ items.
pub(crate) fn merge_resources<T, F, D>(existing: &mut Vec<T>, incoming: &[T], get_id: F, is_deleted: D)
where
    T: Clone,
    F: Fn(&T) -> &str,
    D: Fn(&T) -> bool,
{
    // Phase 1: Build index using borrowed references (no cloning needed)
    // Pre-allocate with exact capacity to avoid reallocations during collection
    let mut index: HashMap<&str, usize> = HashMap::with_capacity(existing.len());
    for (i, item) in existing.iter().enumerate() {
        index.insert(get_id(item), i);
    }

    // Phase 2: Categorize incoming items
    // Pre-allocate with estimated capacities based on typical usage patterns
    let mut updates: Vec<(usize, &T)> = Vec::with_capacity(incoming.len());
    let mut inserts: Vec<&T> = Vec::with_capacity(incoming.len() / 4);
    let mut to_remove: Vec<usize> = Vec::with_capacity(incoming.len() / 10);

    for item in incoming {
        let id = get_id(item);
        let pos = index.get(id).copied();

        if is_deleted(item) {
            // Mark for removal if exists
            if let Some(idx) = pos {
                to_remove.push(idx);
            }
        } else if let Some(idx) = pos {
            // Update existing
            updates.push((idx, item));
        } else {
            // New item
            inserts.push(item);
        }
    }

    // Phase 3: Apply updates (vec length unchanged)
    for (idx, item) in updates {
        existing[idx] = item.clone();
    }

    // Phase 4: Append new items (reserve capacity before extending)
    existing.reserve(inserts.len());
    existing.extend(inserts.into_iter().cloned());

    // Phase 5: Remove deleted items in reverse order to preserve indices
    to_remove.sort_unstable();
    for idx in to_remove.into_iter().rev() {
        existing.remove(idx);
    }
}
