//! Comprehensive E2E tests for filter evaluation against real Todoist API.
//!
//! Tests spec section 9: Filter Evaluation
//!
//! These tests validate filter parsing and evaluation against real Todoist data.
//! They require a valid Todoist API token set in .env.local as:
//! TODOIST_TEST_API_TOKEN=<token>
//!
//! Run with: cargo test --package todoist-cache --features e2e --test filter_comprehensive_e2e

#![cfg(feature = "e2e")]

use std::fs;

use chrono::{Duration, Local};
use todoist_api_rs::client::TodoistClient;
use todoist_api_rs::sync::{SyncCommand, SyncRequest, SyncResponse};
use todoist_cache_rs::filter::{FilterContext, FilterEvaluator, FilterParser};

// ============================================================================
// Test Context for Rate Limit Management
// ============================================================================

/// Reads the API token from .env.local or environment variable.
fn get_test_token() -> Option<String> {
    // Try environment variable first
    if let Ok(token) = std::env::var("TODOIST_TEST_API_TOKEN") {
        return Some(token);
    }
    if let Ok(token) = std::env::var("TODOIST_TEST_API_KEY") {
        return Some(token);
    }

    // Try various relative paths from different working directories
    let paths = [
        ".env.local",
        "../.env.local",
        "../../.env.local",
        "../../../.env.local",
        "../../../../.env.local",
    ];

    for path in &paths {
        if let Ok(contents) = fs::read_to_string(path) {
            for line in contents.lines() {
                if let Some(token) = line
                    .strip_prefix("TODOIST_TEST_API_TOKEN=")
                    .or_else(|| line.strip_prefix("todoist_test_api_key="))
                {
                    return Some(token.trim().to_string());
                }
            }
        }
    }

    None
}

/// Helper context for managing test state with minimal API calls
struct FilterTestContext {
    client: TodoistClient,
    sync_token: String,
    inbox_id: String,
    items: Vec<todoist_api_rs::sync::Item>,
    projects: Vec<todoist_api_rs::sync::Project>,
    sections: Vec<todoist_api_rs::sync::Section>,
    labels: Vec<todoist_api_rs::sync::Label>,
}

impl FilterTestContext {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let token = get_test_token().ok_or("TODOIST_TEST_API_TOKEN not found")?;
        let client = TodoistClient::new(token)?;

        // ONE full sync at initialization
        let response = client.sync(SyncRequest::full_sync()).await?;

        let inbox_id = response
            .projects
            .iter()
            .find(|p| p.inbox_project && !p.is_deleted)
            .ok_or("Should have inbox project")?
            .id
            .clone();

