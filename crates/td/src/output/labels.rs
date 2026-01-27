//! Label output formatting.

use owo_colors::OwoColorize;
use serde::Serialize;
use todoist_api_rs::sync::Label;

use crate::commands::labels::{LabelAddResult, LabelDeleteResult, LabelEditResult};

use super::helpers::{truncate_id, truncate_str};

/// JSON output structure for labels list command.
#[derive(Serialize)]
pub struct LabelsListOutput<'a> {
    pub labels: Vec<LabelOutput<'a>>,
}

/// JSON output structure for a single label.
#[derive(Serialize)]
pub struct LabelOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    pub is_favorite: bool,
    pub item_order: i32,
}

/// Formats labels as JSON.
pub fn format_labels_json(labels: &[&Label]) -> Result<String, serde_json::Error> {
    let labels_output: Vec<LabelOutput> = labels
        .iter()
        .map(|l| LabelOutput {
            id: &l.id,
            name: &l.name,
            color: l.color.as_deref(),
            is_favorite: l.is_favorite,
            item_order: l.item_order,
        })
        .collect();

    let output = LabelsListOutput {
        labels: labels_output,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats labels as a table.
pub fn format_labels_table(labels: &[&Label], use_colors: bool) -> String {
    if labels.is_empty() {
        return "No labels found.\n".to_string();
    }

    let mut output = String::new();

    // Header
    let header = format!("{:<8} {:<4} {:<20} {}", "ID", "Fav", "Name", "Color");
    if use_colors {
        output.push_str(&format!("{}\n", header.dimmed()));
    } else {
        output.push_str(&header);
        output.push('\n');
    }

    // Labels
    for label in labels {
        let id_prefix = truncate_id(&label.id);
        let fav = if label.is_favorite {
            if use_colors {
                "★".yellow().to_string()
            } else {
                "★".to_string()
            }
        } else {
            " ".to_string()
        };
        let name = format!("@{}", label.name);
        let color = label.color.as_deref().unwrap_or("");

        let line = format!(
            "{:<8} {:<4} {:<20} {}",
            id_prefix,
            fav,
            truncate_str(&name, 20),
            color
        );
        output.push_str(&line);
        output.push('\n');
    }

    output
}

/// JSON output structure for a created label.
#[derive(Serialize)]
pub struct CreatedLabelOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    pub is_favorite: bool,
}

/// Formats a created label as JSON.
pub fn format_created_label(result: &LabelAddResult) -> Result<String, serde_json::Error> {
    let output = CreatedLabelOutput {
        id: &result.id,
        name: &result.name,
        color: result.color.as_deref(),
        is_favorite: result.is_favorite,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for an edited label.
#[derive(Serialize)]
pub struct EditedLabelOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub updated_fields: &'a [String],
}

/// Formats an edited label as JSON.
pub fn format_edited_label(result: &LabelEditResult) -> Result<String, serde_json::Error> {
    let output = EditedLabelOutput {
        id: &result.id,
        name: &result.name,
        updated_fields: &result.updated_fields,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for a deleted label.
#[derive(Serialize)]
pub struct DeletedLabelOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub status: &'static str,
}

/// Formats a deleted label as JSON.
pub fn format_deleted_label(result: &LabelDeleteResult) -> Result<String, serde_json::Error> {
    let output = DeletedLabelOutput {
        id: &result.id,
        name: &result.name,
        status: "deleted",
    };

    serde_json::to_string_pretty(&output)
}
