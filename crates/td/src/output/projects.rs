//! Project output formatting.

use std::collections::HashMap;

use owo_colors::OwoColorize;
use serde::Serialize;
use todoist_api::sync::Project;
use todoist_cache::Cache;

use crate::commands::projects::{
    ProjectAddResult, ProjectArchiveResult, ProjectDeleteResult, ProjectEditResult,
    ProjectUnarchiveResult, ProjectsShowResult,
};

use super::helpers::{format_due, format_priority, truncate_id, truncate_str};

/// JSON output structure for a created project.
#[derive(Serialize)]
pub struct CreatedProjectOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<&'a str>,
    pub is_favorite: bool,
}

/// Formats a created project as JSON.
pub fn format_created_project(result: &ProjectAddResult) -> Result<String, serde_json::Error> {
    let output = CreatedProjectOutput {
        id: &result.id,
        name: &result.name,
        color: result.color.as_deref(),
        parent_id: result.parent_id.as_deref(),
        parent_name: result.parent_name.as_deref(),
        is_favorite: result.is_favorite,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for an edited project.
#[derive(Serialize)]
pub struct EditedProjectOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub updated_fields: &'a [String],
}

/// Formats an edited project as JSON.
pub fn format_edited_project(result: &ProjectEditResult) -> Result<String, serde_json::Error> {
    let output = EditedProjectOutput {
        id: &result.id,
        name: &result.name,
        updated_fields: &result.updated_fields,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for an archived project.
#[derive(Serialize)]
pub struct ArchivedProjectOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub status: &'static str,
}

/// Formats an archived project as JSON.
pub fn format_archived_project(
    result: &ProjectArchiveResult,
) -> Result<String, serde_json::Error> {
    let output = ArchivedProjectOutput {
        id: &result.id,
        name: &result.name,
        status: "archived",
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for an unarchived project.
#[derive(Serialize)]
pub struct UnarchivedProjectOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub status: &'static str,
}

/// Formats an unarchived project as JSON.
pub fn format_unarchived_project(
    result: &ProjectUnarchiveResult,
) -> Result<String, serde_json::Error> {
    let output = UnarchivedProjectOutput {
        id: &result.id,
        name: &result.name,
        status: "unarchived",
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for a deleted project.
#[derive(Serialize)]
pub struct DeletedProjectOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub status: &'static str,
}

/// Formats a deleted project as JSON.
pub fn format_deleted_project(result: &ProjectDeleteResult) -> Result<String, serde_json::Error> {
    let output = DeletedProjectOutput {
        id: &result.id,
        name: &result.name,
        status: "deleted",
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for projects list command.
#[derive(Serialize)]
pub struct ProjectsListOutput<'a> {
    pub projects: Vec<ProjectOutput<'a>>,
}

/// JSON output structure for a single project.
#[derive(Serialize)]
pub struct ProjectOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<&'a str>,
    pub is_favorite: bool,
    pub is_archived: bool,
    pub is_inbox: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_style: Option<&'a str>,
    pub task_count: usize,
}

/// Formats projects as JSON.
pub fn format_projects_json(projects: &[&Project]) -> Result<String, serde_json::Error> {
    let projects_output: Vec<ProjectOutput> = projects
        .iter()
        .map(|p| ProjectOutput {
            id: &p.id,
            name: &p.name,
            color: p.color.as_deref(),
            parent_id: p.parent_id.as_deref(),
            is_favorite: p.is_favorite,
            is_archived: p.is_archived,
            is_inbox: p.inbox_project,
            view_style: p.view_style.as_deref(),
            task_count: 0, // We don't have task count in this context
        })
        .collect();

    let output = ProjectsListOutput {
        projects: projects_output,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats projects as a table.
pub fn format_projects_table(
    projects: &[&Project],
    cache: &Cache,
    use_colors: bool,
    tree: bool,
) -> String {
    if projects.is_empty() {
        return "No projects found.\n".to_string();
    }

    let mut output = String::new();

    if tree {
        // Tree view: show hierarchy with indentation
        output.push_str(&format_projects_tree(projects, cache, use_colors));
    } else {
        // Flat view: simple table
        output.push_str(&format_projects_flat(projects, cache, use_colors));
    }

    output
}

/// Formats projects as a flat table.
fn format_projects_flat(projects: &[&Project], cache: &Cache, use_colors: bool) -> String {
    let mut output = String::new();

    // Header
    let header = format!(
        "{:<8} {:<4} {:<25} {:<6} {}",
        "ID", "Fav", "Name", "Tasks", "Color"
    );
    if use_colors {
        output.push_str(&format!("{}\n", header.dimmed()));
    } else {
        output.push_str(&header);
        output.push('\n');
    }

    // Count tasks per project
    let task_counts = count_tasks_per_project(cache);

    // Projects
    for project in projects {
        let id_prefix = truncate_id(&project.id);
        let fav = if project.is_favorite {
            if use_colors {
                "★".yellow().to_string()
            } else {
                "★".to_string()
            }
        } else {
            " ".to_string()
        };
        let name = format_project_name(project, use_colors);
        let task_count = task_counts.get(&project.id).copied().unwrap_or(0);
        let color = project.color.as_deref().unwrap_or("");

        let line = format!(
            "{:<8} {:<4} {:<25} {:<6} {}",
            id_prefix,
            fav,
            truncate_str(&name, 25),
            task_count,
            color
        );
        output.push_str(&line);
        output.push('\n');
    }

    output
}

/// Formats projects as a tree with indentation.
fn format_projects_tree(projects: &[&Project], cache: &Cache, use_colors: bool) -> String {
    let mut output = String::new();

    // Build parent-child relationships
    let mut children_map: HashMap<Option<&str>, Vec<&Project>> = HashMap::new();
    for project in projects {
        children_map
            .entry(project.parent_id.as_deref())
            .or_default()
            .push(project);
    }

    // Sort children by child_order
    for children in children_map.values_mut() {
        children.sort_by_key(|p| p.child_order);
    }

    // Count tasks per project
    let task_counts = count_tasks_per_project(cache);

    // Recursively print tree starting from root projects
    fn print_tree(
        output: &mut String,
        parent_id: Option<&str>,
        children_map: &HashMap<Option<&str>, Vec<&Project>>,
        task_counts: &HashMap<String, usize>,
        depth: usize,
        use_colors: bool,
    ) {
        if let Some(children) = children_map.get(&parent_id) {
            for project in children {
                let indent = "  ".repeat(depth);
                let prefix = if depth > 0 { "└─ " } else { "" };
                let fav = if project.is_favorite { "★ " } else { "" };
                let task_count = task_counts.get(&project.id).copied().unwrap_or(0);
                let id_prefix = truncate_id(&project.id);

                let name_display = if use_colors {
                    if project.inbox_project {
                        project.name.cyan().to_string()
                    } else if project.is_favorite {
                        format!("{}{}", fav.yellow(), project.name)
                    } else {
                        project.name.clone()
                    }
                } else {
                    format!("{}{}", fav, project.name)
                };

                let line = format!(
                    "{}{}{} ({}) [{}]",
                    indent, prefix, name_display, task_count, id_prefix
                );
                output.push_str(&line);
                output.push('\n');

                // Recursively print children
                print_tree(
                    output,
                    Some(&project.id),
                    children_map,
                    task_counts,
                    depth + 1,
                    use_colors,
                );
            }
        }
    }

    print_tree(&mut output, None, &children_map, &task_counts, 0, use_colors);

    output
}

/// Counts tasks per project.
fn count_tasks_per_project(cache: &Cache) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for item in &cache.items {
        if !item.is_deleted && !item.checked {
            *counts.entry(item.project_id.clone()).or_insert(0) += 1;
        }
    }
    counts
}

/// Formats a project name with special indicators.
fn format_project_name(project: &Project, use_colors: bool) -> String {
    let mut name = project.name.clone();

    if project.inbox_project {
        if use_colors {
            name = name.cyan().to_string();
        } else {
            name = format!("{} (Inbox)", name);
        }
    }

    if project.is_archived {
        if use_colors {
            name = name.strikethrough().dimmed().to_string();
        } else {
            name = format!("{} [archived]", name);
        }
    }

    name
}

// ============================================================================
// Project Details Output Formatting (projects show command)
// ============================================================================

/// JSON output structure for project details (projects show command).
#[derive(Serialize)]
pub struct ProjectDetailsOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<&'a str>,
    pub is_favorite: bool,
    pub is_archived: bool,
    pub is_inbox: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_style: Option<&'a str>,
    pub task_count: usize,
    pub section_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SectionOutput<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<ProjectTaskOutput<'a>>,
}

/// JSON output for a section in project details.
#[derive(Serialize)]
pub struct SectionOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub order: i32,
}

/// JSON output for a task in project details.
#[derive(Serialize)]
pub struct ProjectTaskOutput<'a> {
    pub id: &'a str,
    pub content: &'a str,
    pub priority: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_id: Option<&'a str>,
}

/// Formats project details as JSON (projects show command).
pub fn format_project_details_json(
    result: &ProjectsShowResult,
) -> Result<String, serde_json::Error> {
    let sections: Vec<SectionOutput> = result
        .sections
        .iter()
        .map(|s| SectionOutput {
            id: &s.id,
            name: &s.name,
            order: s.section_order,
        })
        .collect();

    let tasks: Vec<ProjectTaskOutput> = result
        .tasks
        .iter()
        .map(|t| ProjectTaskOutput {
            id: &t.id,
            content: &t.content,
            // Convert API priority (4=highest) to user priority (1=highest)
            priority: (5 - t.priority) as u8,
            due: t.due.as_ref().map(|d| d.date.as_str()),
            section_id: t.section_id.as_deref(),
        })
        .collect();

    let output = ProjectDetailsOutput {
        id: &result.project.id,
        name: &result.project.name,
        color: result.project.color.as_deref(),
        parent_id: result.project.parent_id.as_deref(),
        parent_name: result.parent_name.as_deref(),
        is_favorite: result.project.is_favorite,
        is_archived: result.project.is_archived,
        is_inbox: result.project.inbox_project,
        view_style: result.project.view_style.as_deref(),
        task_count: result.task_count,
        section_count: result.section_count,
        sections,
        tasks,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats project details as a human-readable table (projects show command).
pub fn format_project_details_table(result: &ProjectsShowResult, use_colors: bool) -> String {
    let mut output = String::new();

    // Project header
    let name_label = if use_colors {
        "Project:".bold().to_string()
    } else {
        "Project:".to_string()
    };
    output.push_str(&format!("{} {}\n", name_label, result.project.name));

    // ID
    output.push_str(&format!("ID: {}\n", result.project.id));

    // Parent project
    if let Some(ref parent_name) = result.parent_name {
        output.push_str(&format!("Parent: {}\n", parent_name));
    }

    // Color
    if let Some(ref color) = result.project.color {
        output.push_str(&format!("Color: {}\n", color));
    }

    // View style
    if let Some(ref view_style) = result.project.view_style {
        output.push_str(&format!("View style: {}\n", view_style));
    }

    // Favorite
    if result.project.is_favorite {
        let fav = if use_colors {
            "★ Yes".yellow().to_string()
        } else {
            "Yes".to_string()
        };
        output.push_str(&format!("Favorite: {}\n", fav));
    }

    // Inbox indicator
    if result.project.inbox_project {
        let inbox = if use_colors {
            "Yes".cyan().to_string()
        } else {
            "Yes".to_string()
        };
        output.push_str(&format!("Inbox: {}\n", inbox));
    }

    // Archived indicator
    if result.project.is_archived {
        let archived = if use_colors {
            "Yes".dimmed().to_string()
        } else {
            "Yes".to_string()
        };
        output.push_str(&format!("Archived: {}\n", archived));
    }

    // Task and section counts
    output.push_str(&format!("Tasks: {}\n", result.task_count));
    output.push_str(&format!("Sections: {}\n", result.section_count));

    // Sections list (if requested)
    if !result.sections.is_empty() {
        output.push_str(&format!("\nSections ({}):\n", result.sections.len()));
        let mut sorted_sections = result.sections.clone();
        sorted_sections.sort_by_key(|s| s.section_order);
        for section in &sorted_sections {
            let id_prefix = truncate_id(&section.id);
            output.push_str(&format!("  {} {}\n", id_prefix, section.name));
        }
    }

    // Tasks list (if requested)
    if !result.tasks.is_empty() {
        output.push_str(&format!("\nTasks ({}):\n", result.tasks.len()));
        for task in &result.tasks {
            let id_prefix = truncate_id(&task.id);
            let priority = format_priority(task.priority, use_colors);
            let due = format_due(task.due.as_ref().map(|d| &d.date), use_colors);
            let due_str = if due.is_empty() {
                String::new()
            } else {
                format!(" [{}]", due)
            };
            output.push_str(&format!(
                "  {} {} {}{}\n",
                id_prefix, priority, task.content, due_str
            ));
        }
    }

    output
}