        Ok(Self {
            client,
            sync_token: response.sync_token,
            inbox_id,
            items: response.items,
            projects: response.projects,
            sections: response.sections,
            labels: response.labels,
        })
    }

    fn inbox_id(&self) -> &str {
        &self.inbox_id
    }

    async fn execute(
        &mut self,
        commands: Vec<SyncCommand>,
    ) -> Result<SyncResponse, todoist_api_rs::error::Error> {
        let request = SyncRequest::incremental(&self.sync_token)
            .with_resource_types(vec!["all".to_string()])
            .add_commands(commands);

        let response = self.client.sync(request).await?;
        self.sync_token = response.sync_token.clone();
        self.merge_response(&response);
        Ok(response)
    }

    fn merge_response(&mut self, response: &SyncResponse) {
        // Merge items
        for item in &response.items {
            if let Some(existing) = self.items.iter_mut().find(|i| i.id == item.id) {
                *existing = item.clone();
            } else {
                self.items.push(item.clone());
            }
        }
        // Merge projects
        for project in &response.projects {
            if let Some(existing) = self.projects.iter_mut().find(|p| p.id == project.id) {
                *existing = project.clone();
            } else {
                self.projects.push(project.clone());
            }
        }
        // Merge sections
        for section in &response.sections {
            if let Some(existing) = self.sections.iter_mut().find(|s| s.id == section.id) {
                *existing = section.clone();
            } else {
                self.sections.push(section.clone());
            }
        }
        // Merge labels
        for label in &response.labels {
            if let Some(existing) = self.labels.iter_mut().find(|l| l.id == label.id) {
                *existing = label.clone();
            } else {
                self.labels.push(label.clone());
            }
        }
    }

    fn find_item(&self, id: &str) -> Option<&todoist_api_rs::sync::Item> {
        self.items.iter().find(|i| i.id == id && !i.is_deleted)
    }

    /// Evaluate a filter against the current cached items
    fn evaluate_filter(&self, filter_query: &str) -> Vec<&todoist_api_rs::sync::Item> {
        let filter = FilterParser::parse(filter_query).expect("Filter should parse");
        let context = FilterContext::new(&self.projects, &self.sections, &self.labels);
        let evaluator = FilterEvaluator::new(&filter, &context);
        evaluator.filter_items(&self.items)
    }

    /// Create a task with given parameters
    async fn create_task(
        &mut self,
        content: &str,
        project_id: &str,
        extra_args: Option<serde_json::Value>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let mut args = serde_json::json!({
            "content": content,
            "project_id": project_id
        });

        if let Some(extra) = extra_args {
            if let (Some(obj), Some(extra_obj)) = (args.as_object_mut(), extra.as_object()) {
                for (k, v) in extra_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }

        let command = SyncCommand::with_temp_id("item_add", &temp_id, args);
        let response = self.execute(vec![command]).await?;

        response
            .real_id(&temp_id)
            .cloned()
            .ok_or_else(|| "No temp_id mapping returned".into())
    }

    /// Create a project
    async fn create_project(&mut self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let command =
            SyncCommand::with_temp_id("project_add", &temp_id, serde_json::json!({ "name": name }));
        let response = self.execute(vec![command]).await?;
        response
            .real_id(&temp_id)
            .cloned()
            .ok_or_else(|| "No temp_id mapping returned".into())
    }

    /// Create a project with a parent
    async fn create_subproject(
        &mut self,
        name: &str,
        parent_id: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let command = SyncCommand::with_temp_id(
            "project_add",
            &temp_id,
            serde_json::json!({ "name": name, "parent_id": parent_id }),
        );
        let response = self.execute(vec![command]).await?;
        response
            .real_id(&temp_id)
            .cloned()
            .ok_or_else(|| "No temp_id mapping returned".into())
    }

    /// Create a section
    async fn create_section(
        &mut self,
        name: &str,
        project_id: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let command = SyncCommand::with_temp_id(
            "section_add",
            &temp_id,
            serde_json::json!({ "name": name, "project_id": project_id }),
        );
        let response = self.execute(vec![command]).await?;
        response
            .real_id(&temp_id)
            .cloned()
            .ok_or_else(|| "No temp_id mapping returned".into())
    }

    /// Create a label
    async fn create_label(&mut self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let temp_id = uuid::Uuid::new_v4().to_string();
        let command =
            SyncCommand::with_temp_id("label_add", &temp_id, serde_json::json!({ "name": name }));
        let response = self.execute(vec![command]).await?;
        response
            .real_id(&temp_id)
            .cloned()
            .ok_or_else(|| "No temp_id mapping returned".into())
    }

    /// Batch delete resources
    async fn batch_delete(
        &mut self,
        task_ids: &[&str],
        project_ids: &[&str],
        section_ids: &[&str],
        label_ids: &[&str],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut commands = Vec::new();

        for id in task_ids {
            commands.push(SyncCommand::new(
                "item_delete",
                serde_json::json!({"id": id}),
            ));
        }
        for id in section_ids {
            commands.push(SyncCommand::new(
                "section_delete",
                serde_json::json!({"id": id}),
            ));
        }
        for id in project_ids {
            commands.push(SyncCommand::new(
                "project_delete",
                serde_json::json!({"id": id}),
            ));
        }
        for id in label_ids {
            commands.push(SyncCommand::new(
                "label_delete",
                serde_json::json!({"id": id}),
            ));
        }

        if !commands.is_empty() {
            let response = self.execute(commands).await?;
            if response.has_errors() {
                eprintln!(
                    "Warning: Some cleanup operations failed: {:?}",
                    response.errors()
                );
            }
        }

        Ok(())
    }
}

// ============================================================================
// Date Helpers
// ============================================================================

fn today_str() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn tomorrow_str() -> String {
    (Local::now() + Duration::days(1))
        .format("%Y-%m-%d")
        .to_string()
}

fn yesterday_str() -> String {
    (Local::now() - Duration::days(1))
        .format("%Y-%m-%d")
        .to_string()
}

fn days_from_now(days: i64) -> String {
    (Local::now() + Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

// ============================================================================
// 9.1 Date Filters
// ============================================================================

#[tokio::test]
async fn test_filter_today() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();
    let today = today_str();
    let tomorrow = tomorrow_str();

    // Create: task due today, task due tomorrow, task with no date
    let task_today = ctx
        .create_task(
            "E2E filter test - today",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": today}})),
        )
        .await
        .expect("create task");

    let task_tomorrow = ctx
        .create_task(
            "E2E filter test - tomorrow",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": tomorrow}})),
        )
        .await
        .expect("create task");

    let task_no_date = ctx
        .create_task("E2E filter test - no date", &inbox_id, None)
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("today");

    // Today task should match
    assert!(
        matches.iter().any(|i| i.id == task_today),
        "Filter 'today' should match task due today"
    );

    // Tomorrow task should NOT match
    assert!(
        !matches.iter().any(|i| i.id == task_tomorrow),
        "Filter 'today' should NOT match task due tomorrow"
    );

    // No date task should NOT match
    assert!(
        !matches.iter().any(|i| i.id == task_no_date),
        "Filter 'today' should NOT match task with no date"
    );

    // Cleanup
    ctx.batch_delete(&[&task_today, &task_tomorrow, &task_no_date], &[], &[], &[])
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn test_filter_tomorrow() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();
    let today = today_str();
    let tomorrow = tomorrow_str();
    let next_week = days_from_now(7);

    // Create tasks
    let task_today = ctx
        .create_task(
            "E2E filter test - today",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": today}})),
        )
        .await
        .expect("create task");

    let task_tomorrow = ctx
        .create_task(
            "E2E filter test - tomorrow",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": tomorrow}})),
        )
        .await
        .expect("create task");

    let task_next_week = ctx
        .create_task(
            "E2E filter test - next week",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": next_week}})),
        )
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("tomorrow");

    // Only tomorrow task should match
    assert!(
        matches.iter().any(|i| i.id == task_tomorrow),
        "Filter 'tomorrow' should match task due tomorrow"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_today),
        "Filter 'tomorrow' should NOT match task due today"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_next_week),
        "Filter 'tomorrow' should NOT match task due next week"
    );

    // Cleanup
    ctx.batch_delete(
        &[&task_today, &task_tomorrow, &task_next_week],
        &[],
        &[],
        &[],
    )
    .await
    .expect("cleanup");
}

