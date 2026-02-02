//! Filter evaluation against cached items.
//!
//! This module provides the [`FilterEvaluator`] for evaluating parsed filter expressions
//! against Todoist items from the cache.
//!
//! # Example
//!
//! ```
//! use todoist_cache_rs::filter::{FilterParser, FilterEvaluator, FilterContext};
//! use todoist_api_rs::sync::{Item, Project, Section, Label};
//!
//! // Parse a filter
//! let filter = FilterParser::parse("today & p1").unwrap();
//!
//! // Create evaluation context
//! let context = FilterContext::new(
//!     &[],       // projects
//!     &[],       // sections
//!     &[],       // labels
//! );
//!
//! // Create an item to test
//! let item = Item {
//!     id: "1".to_string(),
//!     project_id: "proj-1".to_string(),
//!     content: "Important task".to_string(),
//!     description: String::new(),
//!     priority: 4, // p1 in Todoist API (inverted)
//!     due: None,
//!     // ... other fields
//!     # user_id: None,
//!     # deadline: None,
//!     # parent_id: None,
//!     # child_order: 0,
//!     # section_id: None,
//!     # day_order: 0,
//!     # is_collapsed: false,
//!     # labels: vec![],
//!     # added_by_uid: None,
//!     # assigned_by_uid: None,
//!     # responsible_uid: None,
//!     # checked: false,
//!     # is_deleted: false,
//!     # added_at: None,
//!     # updated_at: None,
//!     # completed_at: None,
//!     # duration: None,
//! };
//!
//! // Evaluate the filter
//! let evaluator = FilterEvaluator::new(&filter, &context);
//! let matches = evaluator.matches(&item);
//! ```

use chrono::{Datelike, Local, NaiveDate};
use todoist_api_rs::sync::{Item, Label, Project, Section};

use super::ast::Filter;

/// Context for filter evaluation.
///
/// Contains reference data needed to resolve project/section/label names to IDs
/// and to build hierarchies for `##project` (project with subprojects) filters.
#[derive(Debug, Clone)]
pub struct FilterContext<'a> {
    projects: &'a [Project],
    sections: &'a [Section],
    labels: &'a [Label],
}

impl<'a> FilterContext<'a> {
    /// Creates a new filter context.
    ///
    /// # Arguments
    ///
    /// * `projects` - All projects from the cache
    /// * `sections` - All sections from the cache
    /// * `labels` - All labels from the cache
    pub fn new(projects: &'a [Project], sections: &'a [Section], labels: &'a [Label]) -> Self {
        Self {
            projects,
            sections,
            labels,
        }
    }

    /// Finds a project by name (case-insensitive).
    ///
    /// Only returns non-deleted projects.
    pub fn find_project_by_name(&self, name: &str) -> Option<&Project> {
        let name_lower = name.to_lowercase();
        self.projects
            .iter()
            .find(|p| !p.is_deleted && p.name.to_lowercase() == name_lower)
    }

    /// Gets all project IDs that match the given project name or are subprojects of it.
    /// Used for `##project` filters.
    pub fn get_project_ids_with_subprojects(&self, name: &str) -> Vec<&str> {
        let Some(root_project) = self.find_project_by_name(name) else {
            return vec![];
        };

        let mut ids = vec![root_project.id.as_str()];
        self.collect_subproject_ids(&root_project.id, &mut ids);
        ids
    }

    /// Recursively collects all subproject IDs for a given parent project.
    fn collect_subproject_ids<'b>(&'b self, parent_id: &str, ids: &mut Vec<&'b str>) {
        for project in self.projects.iter() {
            if project.parent_id.as_deref() == Some(parent_id) && !project.is_deleted {
                ids.push(&project.id);
                self.collect_subproject_ids(&project.id, ids);
            }
        }
    }

    /// Finds a section by name (case-insensitive).
    ///
    /// Only returns non-deleted sections.
    pub fn find_section_by_name(&self, name: &str) -> Option<&Section> {
        let name_lower = name.to_lowercase();
        self.sections
            .iter()
            .find(|s| !s.is_deleted && s.name.to_lowercase() == name_lower)
    }

    /// Checks if a label name exists (case-insensitive).
    ///
    /// Only considers non-deleted labels.
    pub fn label_exists(&self, name: &str) -> bool {
        let name_lower = name.to_lowercase();
        self.labels
            .iter()
            .any(|l| !l.is_deleted && l.name.to_lowercase() == name_lower)
    }
}

/// Evaluates a parsed filter against items.
///
/// The evaluator takes a reference to a parsed [`Filter`] and a [`FilterContext`],
/// then can test whether items match the filter criteria.
#[derive(Debug)]
pub struct FilterEvaluator<'a> {
    filter: &'a Filter,
    context: &'a FilterContext<'a>,
}

impl<'a> FilterEvaluator<'a> {
    /// Creates a new filter evaluator.
    ///
    /// # Arguments
    ///
    /// * `filter` - The parsed filter to evaluate
    /// * `context` - The context containing projects, sections, and labels
    pub fn new(filter: &'a Filter, context: &'a FilterContext<'a>) -> Self {
        Self { filter, context }
    }

    /// Returns true if the item matches the filter.
    pub fn matches(&self, item: &Item) -> bool {
        self.evaluate_filter(self.filter, item)
    }

