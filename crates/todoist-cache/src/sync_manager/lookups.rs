//! Resource lookup functionality for SyncManager.
//!
//! This module provides lookup methods for finding projects, sections, labels,
//! and items in the cache with auto-sync fallback and fuzzy matching suggestions.

use strsim::levenshtein;
use todoist_api_rs::sync::{Item, Label, Project, Section};

use crate::{SyncError, SyncManager, SyncResult};

/// Maximum Levenshtein distance to consider a name as a suggestion.
const MAX_SUGGESTION_DISTANCE: usize = 3;

/// Formats the "not found" error message, optionally including a suggestion.
pub(crate) fn format_not_found_error(
    resource_type: &str,
    identifier: &str,
    suggestion: Option<&str>,
) -> String {
    let base = format!(
        "{} '{}' not found. Try running 'td sync' to refresh your cache.",
        resource_type, identifier
    );
    match suggestion {
        Some(s) => format!("{} Did you mean '{}'?", base, s),
        None => base,
    }
}

/// Finds the best matching name from a list of candidates using Levenshtein distance.
///
/// Returns the best match if its edit distance is within the threshold,
/// otherwise returns `None`.
pub(crate) fn find_similar_name<'a>(
    query: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Option<String> {
    let query_lower = query.to_lowercase();

    let (best_match, best_distance) = candidates
        .filter(|name| !name.is_empty())
        .map(|name| {
            let distance = levenshtein(&query_lower, &name.to_lowercase());
            (name.to_string(), distance)
        })
        .min_by_key(|(_, d)| *d)?;

    // Only suggest if the distance is within threshold and not an exact match
    if best_distance > 0 && best_distance <= MAX_SUGGESTION_DISTANCE {
        Some(best_match)
    } else {
        None
    }
}

/// Result of an item lookup by prefix.
pub(crate) enum ItemLookupResult<'a> {
    /// Found exactly one matching item.
    Found(&'a Item),
    /// Multiple items match the prefix (contains error message).
    Ambiguous(String),
    /// No matching item found.
    NotFound,
}

impl SyncManager {
    // ==================== Smart Lookup Methods ====================

    /// Resolves a project by name or ID, with auto-sync fallback.
    ///
    /// This method first attempts to find the project in the cache. If not found,
    /// it performs a sync and retries the lookup. This provides a seamless experience
    /// where users can reference recently-created projects without manual syncing.
    ///
    /// # Arguments
    ///
    /// * `name_or_id` - The project name (case-insensitive) or ID to search for
    ///
    /// # Returns
    ///
    /// A reference to the matching `Project` from the cache.
    ///
    /// # Errors
    ///
    /// Returns `SyncError::NotFound` if the project cannot be found even after syncing.
    /// Returns `SyncError::Api` if the sync operation fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_cache_rs::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token")?;
    ///     let store = CacheStore::new()?;
    ///     let mut manager = SyncManager::new(client, store)?;
    ///
    ///     // Find by name (case-insensitive)
    ///     let project = manager.resolve_project("work").await?;
    ///     println!("Found project: {} ({})", project.name, project.id);
    ///
    ///     // Find by ID
    ///     let project = manager.resolve_project("12345678").await?;
    ///     println!("Found project: {}", project.name);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn resolve_project(&mut self, name_or_id: &str) -> SyncResult<&Project> {
        // Try cache first
        if self.find_project_in_cache(name_or_id).is_some() {
            // Re-borrow to return reference (can't return from the if-let due to borrow checker)
            return Ok(self.find_project_in_cache(name_or_id).unwrap());
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        self.find_project_in_cache(name_or_id).ok_or_else(|| {
            // Find similar project names for suggestion
            let suggestion = find_similar_name(
                name_or_id,
                self.cache()
                    .projects
                    .iter()
                    .filter(|p| !p.is_deleted)
                    .map(|p| p.name.as_str()),
            );
            SyncError::NotFound {
                resource_type: "Project",
                identifier: name_or_id.to_string(),
                suggestion,
            }
        })
    }

