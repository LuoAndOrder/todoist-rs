//! Filter output formatting.

use owo_colors::OwoColorize;
use serde::Serialize;
use todoist_api::sync::Filter;

use crate::commands::filters::{
    FilterAddResult, FilterDeleteResult, FilterEditResult, FilterShowResult,
};

use super::helpers::{truncate_id, truncate_str};

/// JSON output structure for filters list command.
#[derive(Serialize)]
pub struct FiltersListOutput<'a> {
    pub filters: Vec<FilterOutput<'a>>,
}

/// JSON output structure for a single filter.
#[derive(Serialize)]
pub struct FilterOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub query: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    pub is_favorite: bool,
    pub item_order: i32,
}

/// Formats filters as JSON.
pub fn format_filters_json(filters: &[&Filter]) -> Result<String, serde_json::Error> {
    let filters_output: Vec<FilterOutput> = filters
        .iter()
        .map(|f| FilterOutput {
            id: &f.id,
            name: &f.name,
            query: &f.query,
            color: f.color.as_deref(),
            is_favorite: f.is_favorite,
            item_order: f.item_order,
        })
        .collect();

    let output = FiltersListOutput {
        filters: filters_output,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats filters as a table.
pub fn format_filters_table(filters: &[&Filter], use_colors: bool) -> String {
    if filters.is_empty() {
        return "No filters found.\n".to_string();
    }

    let mut output = String::new();

    // Header
    let header = format!("{:<8} {:<4} {:<25} {}", "ID", "Fav", "Name", "Query");
    if use_colors {
        output.push_str(&format!("{}\n", header.dimmed()));
    } else {
        output.push_str(&header);
        output.push('\n');
    }

    // Filters
    for filter in filters {
        let id_prefix = truncate_id(&filter.id);
        let fav = if filter.is_favorite {
            if use_colors {
                "★".yellow().to_string()
            } else {
                "★".to_string()
            }
        } else {
            " ".to_string()
        };
        let name = truncate_str(&filter.name, 25);
        let query = truncate_str(&filter.query, 40);

        let line = format!("{:<8} {:<4} {:<25} {}", id_prefix, fav, name, query);
        output.push_str(&line);
        output.push('\n');
    }

    output
}

/// JSON output structure for a created filter.
#[derive(Serialize)]
pub struct CreatedFilterOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub query: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    pub is_favorite: bool,
}

/// Formats a created filter as JSON.
pub fn format_created_filter(result: &FilterAddResult) -> Result<String, serde_json::Error> {
    let output = CreatedFilterOutput {
        id: &result.id,
        name: &result.name,
        query: &result.query,
        color: result.color.as_deref(),
        is_favorite: result.is_favorite,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for filter details (filters show command).
#[derive(Serialize)]
pub struct FilterDetailsOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub query: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    pub is_favorite: bool,
    pub item_order: i32,
}

/// Formats filter details as JSON (filters show command).
pub fn format_filter_details_json(result: &FilterShowResult) -> Result<String, serde_json::Error> {
    let output = FilterDetailsOutput {
        id: &result.filter.id,
        name: &result.filter.name,
        query: &result.filter.query,
        color: result.filter.color.as_deref(),
        is_favorite: result.filter.is_favorite,
        item_order: result.filter.item_order,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats filter details as a human-readable table (filters show command).
pub fn format_filter_details_table(result: &FilterShowResult, use_colors: bool) -> String {
    let mut output = String::new();

    // Filter header
    let name_label = if use_colors {
        "Filter:".bold().to_string()
    } else {
        "Filter:".to_string()
    };
    output.push_str(&format!("{} {}\n", name_label, result.filter.name));

    // ID
    output.push_str(&format!("ID: {}\n", result.filter.id));

    // Query
    output.push_str(&format!("Query: {}\n", result.filter.query));

    // Color
    if let Some(ref color) = result.filter.color {
        output.push_str(&format!("Color: {}\n", color));
    }

    // Favorite
    if result.filter.is_favorite {
        let fav = if use_colors {
            "★ Yes".yellow().to_string()
        } else {
            "Yes".to_string()
        };
        output.push_str(&format!("Favorite: {}\n", fav));
    }

    // Order
    output.push_str(&format!("Order: {}\n", result.filter.item_order));

    output
}

/// JSON output structure for an edited filter.
#[derive(Serialize)]
pub struct EditedFilterOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub updated_fields: &'a [String],
}

/// Formats an edited filter as JSON.
pub fn format_edited_filter(result: &FilterEditResult) -> Result<String, serde_json::Error> {
    let output = EditedFilterOutput {
        id: &result.id,
        name: &result.name,
        updated_fields: &result.updated_fields,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for a deleted filter.
#[derive(Serialize)]
pub struct DeletedFilterOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub status: &'static str,
}

/// Formats a deleted filter as JSON.
pub fn format_deleted_filter(result: &FilterDeleteResult) -> Result<String, serde_json::Error> {
    let output = DeletedFilterOutput {
        id: &result.id,
        name: &result.name,
        status: "deleted",
    };

    serde_json::to_string_pretty(&output)
}