#[tokio::test]
async fn test_filter_overdue() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();
    let yesterday = yesterday_str();
    let today = today_str();
    let tomorrow = tomorrow_str();

    // Create tasks
    let task_yesterday = ctx
        .create_task(
            "E2E filter test - yesterday",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": yesterday}})),
        )
        .await
        .expect("create task");

    let task_today = ctx
        .create_task(
            "E2E filter test - today",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": today}})),
        )
        .await
        .expect("create task");

    let task_tomorrow = ctx
        .create_task(
            "E2E filter test - tomorrow",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": tomorrow}})),
        )
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("overdue");

    // Only yesterday task should match (overdue)
    assert!(
        matches.iter().any(|i| i.id == task_yesterday),
        "Filter 'overdue' should match task due yesterday"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_today),
        "Filter 'overdue' should NOT match task due today"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_tomorrow),
        "Filter 'overdue' should NOT match task due tomorrow"
    );

    // Cleanup
    ctx.batch_delete(
        &[&task_yesterday, &task_today, &task_tomorrow],
        &[],
        &[],
        &[],
    )
    .await
    .expect("cleanup");
}

#[tokio::test]
async fn test_filter_no_date() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();
    let today = today_str();

    // Create tasks
    let task_with_date = ctx
        .create_task(
            "E2E filter test - with date",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": today}})),
        )
        .await
        .expect("create task");

    let task_no_date = ctx
        .create_task("E2E filter test - no date", &inbox_id, None)
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("no date");

    // Only no-date task should match
    assert!(
        matches.iter().any(|i| i.id == task_no_date),
        "Filter 'no date' should match task without due date"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_with_date),
        "Filter 'no date' should NOT match task with due date"
    );

    // Cleanup
    ctx.batch_delete(&[&task_with_date, &task_no_date], &[], &[], &[])
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn test_filter_7_days() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();
    let today = today_str();
    let in_5_days = days_from_now(5);
    let in_10_days = days_from_now(10);

    // Create tasks
    let task_today = ctx
        .create_task(
            "E2E filter test - due today",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": today}})),
        )
        .await
        .expect("create task");

    let task_5_days = ctx
        .create_task(
            "E2E filter test - due in 5 days",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": in_5_days}})),
        )
        .await
        .expect("create task");

    let task_10_days = ctx
        .create_task(
            "E2E filter test - due in 10 days",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": in_10_days}})),
        )
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("7 days");

    // Tasks due today and in 5 days should match (within 7 days)
    assert!(
        matches.iter().any(|i| i.id == task_today),
        "Filter '7 days' should match task due today"
    );
    assert!(
        matches.iter().any(|i| i.id == task_5_days),
        "Filter '7 days' should match task due in 5 days"
    );

    // Task due in 10 days should NOT match
    assert!(
        !matches.iter().any(|i| i.id == task_10_days),
        "Filter '7 days' should NOT match task due in 10 days"
    );

    // Cleanup
    ctx.batch_delete(&[&task_today, &task_5_days, &task_10_days], &[], &[], &[])
        .await
        .expect("cleanup");
}

// ============================================================================
// 9.2 Priority Filters
// ============================================================================