    /// Helper to find a project in the cache by name or ID.
    ///
    /// Searches for non-deleted projects where either:
    /// - The name matches (case-insensitive)
    /// - The ID matches exactly
    fn find_project_in_cache(&self, name_or_id: &str) -> Option<&Project> {
        let name_lower = name_or_id.to_lowercase();
        self.cache()
            .projects
            .iter()
            .find(|p| !p.is_deleted && (p.name.to_lowercase() == name_lower || p.id == name_or_id))
    }

    /// Resolves a section by name or ID, with auto-sync fallback.
    ///
    /// This method first attempts to find the section in the cache. If not found,
    /// it performs a sync and retries the lookup.
    ///
    /// # Arguments
    ///
    /// * `name_or_id` - The section name (case-insensitive) or ID to search for
    /// * `project_id` - Optional project ID to scope the search. If provided, only
    ///   sections in that project are considered for name matching.
    ///
    /// # Returns
    ///
    /// A reference to the matching `Section` from the cache.
    ///
    /// # Errors
    ///
    /// Returns `SyncError::NotFound` if the section cannot be found even after syncing.
    /// Returns `SyncError::Api` if the sync operation fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_cache_rs::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token")?;
    ///     let store = CacheStore::new()?;
    ///     let mut manager = SyncManager::new(client, store)?;
    ///
    ///     // Find by name within a specific project
    ///     let section = manager.resolve_section("To Do", Some("12345678")).await?;
    ///     println!("Found section: {} ({})", section.name, section.id);
    ///
    ///     // Find by ID (project_id is ignored for ID lookups)
    ///     let section = manager.resolve_section("87654321", None).await?;
    ///     println!("Found section: {}", section.name);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn resolve_section(
        &mut self,
        name_or_id: &str,
        project_id: Option<&str>,
    ) -> SyncResult<&Section> {
        // Try cache first
        if self.find_section_in_cache(name_or_id, project_id).is_some() {
            return Ok(self.find_section_in_cache(name_or_id, project_id).unwrap());
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        self.find_section_in_cache(name_or_id, project_id)
            .ok_or_else(|| {
                // Find similar section names for suggestion (within same project if specified)
                let suggestion = find_similar_name(
                    name_or_id,
                    self.cache()
                        .sections
                        .iter()
                        .filter(|s| {
                            !s.is_deleted && project_id.is_none_or(|pid| s.project_id == pid)
                        })
                        .map(|s| s.name.as_str()),
                );
                SyncError::NotFound {
                    resource_type: "Section",
                    identifier: name_or_id.to_string(),
                    suggestion,
                }
            })
    }

    /// Helper to find a section in the cache by name or ID.
    ///
    /// Searches for non-deleted sections where either:
    /// - The ID matches exactly (ignores project_id filter)
    /// - The name matches (case-insensitive) and optionally within the specified project
    fn find_section_in_cache(
        &self,
        name_or_id: &str,
        project_id: Option<&str>,
    ) -> Option<&Section> {
        let name_lower = name_or_id.to_lowercase();
        self.cache().sections.iter().find(|s| {
            if s.is_deleted {
                return false;
            }
            // ID match takes precedence (ignores project filter)
            if s.id == name_or_id {
                return true;
            }
            // Name match with optional project filter
            if s.name.to_lowercase() == name_lower {
                return project_id.is_none_or(|pid| s.project_id == pid);
            }
            false
        })
    }

    /// Resolves a label by name or ID, with auto-sync fallback.
    ///
    /// This method first attempts to find the label in the cache. If not found,
    /// it performs a sync and retries the lookup.
    ///
    /// # Arguments
    ///
    /// * `name_or_id` - The label name (case-insensitive) or ID to search for
    ///
    /// # Returns
    ///
    /// A reference to the matching `Label` from the cache.
    ///
    /// # Errors
    ///
    /// Returns `SyncError::NotFound` if the label cannot be found even after syncing.
    /// Returns `SyncError::Api` if the sync operation fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_cache_rs::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token")?;
    ///     let store = CacheStore::new()?;
    ///     let mut manager = SyncManager::new(client, store)?;
    ///
    ///     // Find by name (case-insensitive)
    ///     let label = manager.resolve_label("urgent").await?;
    ///     println!("Found label: {} ({})", label.name, label.id);
    ///
    ///     // Find by ID
    ///     let label = manager.resolve_label("12345678").await?;
    ///     println!("Found label: {}", label.name);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn resolve_label(&mut self, name_or_id: &str) -> SyncResult<&Label> {
        // Try cache first
        if self.find_label_in_cache(name_or_id).is_some() {
            return Ok(self.find_label_in_cache(name_or_id).unwrap());
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        self.find_label_in_cache(name_or_id).ok_or_else(|| {
            // Find similar label names for suggestion
            let suggestion = find_similar_name(
                name_or_id,
                self.cache()
                    .labels
                    .iter()
                    .filter(|l| !l.is_deleted)
                    .map(|l| l.name.as_str()),
            );
            SyncError::NotFound {
                resource_type: "Label",
                identifier: name_or_id.to_string(),
                suggestion,
            }
        })
    }

    /// Helper to find a label in the cache by name or ID.
    ///
    /// Searches for non-deleted labels where either:
    /// - The name matches (case-insensitive)
    /// - The ID matches exactly
    fn find_label_in_cache(&self, name_or_id: &str) -> Option<&Label> {
        let name_lower = name_or_id.to_lowercase();
        self.cache()
            .labels
            .iter()
            .find(|l| !l.is_deleted && (l.name.to_lowercase() == name_lower || l.id == name_or_id))
    }

    /// Resolves an item (task) by ID, with auto-sync fallback.
    ///
    /// This method first attempts to find the item in the cache. If not found,
    /// it performs a sync and retries the lookup.
    ///
    /// Note: Unlike projects, sections, and labels, items can only be looked up
    /// by ID since task content is not guaranteed to be unique.
    ///
    /// # Arguments
    ///
    /// * `id` - The item ID to search for
    ///
    /// # Returns
    ///
    /// A reference to the matching `Item` from the cache.
    ///
    /// # Errors
    ///
    /// Returns `SyncError::NotFound` if the item cannot be found even after syncing.
    /// Returns `SyncError::Api` if the sync operation fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_cache_rs::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token")?;
    ///     let store = CacheStore::new()?;
    ///     let mut manager = SyncManager::new(client, store)?;
    ///
    ///     // Find by ID
    ///     let item = manager.resolve_item("12345678").await?;
    ///     println!("Found item: {} ({})", item.content, item.id);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn resolve_item(&mut self, id: &str) -> SyncResult<&Item> {
        // Try cache first
        if self.find_item_in_cache(id).is_some() {
            return Ok(self.find_item_in_cache(id).unwrap());
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        self.find_item_in_cache(id)
            .ok_or_else(|| SyncError::NotFound {
                resource_type: "Item",
                identifier: id.to_string(),
                suggestion: None, // Items are looked up by ID, no name suggestions
            })
    }

    /// Helper to find an item in the cache by ID.
    ///
    /// Searches for non-deleted items where the ID matches exactly.
    fn find_item_in_cache(&self, id: &str) -> Option<&Item> {
        self.cache()
            .items
            .iter()
            .find(|i| !i.is_deleted && i.id == id)
    }

    /// Resolves an item (task) by ID or unique prefix, with auto-sync fallback.
    ///
    /// This method first attempts to find the item in the cache by exact ID match
    /// or unique prefix. If not found, it performs a sync and retries the lookup.
    ///
    /// # Arguments
    ///
    /// * `id_or_prefix` - The item ID or unique prefix to search for
    /// * `require_checked` - If `Some(true)`, only match completed items.
    ///   If `Some(false)`, only match uncompleted items.
    ///   If `None`, match any item regardless of completion status.
    ///
    /// # Returns
    ///
    /// A reference to the matching `Item` from the cache.
    ///
    /// # Errors
    ///
    /// Returns `SyncError::NotFound` if the item cannot be found even after syncing.
    /// Returns `SyncError::Api` if the sync operation fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use todoist_api_rs::client::TodoistClient;
    /// use todoist_cache_rs::{CacheStore, SyncManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = TodoistClient::new("your-api-token")?;
    ///     let store = CacheStore::new()?;
    ///     let mut manager = SyncManager::new(client, store)?;
    ///
    ///     // Find uncompleted task by ID prefix
    ///     let item = manager.resolve_item_by_prefix("abc123", Some(false)).await?;
    ///     println!("Found item: {} ({})", item.content, item.id);
    ///
    ///     // Find any task by prefix (completed or not)
    ///     let item = manager.resolve_item_by_prefix("def456", None).await?;
    ///     println!("Found item: {}", item.content);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn resolve_item_by_prefix(
        &mut self,
        id_or_prefix: &str,
        require_checked: Option<bool>,
    ) -> SyncResult<&Item> {
        // Try cache first
        match self.find_item_by_prefix_in_cache(id_or_prefix, require_checked) {
            ItemLookupResult::Found(_) => {
                // Re-lookup to return reference (borrow checker limitation)
                if let ItemLookupResult::Found(item) =
                    self.find_item_by_prefix_in_cache(id_or_prefix, require_checked)
                {
                    return Ok(item);
                }
                unreachable!()
            }
            ItemLookupResult::Ambiguous(msg) => {
                return Err(SyncError::NotFound {
                    resource_type: "Item",
                    identifier: msg,
                    suggestion: None,
                });
            }
            ItemLookupResult::NotFound => {
                // Continue to sync
            }
        }

        // Not found - sync and retry
        self.sync().await?;

        // Try again after sync
        match self.find_item_by_prefix_in_cache(id_or_prefix, require_checked) {
            ItemLookupResult::Found(item) => Ok(item),
            ItemLookupResult::Ambiguous(msg) => Err(SyncError::NotFound {
                resource_type: "Item",
                identifier: msg,
                suggestion: None,
            }),
            ItemLookupResult::NotFound => Err(SyncError::NotFound {
                resource_type: "Item",
                identifier: id_or_prefix.to_string(),
                suggestion: None, // Items are looked up by ID, no name suggestions
            }),
        }
    }

    /// Helper to find an item in the cache by ID or unique prefix.
    ///
    /// Returns the found item, an ambiguity error message, or not found.
    fn find_item_by_prefix_in_cache(
        &self,
        id_or_prefix: &str,
        require_checked: Option<bool>,
    ) -> ItemLookupResult<'_> {
        // First try exact match
        if let Some(item) = self.cache().items.iter().find(|i| {
            !i.is_deleted
                && i.id == id_or_prefix
                && require_checked.is_none_or(|checked| i.checked == checked)
        }) {
            return ItemLookupResult::Found(item);
        }

        // Try prefix match
        let matches: Vec<&Item> = self
            .cache()
            .items
            .iter()
            .filter(|i| {
                !i.is_deleted
                    && i.id.starts_with(id_or_prefix)
                    && require_checked.is_none_or(|checked| i.checked == checked)
            })
            .collect();

        match matches.len() {
            0 => ItemLookupResult::NotFound,
            1 => ItemLookupResult::Found(matches[0]),
            _ => {
                // Ambiguous prefix - provide helpful error message
                let mut msg = format!(
                    "Ambiguous task ID \"{}\"\n\nMultiple tasks match this prefix:",
                    id_or_prefix
                );
                for item in matches.iter().take(5) {
                    let prefix = &item.id[..6.min(item.id.len())];
                    msg.push_str(&format!("\n  {}  {}", prefix, item.content));
                }
                if matches.len() > 5 {
                    msg.push_str(&format!("\n  ... and {} more", matches.len() - 5));
                }
                msg.push_str("\n\nPlease use a longer prefix.");
                ItemLookupResult::Ambiguous(msg)
            }
        }
    }
}
