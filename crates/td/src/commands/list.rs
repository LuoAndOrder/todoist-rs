//! List command implementation.
//!
//! Lists tasks from the local cache, optionally filtered by various criteria.

use todoist_api::sync::Item;
use todoist_cache::filter::{FilterContext, FilterEvaluator, FilterParser};
use todoist_cache::{Cache, CacheStore, SyncManager};

use super::{CommandContext, Result};
use crate::cli::SortField;
use crate::output::{format_items_json, format_items_table};

/// Options for the list command.
#[derive(Debug)]
pub struct ListOptions {
    /// Filter expression.
    pub filter: Option<String>,
    /// Filter by project name or ID.
    pub project: Option<String>,
    /// Filter by label name.
    pub label: Option<String>,
    /// Filter by priority (1-4).
    pub priority: Option<u8>,
    /// Filter by section name.
    pub section: Option<String>,
    /// Show only overdue tasks.
    pub overdue: bool,
    /// Show only tasks without due date.
    pub no_due: bool,
    /// Limit results.
    pub limit: u32,
    /// Show all tasks (no limit).
    pub all: bool,
    /// Pagination cursor (not yet implemented).
    #[allow(dead_code)]
    pub cursor: Option<String>,
    /// Sort field.
    pub sort: Option<SortField>,
    /// Reverse sort order.
    pub reverse: bool,
}

/// Executes the list command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - List command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails or if the filter expression is invalid.
pub async fn execute(ctx: &CommandContext, opts: &ListOptions, token: &str) -> Result<()> {
    // Initialize sync manager
    let client = todoist_api::client::TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Only sync if explicitly requested with --sync flag
    if ctx.sync_first {
        if ctx.verbose {
            eprintln!("Syncing with Todoist...");
        }
        manager.sync().await?;
    }

    let cache = manager.cache();

    // Get items and apply filters
    let items = filter_items(cache, opts)?;

    // Sort items
    let items = sort_items(items, opts);

    // Apply limit
    let items = apply_limit(items, opts);

    // Output
    if ctx.json_output {
        let output = format_items_json(&items, cache)?;
        println!("{output}");
    } else if !ctx.quiet {
        let output = format_items_table(&items, cache, ctx.use_colors);
        print!("{output}");
    }

    Ok(())
}

/// Filters items based on the provided options.
fn filter_items<'a>(cache: &'a Cache, opts: &ListOptions) -> Result<Vec<&'a Item>> {
    let mut items: Vec<&Item> = cache
        .items
        .iter()
        .filter(|i| !i.is_deleted && !i.checked)
        .collect();

    // Apply filter expression if provided
    if let Some(filter_expr) = &opts.filter {
        let filter = FilterParser::parse(filter_expr)?;
        let context = FilterContext::new(&cache.projects, &cache.sections, &cache.labels);
        let evaluator = FilterEvaluator::new(&filter, &context);
        items.retain(|i| evaluator.matches(i));
    }

    // Apply project filter
    if let Some(project_name) = &opts.project {
        let project_name_lower = project_name.to_lowercase();
        let project_id = cache
            .projects
            .iter()
            .find(|p| p.name.to_lowercase() == project_name_lower || p.id == *project_name)
            .map(|p| &p.id);

        if let Some(pid) = project_id {
            items.retain(|i| &i.project_id == pid);
        } else {
            // No matching project, return empty
            return Ok(vec![]);
        }
    }

    // Apply label filter
    if let Some(label_name) = &opts.label {
        let label_lower = label_name.to_lowercase();
        items.retain(|i| i.labels.iter().any(|l| l.to_lowercase() == label_lower));
    }

    // Apply priority filter (convert user priority 1-4 to API priority 4-1)
    if let Some(priority) = opts.priority {
        let api_priority = 5 - priority as i32;
        items.retain(|i| i.priority == api_priority);
    }

    // Apply section filter
    if let Some(section_name) = &opts.section {
        let section_name_lower = section_name.to_lowercase();
        let section_id = cache
            .sections
            .iter()
            .find(|s| s.name.to_lowercase() == section_name_lower || s.id == *section_name)
            .map(|s| &s.id);

        if let Some(sid) = section_id {
            items.retain(|i| i.section_id.as_ref() == Some(sid));
        } else {
            return Ok(vec![]);
        }
    }

    // Apply overdue filter
    if opts.overdue {
        let today = chrono::Local::now().date_naive();
        items.retain(|i| {
            i.due.as_ref().is_some_and(|due| {
                chrono::NaiveDate::parse_from_str(&due.date, "%Y-%m-%d")
                    .ok()
                    .is_some_and(|d| d < today)
            })
        });
    }

    // Apply no_due filter
    if opts.no_due {
        items.retain(|i| i.due.is_none());
    }

    Ok(items)
}

/// Sorts items based on the provided options.
fn sort_items<'a>(mut items: Vec<&'a Item>, opts: &ListOptions) -> Vec<&'a Item> {
    if let Some(sort_field) = &opts.sort {
        match sort_field {
            SortField::Due => {
                items.sort_by(|a, b| {
                    let a_date = a.due.as_ref().map(|d| d.date.as_str());
                    let b_date = b.due.as_ref().map(|d| d.date.as_str());
                    // Items without due date go last
                    match (a_date, b_date) {
                        (None, None) => std::cmp::Ordering::Equal,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (Some(a), Some(b)) => a.cmp(b),
                    }
                });
            }
            SortField::Priority => {
                // Higher API priority (4) = higher user priority (p1)
                items.sort_by(|a, b| b.priority.cmp(&a.priority));
            }
            SortField::Created => {
                items.sort_by(|a, b| {
                    let a_date = a.added_at.as_deref();
                    let b_date = b.added_at.as_deref();
                    match (a_date, b_date) {
                        (None, None) => std::cmp::Ordering::Equal,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (Some(a), Some(b)) => a.cmp(b),
                    }
                });
            }
            SortField::Project => {
                items.sort_by(|a, b| a.project_id.cmp(&b.project_id));
            }
        }
    }

    if opts.reverse {
        items.reverse();
    }

    items
}

/// Applies the limit to the items.
fn apply_limit<'a>(items: Vec<&'a Item>, opts: &ListOptions) -> Vec<&'a Item> {
    if opts.all {
        items
    } else {
        items.into_iter().take(opts.limit as usize).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_options_defaults() {
        let opts = ListOptions {
            filter: None,
            project: None,
            label: None,
            priority: None,
            section: None,
            overdue: false,
            no_due: false,
            limit: 50,
            all: false,
            cursor: None,
            sort: None,
            reverse: false,
        };

        assert!(!opts.all);
        assert_eq!(opts.limit, 50);
    }
}