#[tokio::test]
async fn test_filter_p1() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create tasks with different priorities
    // API: priority 4 = p1, 3 = p2, 2 = p3, 1 = p4 (default)
    let task_p1 = ctx
        .create_task(
            "E2E filter test - p1",
            &inbox_id,
            Some(serde_json::json!({"priority": 4})),
        )
        .await
        .expect("create task");

    let task_p2 = ctx
        .create_task(
            "E2E filter test - p2",
            &inbox_id,
            Some(serde_json::json!({"priority": 3})),
        )
        .await
        .expect("create task");

    let task_p3 = ctx
        .create_task(
            "E2E filter test - p3",
            &inbox_id,
            Some(serde_json::json!({"priority": 2})),
        )
        .await
        .expect("create task");

    let task_p4 = ctx
        .create_task(
            "E2E filter test - p4",
            &inbox_id,
            Some(serde_json::json!({"priority": 1})),
        )
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("p1");

    // Only p1 task should match
    assert!(
        matches.iter().any(|i| i.id == task_p1),
        "Filter 'p1' should match p1 task"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_p2),
        "Filter 'p1' should NOT match p2 task"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_p3),
        "Filter 'p1' should NOT match p3 task"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_p4),
        "Filter 'p1' should NOT match p4 task"
    );

    // Cleanup
    ctx.batch_delete(&[&task_p1, &task_p2, &task_p3, &task_p4], &[], &[], &[])
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn test_filter_p1_or_p2() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create tasks with different priorities
    let task_p1 = ctx
        .create_task(
            "E2E filter test - p1",
            &inbox_id,
            Some(serde_json::json!({"priority": 4})),
        )
        .await
        .expect("create task");

    let task_p2 = ctx
        .create_task(
            "E2E filter test - p2",
            &inbox_id,
            Some(serde_json::json!({"priority": 3})),
        )
        .await
        .expect("create task");

    let task_p3 = ctx
        .create_task(
            "E2E filter test - p3",
            &inbox_id,
            Some(serde_json::json!({"priority": 2})),
        )
        .await
        .expect("create task");

    let task_p4 = ctx
        .create_task(
            "E2E filter test - p4",
            &inbox_id,
            Some(serde_json::json!({"priority": 1})),
        )
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("p1 | p2");

    // p1 and p2 should match
    assert!(
        matches.iter().any(|i| i.id == task_p1),
        "Filter 'p1 | p2' should match p1 task"
    );
    assert!(
        matches.iter().any(|i| i.id == task_p2),
        "Filter 'p1 | p2' should match p2 task"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_p3),
        "Filter 'p1 | p2' should NOT match p3 task"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_p4),
        "Filter 'p1 | p2' should NOT match p4 task"
    );

    // Cleanup
    ctx.batch_delete(&[&task_p1, &task_p2, &task_p3, &task_p4], &[], &[], &[])
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn test_filter_p4_default_priority() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create task with no explicit priority (defaults to p4)
    let task_default = ctx
        .create_task("E2E filter test - default priority", &inbox_id, None)
        .await
        .expect("create task");

    // Create task with explicit p1
    let task_p1 = ctx
        .create_task(
            "E2E filter test - p1",
            &inbox_id,
            Some(serde_json::json!({"priority": 4})),
        )
        .await
        .expect("create task");

    // Verify default priority is p4 (API value 1)
    let item = ctx.find_item(&task_default).expect("Task should exist");
    assert_eq!(item.priority, 1, "Default priority should be 1 (p4)");

    // Evaluate filter
    let matches = ctx.evaluate_filter("p4");

    // Default priority task should match p4
    assert!(
        matches.iter().any(|i| i.id == task_default),
        "Filter 'p4' should match default priority task"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_p1),
        "Filter 'p4' should NOT match p1 task"
    );

    // Cleanup
    ctx.batch_delete(&[&task_default, &task_p1], &[], &[], &[])
        .await
        .expect("cleanup");
}

// ============================================================================
// 9.3 Label Filters
// ============================================================================

#[tokio::test]
async fn test_filter_single_label() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create labels
    let label_work = ctx
        .create_label("e2e_filter_work")
        .await
        .expect("create label");
    let label_home = ctx
        .create_label("e2e_filter_home")
        .await
        .expect("create label");

    // Create tasks
    let task_work = ctx
        .create_task(
            "E2E filter test - work label",
            &inbox_id,
            Some(serde_json::json!({"labels": ["e2e_filter_work"]})),
        )
        .await
        .expect("create task");

    let task_home = ctx
        .create_task(
            "E2E filter test - home label",
            &inbox_id,
            Some(serde_json::json!({"labels": ["e2e_filter_home"]})),
        )
        .await
        .expect("create task");

    let task_no_label = ctx
        .create_task("E2E filter test - no label", &inbox_id, None)
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("@e2e_filter_work");

    // Only work-labeled task should match
    assert!(
        matches.iter().any(|i| i.id == task_work),
        "Filter '@e2e_filter_work' should match work-labeled task"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_home),
        "Filter '@e2e_filter_work' should NOT match home-labeled task"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_no_label),
        "Filter '@e2e_filter_work' should NOT match unlabeled task"
    );

    // Cleanup
    ctx.batch_delete(
        &[&task_work, &task_home, &task_no_label],
        &[],
        &[],
        &[&label_work, &label_home],
    )
    .await
    .expect("cleanup");
}

#[tokio::test]
async fn test_filter_no_labels() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a label
    let label_test = ctx
        .create_label("e2e_filter_no_labels_test")
        .await
        .expect("create label");

    // Create task with label
    let task_with_label = ctx
        .create_task(
            "E2E filter test - with label",
            &inbox_id,
            Some(serde_json::json!({"labels": ["e2e_filter_no_labels_test"]})),
        )
        .await
        .expect("create task");

    // Create task without labels
    let task_no_label = ctx
        .create_task("E2E filter test - no label", &inbox_id, None)
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("no labels");

    // Only unlabeled task should match
    assert!(
        matches.iter().any(|i| i.id == task_no_label),
        "Filter 'no labels' should match task without labels"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_with_label),
        "Filter 'no labels' should NOT match task with labels"
    );

    // Cleanup
    ctx.batch_delete(
        &[&task_with_label, &task_no_label],
        &[],
        &[],
        &[&label_test],
    )
    .await
    .expect("cleanup");
}

