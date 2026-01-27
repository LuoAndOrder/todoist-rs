//! Today command implementation.
//!
//! Shows today's agenda: tasks due today and optionally overdue/upcoming tasks.

use chrono::{Local, NaiveDate, Utc};
use todoist_api_rs::sync::Item;
use todoist_cache_rs::{Cache, CacheStore, SyncManager};

use super::{CommandContext, Result};

/// Options for the today command.
#[derive(Debug)]
pub struct TodayOptions {
    /// Include overdue tasks (default: true).
    pub include_overdue: bool,
    /// Include tasks due within N days.
    pub include_upcoming: Option<u32>,
}

/// Result of the today command containing categorized tasks.
pub struct TodayResult<'a> {
    /// Overdue tasks.
    pub overdue: Vec<&'a Item>,
    /// Tasks due today.
    pub today: Vec<&'a Item>,
    /// Upcoming tasks (within N days).
    pub upcoming: Vec<&'a Item>,
    /// Number of days for upcoming tasks.
    pub upcoming_days: Option<u32>,
}

/// Executes the today command.
///
/// # Arguments
///
/// * `ctx` - Command context with output settings
/// * `opts` - Today command options
/// * `token` - API token
///
/// # Errors
///
/// Returns an error if syncing fails.
pub async fn execute(ctx: &CommandContext, opts: &TodayOptions, token: &str) -> Result<()> {
    // Initialize sync manager
    let client = todoist_api_rs::client::TodoistClient::new(token);
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    // Sync if needed
    let now = Utc::now();
    if manager.needs_sync(now) {
        if ctx.verbose {
            eprintln!("Syncing with Todoist...");
        }
        manager.sync().await?;
    }

    let cache = manager.cache();

    // Categorize tasks
    let result = categorize_tasks(cache, opts);

    // Output
    if ctx.json_output {
        let output = format_today_json(&result, cache)?;
        println!("{output}");
    } else if !ctx.quiet {
        let output = format_today_table(&result, cache, ctx.use_colors);
        print!("{output}");
    }

    Ok(())
}

/// Categorizes tasks into overdue, today, and upcoming.
fn categorize_tasks<'a>(cache: &'a Cache, opts: &TodayOptions) -> TodayResult<'a> {
    let local_today = Local::now().date_naive();

    let mut overdue: Vec<&Item> = Vec::new();
    let mut today: Vec<&Item> = Vec::new();
    let mut upcoming: Vec<&Item> = Vec::new();

    // Calculate upcoming cutoff date if needed
    let upcoming_cutoff = opts.include_upcoming.map(|days| local_today + chrono::Duration::days(days as i64));

    for item in &cache.items {
        // Skip deleted and completed items
        if item.is_deleted || item.checked {
            continue;
        }

        // Get due date
        let Some(due) = &item.due else {
            continue; // Skip items without due date
        };

        let Ok(due_date) = NaiveDate::parse_from_str(&due.date, "%Y-%m-%d") else {
            continue; // Skip items with unparseable due dates
        };

        if due_date < local_today {
            // Overdue
            if opts.include_overdue {
                overdue.push(item);
            }
        } else if due_date == local_today {
            // Due today
            today.push(item);
        } else if let Some(cutoff) = upcoming_cutoff {
            // Upcoming (within N days)
            if due_date <= cutoff {
                upcoming.push(item);
            }
        }
    }

    // Sort each category by due date, then by priority (highest first)
    let sort_by_due_and_priority = |a: &&Item, b: &&Item| {
        let a_date = a.due.as_ref().map(|d| d.date.as_str()).unwrap_or("");
        let b_date = b.due.as_ref().map(|d| d.date.as_str()).unwrap_or("");
        match a_date.cmp(b_date) {
            std::cmp::Ordering::Equal => b.priority.cmp(&a.priority), // Higher priority first
            other => other,
        }
    };

    overdue.sort_by(sort_by_due_and_priority);
    today.sort_by(|a, b| b.priority.cmp(&a.priority)); // Just by priority for today
    upcoming.sort_by(sort_by_due_and_priority);

    TodayResult {
        overdue,
        today,
        upcoming,
        upcoming_days: opts.include_upcoming,
    }
}

/// Formats the today result as JSON.
fn format_today_json(
    result: &TodayResult,
    cache: &Cache,
) -> std::result::Result<String, serde_json::Error> {
    use serde::Serialize;

    #[derive(Serialize)]
    struct TodayOutput<'a> {
        overdue: Vec<TaskOutput<'a>>,
        today: Vec<TaskOutput<'a>>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        upcoming: Vec<TaskOutput<'a>>,
        total_count: usize,
    }

    #[derive(Serialize)]
    struct TaskOutput<'a> {
        id: &'a str,
        content: &'a str,
        priority: u8,
        due: Option<&'a str>,
        due_time: Option<&'a str>,
        project_id: &'a str,
        project_name: Option<&'a str>,
        labels: &'a [String],
    }

    fn to_task_output<'a>(item: &'a Item, cache: &'a Cache) -> TaskOutput<'a> {
        let project_name = cache
            .projects
            .iter()
            .find(|p| p.id == item.project_id)
            .map(|p| p.name.as_str());

        let (due, due_time) = item.due.as_ref().map_or((None, None), |d| {
            (Some(d.date.as_str()), d.datetime.as_deref())
        });

        TaskOutput {
            id: &item.id,
            content: &item.content,
            // Convert API priority (4=highest) to user priority (1=highest)
            priority: (5 - item.priority) as u8,
            due,
            due_time,
            project_id: &item.project_id,
            project_name,
            labels: &item.labels,
        }
    }

    let output = TodayOutput {
        overdue: result.overdue.iter().map(|i| to_task_output(i, cache)).collect(),
        today: result.today.iter().map(|i| to_task_output(i, cache)).collect(),
        upcoming: result.upcoming.iter().map(|i| to_task_output(i, cache)).collect(),
        total_count: result.overdue.len() + result.today.len() + result.upcoming.len(),
    };

    serde_json::to_string_pretty(&output)
}