    /// Filters a slice of items, returning only those that match.
    ///
    /// Pre-allocates the result vector with an estimated 10% match rate,
    /// which is typical for date-based and priority filters. The minimum
    /// capacity is 16 to handle small collections efficiently.
    pub fn filter_items<'b>(&self, items: &'b [Item]) -> Vec<&'b Item> {
        // Estimate 10% match rate as reasonable default for most filters.
        // Most filters (today, priority, project) match small subsets.
        let estimated_capacity = (items.len() / 10).max(16);
        let mut result = Vec::with_capacity(estimated_capacity);

        for item in items {
            if self.matches(item) {
                result.push(item);
            }
        }

        result
    }

    /// Evaluates a filter expression against an item.
    fn evaluate_filter(&self, filter: &Filter, item: &Item) -> bool {
        match filter {
            // Date filters
            Filter::Today => self.is_due_today(item),
            Filter::Tomorrow => self.is_due_tomorrow(item),
            Filter::Overdue => self.is_overdue(item),
            Filter::NoDate => self.has_no_date(item),
            Filter::Next7Days => self.is_due_within_7_days(item),
            Filter::SpecificDate { month, day } => self.is_due_on_specific_date(item, *month, *day),

            // Priority filters
            // Note: Todoist API uses inverted priority (4 = highest, 1 = lowest)
            // But the user-facing values are p1 = highest, p4 = lowest
            Filter::Priority1 => item.priority == 4,
            Filter::Priority2 => item.priority == 3,
            Filter::Priority3 => item.priority == 2,
            Filter::Priority4 => item.priority == 1,

            // Label filters
            Filter::Label(name) => self.has_label(item, name),
            Filter::NoLabels => self.has_no_labels(item),

            // Project filters
            Filter::Project(name) => self.in_project(item, name),
            Filter::ProjectWithSubprojects(name) => self.in_project_or_subproject(item, name),

            // Section filter
            Filter::Section(name) => self.in_section(item, name),

            // Boolean operators
            Filter::And(left, right) => {
                self.evaluate_filter(left, item) && self.evaluate_filter(right, item)
            }
            Filter::Or(left, right) => {
                self.evaluate_filter(left, item) || self.evaluate_filter(right, item)
            }
            Filter::Not(inner) => !self.evaluate_filter(inner, item),
        }
    }

    /// Checks if the item is due today.
    fn is_due_today(&self, item: &Item) -> bool {
        let Some(due) = &item.due else {
            return false;
        };

        let today = Local::now().date_naive();
        self.parse_due_date(&due.date)
            .is_some_and(|due_date| due_date == today)
    }

    /// Checks if the item is due tomorrow.
    fn is_due_tomorrow(&self, item: &Item) -> bool {
        let Some(due) = &item.due else {
            return false;
        };

        let tomorrow = Local::now().date_naive() + chrono::Duration::days(1);
        self.parse_due_date(&due.date)
            .is_some_and(|due_date| due_date == tomorrow)
    }

    /// Checks if the item is overdue (due date is in the past).
    fn is_overdue(&self, item: &Item) -> bool {
        // Completed items are not overdue
        if item.checked {
            return false;
        }

        let Some(due) = &item.due else {
            return false;
        };

        let today = Local::now().date_naive();
        self.parse_due_date(&due.date)
            .is_some_and(|due_date| due_date < today)
    }

    /// Checks if the item has no due date.
    fn has_no_date(&self, item: &Item) -> bool {
        item.due.is_none()
    }

    /// Checks if the item is due within the next 7 days (including today).
    fn is_due_within_7_days(&self, item: &Item) -> bool {
        let Some(due) = &item.due else {
            return false;
        };

        let today = Local::now().date_naive();
        let end_date = today + chrono::Duration::days(7);

        self.parse_due_date(&due.date)
            .is_some_and(|due_date| due_date >= today && due_date < end_date)
    }

    /// Checks if the item is due on a specific month and day.
    /// The year is inferred: if the date has passed this year, it matches next year.
    fn is_due_on_specific_date(&self, item: &Item, month: u32, day: u32) -> bool {
        let Some(due) = &item.due else {
            return false;
        };

        self.parse_due_date(&due.date)
            .is_some_and(|due_date| due_date.month() == month && due_date.day() == day)
    }

    /// Parses a date string in YYYY-MM-DD format.
    fn parse_due_date(&self, date_str: &str) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()
    }

    /// Checks if the item has the specified label (case-insensitive).
    fn has_label(&self, item: &Item, label_name: &str) -> bool {
        let label_lower = label_name.to_lowercase();
        item.labels.iter().any(|l| l.to_lowercase() == label_lower)
    }

    /// Checks if the item has no labels.
    fn has_no_labels(&self, item: &Item) -> bool {
        item.labels.is_empty()
    }

    /// Checks if the item is in the specified project (case-insensitive).
    fn in_project(&self, item: &Item, project_name: &str) -> bool {
        self.context
            .find_project_by_name(project_name)
            .is_some_and(|project| project.id == item.project_id)
    }

    /// Checks if the item is in the specified project or any of its subprojects.
    fn in_project_or_subproject(&self, item: &Item, project_name: &str) -> bool {
        let project_ids = self.context.get_project_ids_with_subprojects(project_name);
        project_ids.contains(&item.project_id.as_str())
    }

    /// Checks if the item is in the specified section (case-insensitive).
    fn in_section(&self, item: &Item, section_name: &str) -> bool {
        let Some(section_id) = &item.section_id else {
            return false;
        };

        self.context
            .find_section_by_name(section_name)
            .is_some_and(|section| &section.id == section_id)
    }
}

#[cfg(test)]
#[path = "evaluator_tests.rs"]
mod tests;