#[tokio::test]
async fn test_filter_multiple_labels_and() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create labels
    let label_a = ctx
        .create_label("e2e_filter_label_a")
        .await
        .expect("create label");
    let label_b = ctx
        .create_label("e2e_filter_label_b")
        .await
        .expect("create label");

    // Create tasks with different label combinations
    let task_a_only = ctx
        .create_task(
            "E2E filter test - label A only",
            &inbox_id,
            Some(serde_json::json!({"labels": ["e2e_filter_label_a"]})),
        )
        .await
        .expect("create task");

    let task_b_only = ctx
        .create_task(
            "E2E filter test - label B only",
            &inbox_id,
            Some(serde_json::json!({"labels": ["e2e_filter_label_b"]})),
        )
        .await
        .expect("create task");

    let task_both = ctx
        .create_task(
            "E2E filter test - both labels",
            &inbox_id,
            Some(serde_json::json!({"labels": ["e2e_filter_label_a", "e2e_filter_label_b"]})),
        )
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("@e2e_filter_label_a & @e2e_filter_label_b");

    // Only task with both labels should match
    assert!(
        matches.iter().any(|i| i.id == task_both),
        "Filter with AND should match task with both labels"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_a_only),
        "Filter with AND should NOT match task with only label A"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_b_only),
        "Filter with AND should NOT match task with only label B"
    );

    // Cleanup
    ctx.batch_delete(
        &[&task_a_only, &task_b_only, &task_both],
        &[],
        &[],
        &[&label_a, &label_b],
    )
    .await
    .expect("cleanup");
}

#[tokio::test]
async fn test_filter_multiple_labels_or() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create labels
    let label_a = ctx
        .create_label("e2e_filter_or_a")
        .await
        .expect("create label");
    let label_b = ctx
        .create_label("e2e_filter_or_b")
        .await
        .expect("create label");
    let label_c = ctx
        .create_label("e2e_filter_or_c")
        .await
        .expect("create label");

    // Create tasks
    let task_a = ctx
        .create_task(
            "E2E filter test - label A",
            &inbox_id,
            Some(serde_json::json!({"labels": ["e2e_filter_or_a"]})),
        )
        .await
        .expect("create task");

    let task_b = ctx
        .create_task(
            "E2E filter test - label B",
            &inbox_id,
            Some(serde_json::json!({"labels": ["e2e_filter_or_b"]})),
        )
        .await
        .expect("create task");

    let task_c = ctx
        .create_task(
            "E2E filter test - label C",
            &inbox_id,
            Some(serde_json::json!({"labels": ["e2e_filter_or_c"]})),
        )
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("@e2e_filter_or_a | @e2e_filter_or_b");

    // A and B should match, C should not
    assert!(
        matches.iter().any(|i| i.id == task_a),
        "Filter with OR should match task with label A"
    );
    assert!(
        matches.iter().any(|i| i.id == task_b),
        "Filter with OR should match task with label B"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_c),
        "Filter with OR should NOT match task with label C"
    );

    // Cleanup
    ctx.batch_delete(
        &[&task_a, &task_b, &task_c],
        &[],
        &[],
        &[&label_a, &label_b, &label_c],
    )
    .await
    .expect("cleanup");
}

// ============================================================================
// 9.4 Project Filters
// ============================================================================

#[tokio::test]
async fn test_filter_project() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create test project
    let project_a = ctx
        .create_project("E2E_FilterTest_ProjectA")
        .await
        .expect("create project");

    let project_b = ctx
        .create_project("E2E_FilterTest_ProjectB")
        .await
        .expect("create project");

    // Create tasks in different projects
    let task_in_a = ctx
        .create_task("E2E filter test - in project A", &project_a, None)
        .await
        .expect("create task");

    let task_in_b = ctx
        .create_task("E2E filter test - in project B", &project_b, None)
        .await
        .expect("create task");

    let task_in_inbox = ctx
        .create_task("E2E filter test - in inbox", &inbox_id, None)
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("#E2E_FilterTest_ProjectA");

    // Only task in project A should match
    assert!(
        matches.iter().any(|i| i.id == task_in_a),
        "Filter '#ProjectA' should match task in project A"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_in_b),
        "Filter '#ProjectA' should NOT match task in project B"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_in_inbox),
        "Filter '#ProjectA' should NOT match task in inbox"
    );

    // Cleanup
    ctx.batch_delete(
        &[&task_in_a, &task_in_b, &task_in_inbox],
        &[&project_a, &project_b],
        &[],
        &[],
    )
    .await
    .expect("cleanup");
}