/// Formats the today result as a human-readable table.
fn format_today_table(result: &TodayResult, cache: &Cache, use_colors: bool) -> String {
    use owo_colors::OwoColorize;

    let total = result.overdue.len() + result.today.len() + result.upcoming.len();

    if total == 0 {
        return "No tasks for today.\n".to_string();
    }

    let mut output = String::new();

    // Header
    let task_word = if total == 1 { "task" } else { "tasks" };
    let header = format!("Today's Tasks ({} {})", total, task_word);
    if use_colors {
        output.push_str(&format!("{}\n\n", header.bold()));
    } else {
        output.push_str(&format!("{}\n\n", header));
    }

    // Overdue section
    if !result.overdue.is_empty() {
        let section_header = "OVERDUE";
        if use_colors {
            output.push_str(&format!("{}\n", section_header.red().bold()));
        } else {
            output.push_str(&format!("{}\n", section_header));
        }
        for item in &result.overdue {
            output.push_str(&format_task_line(item, cache, use_colors));
        }
        output.push('\n');
    }

    // Due today section
    if !result.today.is_empty() {
        let section_header = "DUE TODAY";
        if use_colors {
            output.push_str(&format!("{}\n", section_header.yellow().bold()));
        } else {
            output.push_str(&format!("{}\n", section_header));
        }
        for item in &result.today {
            output.push_str(&format_task_line(item, cache, use_colors));
        }
        output.push('\n');
    }

    // Upcoming section
    if !result.upcoming.is_empty() {
        let section_header = if let Some(days) = result.upcoming_days {
            format!("UPCOMING (next {} days)", days)
        } else {
            "UPCOMING".to_string()
        };
        if use_colors {
            output.push_str(&format!("{}\n", section_header.cyan().bold()));
        } else {
            output.push_str(&format!("{}\n", section_header));
        }
        for item in &result.upcoming {
            output.push_str(&format_task_line(item, cache, use_colors));
        }
    }

    output
}

/// Formats a single task line for the today view.
fn format_task_line(item: &Item, cache: &Cache, use_colors: bool) -> String {
    use owo_colors::OwoColorize;

    let priority = format_priority(item.priority, use_colors);
    let due = format_due_for_today(item.due.as_ref(), use_colors);
    let project_name = cache
        .projects
        .iter()
        .find(|p| p.id == item.project_id)
        .map(|p| truncate_str(&p.name, 20))
        .unwrap_or_default();

    format!(
        "  {}  {:<12} {:<35} {}\n",
        priority,
        due,
        item.content,
        if use_colors {
            project_name.dimmed().to_string()
        } else {
            project_name
        }
    )
}

/// Formats priority for display.
fn format_priority(api_priority: i32, use_colors: bool) -> String {
    use owo_colors::OwoColorize;

    let user_priority = 5 - api_priority;
    let label = format!("p{user_priority}");

    if use_colors {
        match user_priority {
            1 => label.red().to_string(),
            2 => label.yellow().to_string(),
            3 => label.blue().to_string(),
            _ => label.dimmed().to_string(),
        }
    } else {
        label
    }
}

/// Formats due date for the today view.
fn format_due_for_today(due: Option<&todoist_api_rs::sync::Due>, use_colors: bool) -> String {
    use owo_colors::OwoColorize;

    let Some(due) = due else {
        return String::new();
    };

    let Ok(date) = NaiveDate::parse_from_str(&due.date, "%Y-%m-%d") else {
        return due.date.clone();
    };

    let local_today = Local::now().date_naive();
    let tomorrow = local_today + chrono::Duration::days(1);

    // Format the date part
    let date_str = if date == local_today {
        "Today".to_string()
    } else if date == tomorrow {
        "Tomorrow".to_string()
    } else if date < local_today {
        let days = (local_today - date).num_days();
        if days == 1 {
            "Yesterday".to_string()
        } else {
            format!("{} days ago", days)
        }
    } else {
        date.format("%b %d").to_string()
    };

    // Add time if available
    let display = if let Some(ref datetime) = due.datetime {
        if let Some(time_part) = datetime.split('T').nth(1) {
            let time_clean = time_part.trim_end_matches('Z');
            let hm: String = time_clean.split(':').take(2).collect::<Vec<_>>().join(":");
            if !hm.is_empty() {
                format!("{} {}", date_str, hm)
            } else {
                date_str
            }
        } else {
            date_str
        }
    } else {
        date_str
    };

    if use_colors {
        if date < local_today {
            display.red().to_string()
        } else if date == local_today {
            display.yellow().to_string()
        } else {
            display
        }
    } else {
        display
    }
}

/// Truncates a string to a maximum length.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_today_options_defaults() {
        let opts = TodayOptions {
            include_overdue: true,
            include_upcoming: None,
        };

        assert!(opts.include_overdue);
        assert!(opts.include_upcoming.is_none());
    }

    #[test]
    fn test_today_options_with_upcoming() {
        let opts = TodayOptions {
            include_overdue: true,
            include_upcoming: Some(3),
        };

        assert!(opts.include_overdue);
        assert_eq!(opts.include_upcoming, Some(3));
    }

    #[test]
    fn test_format_priority_no_colors() {
        assert_eq!(format_priority(4, false), "p1");
        assert_eq!(format_priority(3, false), "p2");
        assert_eq!(format_priority(2, false), "p3");
        assert_eq!(format_priority(1, false), "p4");
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("this is a long string", 10), "this is...");
    }
}
