//! Filter evaluation against cached items.
//!
//! This module provides the [`FilterEvaluator`] for evaluating parsed filter expressions
//! against Todoist items from the cache.
//!
//! # Example
//!
//! ```
//! use todoist_cache::filter::{FilterParser, FilterEvaluator, FilterContext};
//! use todoist_api::sync::{Item, Project, Section, Label};
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
use todoist_api::sync::{Item, Label, Project, Section};

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
    pub fn filter_items<'b>(&self, items: &'b [Item]) -> Vec<&'b Item> {
        items.iter().filter(|item| self.matches(item)).collect()
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

            // Label filter
            Filter::Label(name) => self.has_label(item, name),

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

        self.parse_due_date(&due.date).is_some_and(|due_date| {
            due_date.month() == month && due_date.day() == day
        })
    }

    /// Parses a date string in YYYY-MM-DD format.
    fn parse_due_date(&self, date_str: &str) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()
    }

    /// Checks if the item has the specified label (case-insensitive).
    fn has_label(&self, item: &Item, label_name: &str) -> bool {
        let label_lower = label_name.to_lowercase();
        item.labels
            .iter()
            .any(|l| l.to_lowercase() == label_lower)
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
mod tests {
    use super::*;
    use chrono::Local;
    use todoist_api::sync::Due;

    // ==================== Test Helpers ====================

    fn make_item(id: &str, content: &str) -> Item {
        Item {
            id: id.to_string(),
            user_id: None,
            project_id: "proj-1".to_string(),
            content: content.to_string(),
            description: String::new(),
            priority: 1,
            due: None,
            deadline: None,
            parent_id: None,
            child_order: 0,
            section_id: None,
            day_order: 0,
            is_collapsed: false,
            labels: vec![],
            added_by_uid: None,
            assigned_by_uid: None,
            responsible_uid: None,
            checked: false,
            is_deleted: false,
            added_at: None,
            updated_at: None,
            completed_at: None,
            duration: None,
        }
    }

    fn make_due(date: &str) -> Due {
        Due {
            date: date.to_string(),
            datetime: None,
            string: None,
            timezone: None,
            is_recurring: false,
            lang: None,
        }
    }

    fn make_project(id: &str, name: &str, parent_id: Option<&str>) -> Project {
        Project {
            id: id.to_string(),
            name: name.to_string(),
            color: None,
            parent_id: parent_id.map(|s| s.to_string()),
            child_order: 0,
            is_collapsed: false,
            shared: false,
            can_assign_tasks: false,
            is_deleted: false,
            is_archived: false,
            is_favorite: false,
            view_style: None,
            inbox_project: false,
            folder_id: None,
            created_at: None,
            updated_at: None,
        }
    }

    fn make_section(id: &str, name: &str, project_id: &str) -> Section {
        Section {
            id: id.to_string(),
            name: name.to_string(),
            project_id: project_id.to_string(),
            section_order: 0,
            is_collapsed: false,
            is_deleted: false,
            is_archived: false,
            archived_at: None,
            added_at: None,
            updated_at: None,
        }
    }

    fn make_label(id: &str, name: &str) -> Label {
        Label {
            id: id.to_string(),
            name: name.to_string(),
            color: None,
            item_order: 0,
            is_deleted: false,
            is_favorite: false,
        }
    }

    fn today_str() -> String {
        Local::now().format("%Y-%m-%d").to_string()
    }

    fn tomorrow_str() -> String {
        (Local::now() + chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string()
    }

    fn yesterday_str() -> String {
        (Local::now() - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string()
    }

    fn days_from_today_str(days: i64) -> String {
        (Local::now() + chrono::Duration::days(days))
            .format("%Y-%m-%d")
            .to_string()
    }

    // ==================== Date Filter Tests ====================

    #[test]
    fn test_filter_today_matches() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Today;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&today_str()));

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_today_no_match_tomorrow() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Today;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&tomorrow_str()));

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_today_no_match_no_due() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Today;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let item = make_item("1", "Task");
        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_tomorrow_matches() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Tomorrow;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&tomorrow_str()));

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_tomorrow_no_match_today() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Tomorrow;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&today_str()));

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_overdue_matches() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Overdue;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&yesterday_str()));

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_overdue_no_match_today() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Overdue;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&today_str()));

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_overdue_no_match_completed() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Overdue;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&yesterday_str()));
        item.checked = true;

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_overdue_no_match_no_due() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Overdue;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let item = make_item("1", "Task");
        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_no_date_matches() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::NoDate;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let item = make_item("1", "Task");
        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_no_date_no_match_with_due() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::NoDate;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&today_str()));

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_7_days_matches_today() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Next7Days;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&today_str()));

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_7_days_matches_in_5_days() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Next7Days;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&days_from_today_str(5)));

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_7_days_matches_in_6_days() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Next7Days;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&days_from_today_str(6)));

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_7_days_no_match_in_7_days() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Next7Days;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&days_from_today_str(7)));

        // 7 days out is NOT included (it's day 8 in human terms)
        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_7_days_no_match_in_10_days() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Next7Days;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&days_from_today_str(10)));

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_7_days_no_match_overdue() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Next7Days;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&yesterday_str()));

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_7_days_no_match_no_due() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Next7Days;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let item = make_item("1", "Task");
        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_specific_date_matches() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::SpecificDate { month: 1, day: 15 };
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due("2025-01-15"));

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_specific_date_matches_any_year() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::SpecificDate { month: 12, day: 25 };
        let evaluator = FilterEvaluator::new(&filter, &context);

        // Matches regardless of year
        let mut item1 = make_item("1", "Task");
        item1.due = Some(make_due("2024-12-25"));
        assert!(evaluator.matches(&item1));

        let mut item2 = make_item("2", "Task");
        item2.due = Some(make_due("2025-12-25"));
        assert!(evaluator.matches(&item2));

        let mut item3 = make_item("3", "Task");
        item3.due = Some(make_due("2026-12-25"));
        assert!(evaluator.matches(&item3));
    }

    #[test]
    fn test_filter_specific_date_no_match_different_date() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::SpecificDate { month: 1, day: 15 };
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due("2025-01-16")); // Different day

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_specific_date_no_match_different_month() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::SpecificDate { month: 1, day: 15 };
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due("2025-02-15")); // Different month

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_specific_date_no_match_no_due() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::SpecificDate { month: 1, day: 15 };
        let evaluator = FilterEvaluator::new(&filter, &context);

        let item = make_item("1", "Task");
        assert!(!evaluator.matches(&item));
    }

    // ==================== Priority Filter Tests ====================

    #[test]
    fn test_filter_priority1_matches() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Priority1;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.priority = 4; // p1 in API

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_priority1_no_match_p2() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Priority1;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.priority = 3; // p2 in API

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_priority2_matches() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Priority2;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.priority = 3; // p2 in API

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_priority3_matches() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Priority3;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.priority = 2; // p3 in API

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_priority4_matches() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Priority4;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.priority = 1; // p4 in API (default)

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_priority_all_distinct() {
        let context = FilterContext::new(&[], &[], &[]);

        let mut item_p1 = make_item("1", "P1");
        item_p1.priority = 4;

        let mut item_p2 = make_item("2", "P2");
        item_p2.priority = 3;

        let mut item_p3 = make_item("3", "P3");
        item_p3.priority = 2;

        let mut item_p4 = make_item("4", "P4");
        item_p4.priority = 1;

        let eval_p1 = FilterEvaluator::new(&Filter::Priority1, &context);
        let eval_p2 = FilterEvaluator::new(&Filter::Priority2, &context);
        let eval_p3 = FilterEvaluator::new(&Filter::Priority3, &context);
        let eval_p4 = FilterEvaluator::new(&Filter::Priority4, &context);

        // Each filter should match only its priority
        assert!(eval_p1.matches(&item_p1));
        assert!(!eval_p1.matches(&item_p2));
        assert!(!eval_p1.matches(&item_p3));
        assert!(!eval_p1.matches(&item_p4));

        assert!(!eval_p2.matches(&item_p1));
        assert!(eval_p2.matches(&item_p2));
        assert!(!eval_p2.matches(&item_p3));
        assert!(!eval_p2.matches(&item_p4));

        assert!(!eval_p3.matches(&item_p1));
        assert!(!eval_p3.matches(&item_p2));
        assert!(eval_p3.matches(&item_p3));
        assert!(!eval_p3.matches(&item_p4));

        assert!(!eval_p4.matches(&item_p1));
        assert!(!eval_p4.matches(&item_p2));
        assert!(!eval_p4.matches(&item_p3));
        assert!(eval_p4.matches(&item_p4));
    }

    // ==================== Label Filter Tests ====================

    #[test]
    fn test_filter_label_matches() {
        let labels = vec![make_label("l1", "urgent")];
        let context = FilterContext::new(&[], &[], &labels);
        let filter = Filter::Label("urgent".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.labels = vec!["urgent".to_string()];

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_label_case_insensitive() {
        let labels = vec![make_label("l1", "Urgent")];
        let context = FilterContext::new(&[], &[], &labels);
        let filter = Filter::Label("URGENT".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.labels = vec!["urgent".to_string()];

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_label_no_match() {
        let labels = vec![make_label("l1", "urgent")];
        let context = FilterContext::new(&[], &[], &labels);
        let filter = Filter::Label("important".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.labels = vec!["urgent".to_string()];

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_label_no_match_no_labels() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Label("urgent".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let item = make_item("1", "Task");
        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_label_multiple_labels() {
        let labels = vec![
            make_label("l1", "urgent"),
            make_label("l2", "work"),
            make_label("l3", "personal"),
        ];
        let context = FilterContext::new(&[], &[], &labels);
        let filter = Filter::Label("work".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.labels = vec!["urgent".to_string(), "work".to_string()];

        assert!(evaluator.matches(&item));
    }

    // ==================== Project Filter Tests ====================

    #[test]
    fn test_filter_project_matches() {
        let projects = vec![make_project("proj-1", "Work", None)];
        let context = FilterContext::new(&projects, &[], &[]);
        let filter = Filter::Project("Work".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.project_id = "proj-1".to_string();

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_project_case_insensitive() {
        let projects = vec![make_project("proj-1", "Work", None)];
        let context = FilterContext::new(&projects, &[], &[]);
        let filter = Filter::Project("WORK".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.project_id = "proj-1".to_string();

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_project_no_match() {
        let projects = vec![
            make_project("proj-1", "Work", None),
            make_project("proj-2", "Personal", None),
        ];
        let context = FilterContext::new(&projects, &[], &[]);
        let filter = Filter::Project("Work".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.project_id = "proj-2".to_string();

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_project_not_found() {
        let projects = vec![make_project("proj-1", "Work", None)];
        let context = FilterContext::new(&projects, &[], &[]);
        let filter = Filter::Project("NonExistent".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.project_id = "proj-1".to_string();

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_project_with_subprojects_matches_parent() {
        let projects = vec![
            make_project("proj-1", "Work", None),
            make_project("proj-2", "Work/Meetings", Some("proj-1")),
        ];
        let context = FilterContext::new(&projects, &[], &[]);
        let filter = Filter::ProjectWithSubprojects("Work".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.project_id = "proj-1".to_string();

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_project_with_subprojects_matches_child() {
        let projects = vec![
            make_project("proj-1", "Work", None),
            make_project("proj-2", "Meetings", Some("proj-1")),
        ];
        let context = FilterContext::new(&projects, &[], &[]);
        let filter = Filter::ProjectWithSubprojects("Work".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.project_id = "proj-2".to_string();

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_project_with_subprojects_matches_grandchild() {
        let projects = vec![
            make_project("proj-1", "Work", None),
            make_project("proj-2", "Meetings", Some("proj-1")),
            make_project("proj-3", "Weekly", Some("proj-2")),
        ];
        let context = FilterContext::new(&projects, &[], &[]);
        let filter = Filter::ProjectWithSubprojects("Work".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.project_id = "proj-3".to_string();

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_project_with_subprojects_no_match_other_project() {
        let projects = vec![
            make_project("proj-1", "Work", None),
            make_project("proj-2", "Personal", None),
        ];
        let context = FilterContext::new(&projects, &[], &[]);
        let filter = Filter::ProjectWithSubprojects("Work".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.project_id = "proj-2".to_string();

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_project_exact_no_match_subproject() {
        // Regular #project should NOT match subprojects
        let projects = vec![
            make_project("proj-1", "Work", None),
            make_project("proj-2", "Meetings", Some("proj-1")),
        ];
        let context = FilterContext::new(&projects, &[], &[]);
        let filter = Filter::Project("Work".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.project_id = "proj-2".to_string();

        assert!(!evaluator.matches(&item));
    }

    // ==================== Section Filter Tests ====================

    #[test]
    fn test_filter_section_matches() {
        let sections = vec![make_section("sec-1", "Inbox", "proj-1")];
        let context = FilterContext::new(&[], &sections, &[]);
        let filter = Filter::Section("Inbox".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.section_id = Some("sec-1".to_string());

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_section_case_insensitive() {
        let sections = vec![make_section("sec-1", "Inbox", "proj-1")];
        let context = FilterContext::new(&[], &sections, &[]);
        let filter = Filter::Section("INBOX".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.section_id = Some("sec-1".to_string());

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_section_no_match() {
        let sections = vec![
            make_section("sec-1", "Inbox", "proj-1"),
            make_section("sec-2", "Archive", "proj-1"),
        ];
        let context = FilterContext::new(&[], &sections, &[]);
        let filter = Filter::Section("Inbox".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.section_id = Some("sec-2".to_string());

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_section_no_match_no_section() {
        let sections = vec![make_section("sec-1", "Inbox", "proj-1")];
        let context = FilterContext::new(&[], &sections, &[]);
        let filter = Filter::Section("Inbox".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let item = make_item("1", "Task");
        assert!(!evaluator.matches(&item));
    }

    // ==================== Boolean Operator Tests ====================

    #[test]
    fn test_filter_and_both_true() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::and(Filter::Today, Filter::Priority1);
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&today_str()));
        item.priority = 4;

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_and_one_false() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::and(Filter::Today, Filter::Priority1);
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&today_str()));
        item.priority = 1; // p4

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_and_both_false() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::and(Filter::Today, Filter::Priority1);
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&tomorrow_str()));
        item.priority = 1;

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_or_both_true() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::or(Filter::Today, Filter::Overdue);
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&today_str()));

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_or_one_true() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::or(Filter::Today, Filter::Overdue);
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&yesterday_str()));

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_or_both_false() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::or(Filter::Today, Filter::Overdue);
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&tomorrow_str()));

        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_not_inverts_true() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::negate(Filter::NoDate);
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.due = Some(make_due(&today_str()));

        assert!(evaluator.matches(&item));
    }

    #[test]
    fn test_filter_not_inverts_false() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::negate(Filter::NoDate);
        let evaluator = FilterEvaluator::new(&filter, &context);

        let item = make_item("1", "Task");
        assert!(!evaluator.matches(&item));
    }

    // ==================== Complex Expression Tests ====================

    #[test]
    fn test_filter_complex_today_or_overdue_and_p1() {
        // (today | overdue) & p1
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::and(
            Filter::or(Filter::Today, Filter::Overdue),
            Filter::Priority1,
        );
        let evaluator = FilterEvaluator::new(&filter, &context);

        // Today + p1 = match
        let mut item1 = make_item("1", "Task 1");
        item1.due = Some(make_due(&today_str()));
        item1.priority = 4;
        assert!(evaluator.matches(&item1));

        // Overdue + p1 = match
        let mut item2 = make_item("2", "Task 2");
        item2.due = Some(make_due(&yesterday_str()));
        item2.priority = 4;
        assert!(evaluator.matches(&item2));

        // Today + p4 = no match
        let mut item3 = make_item("3", "Task 3");
        item3.due = Some(make_due(&today_str()));
        item3.priority = 1;
        assert!(!evaluator.matches(&item3));

        // Tomorrow + p1 = no match
        let mut item4 = make_item("4", "Task 4");
        item4.due = Some(make_due(&tomorrow_str()));
        item4.priority = 4;
        assert!(!evaluator.matches(&item4));
    }

    #[test]
    fn test_filter_complex_with_labels_and_project() {
        // p1 & @urgent & #Work
        let projects = vec![make_project("proj-1", "Work", None)];
        let labels = vec![make_label("l1", "urgent")];
        let context = FilterContext::new(&projects, &[], &labels);

        let filter = Filter::and(
            Filter::and(Filter::Priority1, Filter::Label("urgent".to_string())),
            Filter::Project("Work".to_string()),
        );
        let evaluator = FilterEvaluator::new(&filter, &context);

        // All conditions met
        let mut item1 = make_item("1", "Task 1");
        item1.priority = 4;
        item1.labels = vec!["urgent".to_string()];
        item1.project_id = "proj-1".to_string();
        assert!(evaluator.matches(&item1));

        // Missing label
        let mut item2 = make_item("2", "Task 2");
        item2.priority = 4;
        item2.project_id = "proj-1".to_string();
        assert!(!evaluator.matches(&item2));

        // Wrong project
        let mut item3 = make_item("3", "Task 3");
        item3.priority = 4;
        item3.labels = vec!["urgent".to_string()];
        item3.project_id = "proj-other".to_string();
        assert!(!evaluator.matches(&item3));
    }

    #[test]
    fn test_filter_not_with_complex_expression() {
        // !(today & p1) - items that are NOT both today and p1
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::negate(Filter::and(Filter::Today, Filter::Priority1));
        let evaluator = FilterEvaluator::new(&filter, &context);

        // Today + p1 = no match (because NOT)
        let mut item1 = make_item("1", "Task 1");
        item1.due = Some(make_due(&today_str()));
        item1.priority = 4;
        assert!(!evaluator.matches(&item1));

        // Today + p4 = match
        let mut item2 = make_item("2", "Task 2");
        item2.due = Some(make_due(&today_str()));
        item2.priority = 1;
        assert!(evaluator.matches(&item2));

        // Tomorrow + p1 = match
        let mut item3 = make_item("3", "Task 3");
        item3.due = Some(make_due(&tomorrow_str()));
        item3.priority = 4;
        assert!(evaluator.matches(&item3));
    }

    // ==================== Filter Items Tests ====================

    #[test]
    fn test_filter_items_returns_matching() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Priority1;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item1 = make_item("1", "P1 Task");
        item1.priority = 4;

        let mut item2 = make_item("2", "P2 Task");
        item2.priority = 3;

        let mut item3 = make_item("3", "Another P1 Task");
        item3.priority = 4;

        let items = vec![item1, item2, item3];
        let results = evaluator.filter_items(&items);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "1");
        assert_eq!(results[1].id, "3");
    }

    #[test]
    fn test_filter_items_empty_input() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Today;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let items: Vec<Item> = vec![];
        let results = evaluator.filter_items(&items);

        assert!(results.is_empty());
    }

    #[test]
    fn test_filter_items_no_matches() {
        let context = FilterContext::new(&[], &[], &[]);
        let filter = Filter::Priority1;
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item1 = make_item("1", "P2 Task");
        item1.priority = 3;

        let mut item2 = make_item("2", "P3 Task");
        item2.priority = 2;

        let items = vec![item1, item2];
        let results = evaluator.filter_items(&items);

        assert!(results.is_empty());
    }

    // ==================== FilterContext Tests ====================

    #[test]
    fn test_context_find_project_by_name() {
        let projects = vec![
            make_project("proj-1", "Work", None),
            make_project("proj-2", "Personal", None),
        ];
        let context = FilterContext::new(&projects, &[], &[]);

        let found = context.find_project_by_name("Work");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "proj-1");

        let not_found = context.find_project_by_name("Shopping");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_context_find_project_case_insensitive() {
        let projects = vec![make_project("proj-1", "Work", None)];
        let context = FilterContext::new(&projects, &[], &[]);

        assert!(context.find_project_by_name("work").is_some());
        assert!(context.find_project_by_name("WORK").is_some());
        assert!(context.find_project_by_name("Work").is_some());
    }

    #[test]
    fn test_context_get_project_ids_with_subprojects() {
        let projects = vec![
            make_project("proj-1", "Work", None),
            make_project("proj-2", "Meetings", Some("proj-1")),
            make_project("proj-3", "Weekly", Some("proj-2")),
            make_project("proj-4", "Personal", None),
        ];
        let context = FilterContext::new(&projects, &[], &[]);

        let ids = context.get_project_ids_with_subprojects("Work");
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"proj-1"));
        assert!(ids.contains(&"proj-2"));
        assert!(ids.contains(&"proj-3"));
        assert!(!ids.contains(&"proj-4"));
    }

    #[test]
    fn test_context_find_section_by_name() {
        let sections = vec![
            make_section("sec-1", "To Do", "proj-1"),
            make_section("sec-2", "Done", "proj-1"),
        ];
        let context = FilterContext::new(&[], &sections, &[]);

        let found = context.find_section_by_name("To Do");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "sec-1");

        let not_found = context.find_section_by_name("In Progress");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_context_label_exists() {
        let labels = vec![make_label("l1", "urgent"), make_label("l2", "work")];
        let context = FilterContext::new(&[], &[], &labels);

        assert!(context.label_exists("urgent"));
        assert!(context.label_exists("URGENT")); // case insensitive
        assert!(context.label_exists("work"));
        assert!(!context.label_exists("personal"));
    }

    // ==================== is_deleted Filtering Tests ====================

    #[test]
    fn test_context_find_project_excludes_deleted() {
        let mut deleted_project = make_project("proj-1", "Work", None);
        deleted_project.is_deleted = true;

        let active_project = make_project("proj-2", "Personal", None);
        let projects = vec![deleted_project, active_project];
        let context = FilterContext::new(&projects, &[], &[]);

        // Deleted project should not be found
        assert!(context.find_project_by_name("Work").is_none());

        // Active project should be found
        assert!(context.find_project_by_name("Personal").is_some());
    }

    #[test]
    fn test_context_find_section_excludes_deleted() {
        let mut deleted_section = make_section("sec-1", "To Do", "proj-1");
        deleted_section.is_deleted = true;

        let active_section = make_section("sec-2", "Done", "proj-1");
        let sections = vec![deleted_section, active_section];
        let context = FilterContext::new(&[], &sections, &[]);

        // Deleted section should not be found
        assert!(context.find_section_by_name("To Do").is_none());

        // Active section should be found
        assert!(context.find_section_by_name("Done").is_some());
    }

    #[test]
    fn test_context_label_exists_excludes_deleted() {
        let mut deleted_label = make_label("l1", "urgent");
        deleted_label.is_deleted = true;

        let active_label = make_label("l2", "work");
        let labels = vec![deleted_label, active_label];
        let context = FilterContext::new(&[], &[], &labels);

        // Deleted label should not exist
        assert!(!context.label_exists("urgent"));

        // Active label should exist
        assert!(context.label_exists("work"));
    }

    #[test]
    fn test_context_get_project_ids_with_subprojects_excludes_deleted() {
        let root_project = make_project("proj-1", "Work", None);

        let mut deleted_subproject = make_project("proj-2", "Meetings", Some("proj-1"));
        deleted_subproject.is_deleted = true;

        let active_subproject = make_project("proj-3", "Tasks", Some("proj-1"));

        let projects = vec![root_project, deleted_subproject, active_subproject];
        let context = FilterContext::new(&projects, &[], &[]);

        let ids = context.get_project_ids_with_subprojects("Work");

        // Should include root and active subproject, but not deleted subproject
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"proj-1"));
        assert!(!ids.contains(&"proj-2")); // Deleted
        assert!(ids.contains(&"proj-3"));
    }

    #[test]
    fn test_filter_project_excludes_deleted_project() {
        let mut deleted_project = make_project("proj-1", "Work", None);
        deleted_project.is_deleted = true;

        let projects = vec![deleted_project];
        let context = FilterContext::new(&projects, &[], &[]);
        let filter = Filter::Project("Work".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.project_id = "proj-1".to_string();

        // Filter should not match because project is deleted
        assert!(!evaluator.matches(&item));
    }

    #[test]
    fn test_filter_section_excludes_deleted_section() {
        let mut deleted_section = make_section("sec-1", "Inbox", "proj-1");
        deleted_section.is_deleted = true;

        let sections = vec![deleted_section];
        let context = FilterContext::new(&[], &sections, &[]);
        let filter = Filter::Section("Inbox".to_string());
        let evaluator = FilterEvaluator::new(&filter, &context);

        let mut item = make_item("1", "Task");
        item.section_id = Some("sec-1".to_string());

        // Filter should not match because section is deleted
        assert!(!evaluator.matches(&item));
    }
}