#[tokio::test]
async fn test_filter_project_with_subprojects() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    // Create parent project and subproject
    let parent_project = ctx
        .create_project("E2E_FilterTest_Parent")
        .await
        .expect("create project");

    let child_project = ctx
        .create_subproject("E2E_FilterTest_Child", &parent_project)
        .await
        .expect("create subproject");

    let other_project = ctx
        .create_project("E2E_FilterTest_Other")
        .await
        .expect("create project");

    // Create tasks
    let task_in_parent = ctx
        .create_task("E2E filter test - in parent", &parent_project, None)
        .await
        .expect("create task");

    let task_in_child = ctx
        .create_task("E2E filter test - in child", &child_project, None)
        .await
        .expect("create task");

    let task_in_other = ctx
        .create_task("E2E filter test - in other", &other_project, None)
        .await
        .expect("create task");

    // Evaluate filter with ## (includes subprojects)
    let matches = ctx.evaluate_filter("##E2E_FilterTest_Parent");

    // Both parent and child tasks should match
    assert!(
        matches.iter().any(|i| i.id == task_in_parent),
        "Filter '##Parent' should match task in parent project"
    );
    assert!(
        matches.iter().any(|i| i.id == task_in_child),
        "Filter '##Parent' should match task in child project"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_in_other),
        "Filter '##Parent' should NOT match task in other project"
    );

    // Cleanup
    ctx.batch_delete(
        &[&task_in_parent, &task_in_child, &task_in_other],
        &[&child_project, &parent_project, &other_project],
        &[],
        &[],
    )
    .await
    .expect("cleanup");
}

#[tokio::test]
async fn test_filter_inbox() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create a project
    let project = ctx
        .create_project("E2E_FilterTest_NotInbox")
        .await
        .expect("create project");

    // Create tasks
    let task_in_inbox = ctx
        .create_task("E2E filter test - in inbox", &inbox_id, None)
        .await
        .expect("create task");

    let task_in_project = ctx
        .create_task("E2E filter test - in project", &project, None)
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("#Inbox");

    // Only inbox task should match
    assert!(
        matches.iter().any(|i| i.id == task_in_inbox),
        "Filter '#Inbox' should match task in inbox"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_in_project),
        "Filter '#Inbox' should NOT match task in project"
    );

    // Cleanup
    ctx.batch_delete(&[&task_in_inbox, &task_in_project], &[&project], &[], &[])
        .await
        .expect("cleanup");
}

// ============================================================================
// 9.5 Section Filters
// ============================================================================

#[tokio::test]
async fn test_filter_section() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    // Create project with sections
    let project = ctx
        .create_project("E2E_FilterTest_WithSections")
        .await
        .expect("create project");

    let section_a = ctx
        .create_section("E2E_Section_A", &project)
        .await
        .expect("create section");

    let section_b = ctx
        .create_section("E2E_Section_B", &project)
        .await
        .expect("create section");

    // Create tasks in different sections
    let task_in_a = ctx
        .create_task(
            "E2E filter test - in section A",
            &project,
            Some(serde_json::json!({"section_id": section_a})),
        )
        .await
        .expect("create task");

    let task_in_b = ctx
        .create_task(
            "E2E filter test - in section B",
            &project,
            Some(serde_json::json!({"section_id": section_b})),
        )
        .await
        .expect("create task");

    let task_no_section = ctx
        .create_task("E2E filter test - no section", &project, None)
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("/E2E_Section_A");

    // Only task in section A should match
    assert!(
        matches.iter().any(|i| i.id == task_in_a),
        "Filter '/Section_A' should match task in section A"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_in_b),
        "Filter '/Section_A' should NOT match task in section B"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_no_section),
        "Filter '/Section_A' should NOT match task without section"
    );

    // Cleanup
    ctx.batch_delete(
        &[&task_in_a, &task_in_b, &task_no_section],
        &[&project],
        &[&section_a, &section_b],
        &[],
    )
    .await
    .expect("cleanup");
}

#[tokio::test]
async fn test_filter_section_in_project() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    // Create two projects with same-named sections
    let project1 = ctx
        .create_project("E2E_FilterTest_Project1")
        .await
        .expect("create project");

    let project2 = ctx
        .create_project("E2E_FilterTest_Project2")
        .await
        .expect("create project");

    let section_in_1 = ctx
        .create_section("E2E_Done", &project1)
        .await
        .expect("create section");

    let section_in_2 = ctx
        .create_section("E2E_Done", &project2)
        .await
        .expect("create section");

    // Create tasks
    let task_in_1_done = ctx
        .create_task(
            "E2E filter test - project1 done",
            &project1,
            Some(serde_json::json!({"section_id": section_in_1})),
        )
        .await
        .expect("create task");

    let task_in_2_done = ctx
        .create_task(
            "E2E filter test - project2 done",
            &project2,
            Some(serde_json::json!({"section_id": section_in_2})),
        )
        .await
        .expect("create task");

    // Evaluate filter: #Project1 & /Done
    let matches = ctx.evaluate_filter("#E2E_FilterTest_Project1 & /E2E_Done");

    // Only task in Project1's Done section should match
    assert!(
        matches.iter().any(|i| i.id == task_in_1_done),
        "Filter '#Project1 & /Done' should match task in Project1's Done section"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_in_2_done),
        "Filter '#Project1 & /Done' should NOT match task in Project2's Done section"
    );

    // Cleanup
    ctx.batch_delete(
        &[&task_in_1_done, &task_in_2_done],
        &[&project1, &project2],
        &[&section_in_1, &section_in_2],
        &[],
    )
    .await
    .expect("cleanup");
}

