//! Section output formatting.

use owo_colors::OwoColorize;
use serde::Serialize;
use todoist_api_rs::sync::Section;
use todoist_cache_rs::Cache;

use crate::commands::sections::{SectionAddResult, SectionDeleteResult, SectionEditResult};

use super::helpers::{truncate_id, truncate_str};

/// JSON output structure for sections list command.
#[derive(Serialize)]
pub struct SectionsListOutput<'a> {
    pub sections: Vec<SectionListOutput<'a>>,
}

/// JSON output structure for a single section in list.
#[derive(Serialize)]
pub struct SectionListOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub project_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<&'a str>,
    pub section_order: i32,
    pub is_archived: bool,
}

/// Formats sections as JSON.
pub fn format_sections_json(
    sections: &[&Section],
    cache: &Cache,
) -> Result<String, serde_json::Error> {
    let sections_output: Vec<SectionListOutput> = sections
        .iter()
        .map(|s| {
            let project_name = cache
                .projects
                .iter()
                .find(|p| p.id == s.project_id)
                .map(|p| p.name.as_str());

            SectionListOutput {
                id: &s.id,
                name: &s.name,
                project_id: &s.project_id,
                project_name,
                section_order: s.section_order,
                is_archived: s.is_archived,
            }
        })
        .collect();

    let output = SectionsListOutput {
        sections: sections_output,
    };

    serde_json::to_string_pretty(&output)
}

/// Formats sections as a table.
pub fn format_sections_table(sections: &[&Section], cache: &Cache, use_colors: bool) -> String {
    if sections.is_empty() {
        return "No sections found.\n".to_string();
    }

    let mut output = String::new();

    // Header
    let header = format!("{:<8} {:<25} {:<20}", "ID", "Name", "Project");
    if use_colors {
        output.push_str(&format!("{}\n", header.dimmed()));
    } else {
        output.push_str(&header);
        output.push('\n');
    }

    // Sections
    for section in sections {
        let id_prefix = truncate_id(&section.id);
        let project_name = cache
            .projects
            .iter()
            .find(|p| p.id == section.project_id)
            .map(|p| truncate_str(&p.name, 20))
            .unwrap_or_default();

        let name = if section.is_archived {
            if use_colors {
                format!("{} [archived]", section.name)
                    .dimmed()
                    .to_string()
            } else {
                format!("{} [archived]", section.name)
            }
        } else {
            section.name.clone()
        };

        let line = format!(
            "{:<8} {:<25} {:<20}",
            id_prefix,
            truncate_str(&name, 25),
            project_name
        );
        output.push_str(&line);
        output.push('\n');
    }

    output
}

/// JSON output structure for a created section.
#[derive(Serialize)]
pub struct CreatedSectionOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub project_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<&'a str>,
}

/// Formats a created section as JSON.
pub fn format_created_section(result: &SectionAddResult) -> Result<String, serde_json::Error> {
    let output = CreatedSectionOutput {
        id: &result.id,
        name: &result.name,
        project_id: &result.project_id,
        project_name: result.project_name.as_deref(),
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for an edited section.
#[derive(Serialize)]
pub struct EditedSectionOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub updated_fields: &'a [String],
}

/// Formats an edited section as JSON.
pub fn format_edited_section(result: &SectionEditResult) -> Result<String, serde_json::Error> {
    let output = EditedSectionOutput {
        id: &result.id,
        name: &result.name,
        updated_fields: &result.updated_fields,
    };

    serde_json::to_string_pretty(&output)
}

/// JSON output structure for a deleted section.
#[derive(Serialize)]
pub struct DeletedSectionOutput<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub status: &'static str,
}

/// Formats a deleted section as JSON.
pub fn format_deleted_section(result: &SectionDeleteResult) -> Result<String, serde_json::Error> {
    let output = DeletedSectionOutput {
        id: &result.id,
        name: &result.name,
        status: "deleted",
    };

    serde_json::to_string_pretty(&output)
}