// ============================================================================
// 9.6 Complex Filters
// ============================================================================

#[tokio::test]
async fn test_filter_and_precedence() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();
    let today = today_str();

    // Create label
    let label_urgent = ctx
        .create_label("e2e_filter_urgent")
        .await
        .expect("create label");

    // Create tasks to test: "today | p1 & @urgent"
    // This should be parsed as: "today | (p1 & @urgent)"
    // So: tasks due today OR (p1 AND urgent) should match

    // Task: today only (should match)
    let task_today = ctx
        .create_task(
            "E2E filter test - today only",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": today}})),
        )
        .await
        .expect("create task");

    // Task: p1 + urgent (should match)
    let task_p1_urgent = ctx
        .create_task(
            "E2E filter test - p1 urgent",
            &inbox_id,
            Some(serde_json::json!({"priority": 4, "labels": ["e2e_filter_urgent"]})),
        )
        .await
        .expect("create task");

    // Task: p1 only (should NOT match)
    let task_p1_only = ctx
        .create_task(
            "E2E filter test - p1 only",
            &inbox_id,
            Some(serde_json::json!({"priority": 4})),
        )
        .await
        .expect("create task");

    // Task: urgent only with p4 (should NOT match)
    let task_urgent_only = ctx
        .create_task(
            "E2E filter test - urgent only",
            &inbox_id,
            Some(serde_json::json!({"labels": ["e2e_filter_urgent"]})),
        )
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("today | p1 & @e2e_filter_urgent");

    // today and p1+urgent should match
    assert!(
        matches.iter().any(|i| i.id == task_today),
        "Filter should match task due today"
    );
    assert!(
        matches.iter().any(|i| i.id == task_p1_urgent),
        "Filter should match p1 + urgent task"
    );
    // p1 only should NOT match (AND has higher precedence)
    assert!(
        !matches.iter().any(|i| i.id == task_p1_only),
        "Filter should NOT match p1 only task (AND has higher precedence)"
    );
    // urgent only should NOT match
    assert!(
        !matches.iter().any(|i| i.id == task_urgent_only),
        "Filter should NOT match urgent only task"
    );

    // Cleanup
    ctx.batch_delete(
        &[
            &task_today,
            &task_p1_urgent,
            &task_p1_only,
            &task_urgent_only,
        ],
        &[],
        &[],
        &[&label_urgent],
    )
    .await
    .expect("cleanup");
}

#[tokio::test]
async fn test_filter_parentheses() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();
    let today = today_str();
    let tomorrow = tomorrow_str();
    let next_week = days_from_now(7);

    // Test: "(today | tomorrow) & p1"
    // Task: today + p1 (should match)
    let task_today_p1 = ctx
        .create_task(
            "E2E filter test - today p1",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": today}, "priority": 4})),
        )
        .await
        .expect("create task");

    // Task: tomorrow + p1 (should match)
    let task_tomorrow_p1 = ctx
        .create_task(
            "E2E filter test - tomorrow p1",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": tomorrow}, "priority": 4})),
        )
        .await
        .expect("create task");

    // Task: today + p4 (should NOT match)
    let task_today_p4 = ctx
        .create_task(
            "E2E filter test - today p4",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": today}, "priority": 1})),
        )
        .await
        .expect("create task");

    // Task: next week + p1 (should NOT match)
    let task_nextweek_p1 = ctx
        .create_task(
            "E2E filter test - next week p1",
            &inbox_id,
            Some(serde_json::json!({"due": {"date": next_week}, "priority": 4})),
        )
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("(today | tomorrow) & p1");

    // today+p1 and tomorrow+p1 should match
    assert!(
        matches.iter().any(|i| i.id == task_today_p1),
        "Filter should match today + p1 task"
    );
    assert!(
        matches.iter().any(|i| i.id == task_tomorrow_p1),
        "Filter should match tomorrow + p1 task"
    );
    // today+p4 should NOT match (doesn't have p1)
    assert!(
        !matches.iter().any(|i| i.id == task_today_p4),
        "Filter should NOT match today + p4 task"
    );
    // next_week+p1 should NOT match (not today or tomorrow)
    assert!(
        !matches.iter().any(|i| i.id == task_nextweek_p1),
        "Filter should NOT match next week + p1 task"
    );

    // Cleanup
    ctx.batch_delete(
        &[
            &task_today_p1,
            &task_tomorrow_p1,
            &task_today_p4,
            &task_nextweek_p1,
        ],
        &[],
        &[],
        &[],
    )
    .await
    .expect("cleanup");
}

#[tokio::test]
async fn test_filter_negation_label() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create label
    let label_blocked = ctx
        .create_label("e2e_filter_blocked")
        .await
        .expect("create label");

    // Create tasks
    let task_blocked = ctx
        .create_task(
            "E2E filter test - blocked",
            &inbox_id,
            Some(serde_json::json!({"labels": ["e2e_filter_blocked"]})),
        )
        .await
        .expect("create task");

    let task_not_blocked = ctx
        .create_task("E2E filter test - not blocked", &inbox_id, None)
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("!@e2e_filter_blocked");

    // Blocked task should NOT match
    assert!(
        !matches.iter().any(|i| i.id == task_blocked),
        "Filter '!@blocked' should NOT match blocked task"
    );
    // Non-blocked task should match
    assert!(
        matches.iter().any(|i| i.id == task_not_blocked),
        "Filter '!@blocked' should match non-blocked task"
    );

    // Cleanup
    ctx.batch_delete(
        &[&task_blocked, &task_not_blocked],
        &[],
        &[],
        &[&label_blocked],
    )
    .await
    .expect("cleanup");
}

#[tokio::test]
async fn test_filter_negation_project() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let inbox_id = ctx.inbox_id().to_string();

    // Create project
    let project = ctx
        .create_project("E2E_FilterTest_ToExclude")
        .await
        .expect("create project");

    // Create tasks
    let task_in_project = ctx
        .create_task("E2E filter test - in project", &project, None)
        .await
        .expect("create task");

    let task_in_inbox = ctx
        .create_task("E2E filter test - in inbox", &inbox_id, None)
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter("!#E2E_FilterTest_ToExclude");

    // Task in excluded project should NOT match
    assert!(
        !matches.iter().any(|i| i.id == task_in_project),
        "Filter '!#Project' should NOT match task in that project"
    );
    // Task in inbox should match
    assert!(
        matches.iter().any(|i| i.id == task_in_inbox),
        "Filter '!#Project' should match task NOT in that project"
    );

    // Cleanup
    ctx.batch_delete(&[&task_in_project, &task_in_inbox], &[&project], &[], &[])
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn test_filter_complex_real_world() {
    let Ok(mut ctx) = FilterTestContext::new().await else {
        eprintln!("Skipping test: no API token");
        return;
    };

    let today = today_str();
    let yesterday = yesterday_str();
    let next_week = days_from_now(7);

    // Create project hierarchy
    let work_project = ctx
        .create_project("E2E_FilterTest_Work")
        .await
        .expect("create project");

    let work_subproject = ctx
        .create_subproject("E2E_FilterTest_WorkTasks", &work_project)
        .await
        .expect("create subproject");

    // Create label
    let label_blocked = ctx
        .create_label("e2e_filter_blocked_complex")
        .await
        .expect("create label");

    // Test filter: "##E2E_FilterTest_Work & (p1 | p2) & !@e2e_filter_blocked_complex & (today | overdue)"
    // Should match: tasks in Work or subprojects, with p1 or p2, NOT blocked, and due today or overdue

    // Task 1: Work project, p1, not blocked, today (should match)
    let task_match_1 = ctx
        .create_task(
            "E2E complex - match 1",
            &work_project,
            Some(serde_json::json!({"priority": 4, "due": {"date": today}})),
        )
        .await
        .expect("create task");

    // Task 2: Work subproject, p2, not blocked, overdue (should match)
    let task_match_2 = ctx
        .create_task(
            "E2E complex - match 2",
            &work_subproject,
            Some(serde_json::json!({"priority": 3, "due": {"date": yesterday}})),
        )
        .await
        .expect("create task");

    // Task 3: Work project, p1, blocked (should NOT match)
    let task_blocked = ctx
        .create_task(
            "E2E complex - blocked",
            &work_project,
            Some(serde_json::json!({"priority": 4, "due": {"date": today}, "labels": ["e2e_filter_blocked_complex"]})),
        )
        .await
        .expect("create task");

    // Task 4: Work project, p3 (should NOT match - wrong priority)
    let task_wrong_priority = ctx
        .create_task(
            "E2E complex - wrong priority",
            &work_project,
            Some(serde_json::json!({"priority": 2, "due": {"date": today}})),
        )
        .await
        .expect("create task");

    // Task 5: Work project, p1, next week (should NOT match - wrong date)
    let task_wrong_date = ctx
        .create_task(
            "E2E complex - wrong date",
            &work_project,
            Some(serde_json::json!({"priority": 4, "due": {"date": next_week}})),
        )
        .await
        .expect("create task");

    // Evaluate filter
    let matches = ctx.evaluate_filter(
        "##E2E_FilterTest_Work & (p1 | p2) & !@e2e_filter_blocked_complex & (today | overdue)",
    );

    // Verify matches
    assert!(
        matches.iter().any(|i| i.id == task_match_1),
        "Complex filter should match task 1"
    );
    assert!(
        matches.iter().any(|i| i.id == task_match_2),
        "Complex filter should match task 2"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_blocked),
        "Complex filter should NOT match blocked task"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_wrong_priority),
        "Complex filter should NOT match wrong priority task"
    );
    assert!(
        !matches.iter().any(|i| i.id == task_wrong_date),
        "Complex filter should NOT match wrong date task"
    );

    // Cleanup
    ctx.batch_delete(
        &[
            &task_match_1,
            &task_match_2,
            &task_blocked,
            &task_wrong_priority,
            &task_wrong_date,
        ],
        &[&work_subproject, &work_project],
        &[],
        &[&label_blocked],
    )
    .await
    .expect("cleanup");
}
