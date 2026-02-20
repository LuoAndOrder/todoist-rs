//! CLI-focused end-to-end tests against the real Todoist API.
//!
//! These tests validate realistic user workflows via the `td` binary.
//! They are intentionally scenario-driven (few tests, multi-step flows)
//! to keep coverage meaningful while minimizing rate-limit pressure.

#![cfg(feature = "e2e")]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::{Arc, OnceLock};

use serde_json::Value;
use serial_test::serial;
use tempfile::TempDir;
use uuid::Uuid;

/// Reads the E2E API token from environment or .env.local.
fn get_test_token() -> Option<String> {
    if let Ok(token) = env::var("TODOIST_TEST_API_TOKEN") {
        return Some(token);
    }
    if let Ok(token) = env::var("TODOIST_TEST_API_KEY") {
        return Some(token);
    }

    for path in &[".env.local", "../../.env.local", "../../../.env.local"] {
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

fn unique_suffix() -> String {
    let uuid = Uuid::new_v4().simple().to_string();
    uuid[..8].to_string()
}

fn resolve_td_binary_path() -> PathBuf {
    if let Ok(path) = env::var("CARGO_BIN_EXE_td") {
        return PathBuf::from(path);
    }

    // Fallback for environments where Cargo doesn't export CARGO_BIN_EXE_td
    // for this integration test binary.
    let test_binary = env::current_exe().expect("failed to resolve current test executable path");
    let debug_dir = test_binary
        .parent()
        .and_then(|p| p.parent())
        .expect("failed to resolve target/debug directory")
        .to_path_buf();

    let mut candidate = debug_dir.join("td");
    if cfg!(windows) {
        candidate.set_extension("exe");
    }

    assert!(
        candidate.exists(),
        "td binary not found at expected path: {}",
        candidate.display()
    );
    candidate
}

#[derive(Clone)]
struct CliE2eContext {
    bin_path: PathBuf,
    token: String,
    _sandbox: Arc<TempDir>,
    td_config_path: PathBuf,
    xdg_config_home: PathBuf,
    xdg_cache_home: PathBuf,
}

impl CliE2eContext {
    fn new(token: String) -> Self {
        let sandbox = Arc::new(TempDir::new().expect("failed to create temporary sandbox"));
        let xdg_config_home = sandbox.path().join("xdg-config");
        let xdg_cache_home = sandbox.path().join("xdg-cache");
        let td_config_path = sandbox.path().join("td-config.toml");

        fs::create_dir_all(&xdg_config_home).expect("failed to create XDG config dir");
        fs::create_dir_all(&xdg_cache_home).expect("failed to create XDG cache dir");

        let bin_path = resolve_td_binary_path();

        Self {
            bin_path,
            token,
            _sandbox: sandbox,
            td_config_path,
            xdg_config_home,
            xdg_cache_home,
        }
    }

    fn run_allow_failure(&self, args: &[&str]) -> Option<Output> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.args(args);
        cmd.env("TODOIST_TOKEN", &self.token);
        cmd.env("TD_CONFIG", &self.td_config_path);
        cmd.env("XDG_CONFIG_HOME", &self.xdg_config_home);
        cmd.env("XDG_CACHE_HOME", &self.xdg_cache_home);
        cmd.env("NO_COLOR", "1");

        match cmd.output() {
            Ok(output) => Some(output),
            Err(err) => {
                eprintln!(
                    "cleanup command failed to spawn for args {:?}: {}",
                    args, err
                );
                None
            }
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        let output = self
            .run_allow_failure(args)
            .expect("failed to run td command");

        if output.status.success() {
            return output;
        }

        panic!(
            "td command failed\nargs: {:?}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn run_json(&self, args: &[&str]) -> Value {
        let output = self.run(args);
        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&stdout).unwrap_or_else(|err| {
            panic!(
                "command did not emit valid JSON\nargs: {:?}\nerror: {}\nstdout:\n{}",
                args, err, stdout
            )
        })
    }

    fn run_json_owned(&self, args: Vec<String>) -> Value {
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run_json(&arg_refs)
    }
}

struct CleanupGuard {
    ctx: CliE2eContext,
    task_ids: Vec<String>,
    section_ids: Vec<String>,
    project_ids: Vec<String>,
    label_ids: Vec<String>,
}

impl CleanupGuard {
    fn new(ctx: CliE2eContext) -> Self {
        Self {
            ctx,
            task_ids: Vec::new(),
            section_ids: Vec::new(),
            project_ids: Vec::new(),
            label_ids: Vec::new(),
        }
    }

    fn track_task(&mut self, task_id: impl Into<String>) {
        self.task_ids.push(task_id.into());
    }

    fn track_section(&mut self, section_id: impl Into<String>) {
        self.section_ids.push(section_id.into());
    }

    fn track_project(&mut self, project_id: impl Into<String>) {
        self.project_ids.push(project_id.into());
    }

    fn track_label(&mut self, label_id: impl Into<String>) {
        self.label_ids.push(label_id.into());
    }

    fn cleanup_command(&self, args: Vec<String>) {
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        if let Some(output) = self.ctx.run_allow_failure(&arg_refs) {
            if !output.status.success() {
                eprintln!(
                    "cleanup command failed\nargs: {:?}\nstdout:\n{}\nstderr:\n{}",
                    arg_refs,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        for task_id in self.task_ids.iter().rev() {
            self.cleanup_command(vec![
                "--json".to_string(),
                "delete".to_string(),
                task_id.clone(),
                "--force".to_string(),
            ]);
        }

        for section_id in self.section_ids.iter().rev() {
            self.cleanup_command(vec![
                "--json".to_string(),
                "sections".to_string(),
                "delete".to_string(),
                section_id.clone(),
                "--force".to_string(),
            ]);
        }

        for project_id in self.project_ids.iter().rev() {
            self.cleanup_command(vec![
                "--json".to_string(),
                "projects".to_string(),
                "delete".to_string(),
                project_id.clone(),
                "--force".to_string(),
            ]);
        }

        for label_id in self.label_ids.iter().rev() {
            self.cleanup_command(vec![
                "--json".to_string(),
                "labels".to_string(),
                "delete".to_string(),
                label_id.clone(),
                "--force".to_string(),
            ]);
        }
    }
}

fn setup_context() -> Option<CliE2eContext> {
    static SHARED_CONTEXT: OnceLock<Option<CliE2eContext>> = OnceLock::new();

    SHARED_CONTEXT
        .get_or_init(|| {
            let token = get_test_token()?;
            let ctx = CliE2eContext::new(token);

            // Initialize shared cache once to minimize full-sync pressure.
            let sync = ctx.run_json(&["--json", "sync", "--full"]);
            assert_eq!(sync.get("status").and_then(Value::as_str), Some("success"));

            Some(ctx)
        })
        .clone()
}

fn required_str(json: &Value, key: &str) -> String {
    json.get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string field '{}' in JSON: {}", key, json))
        .to_string()
}

fn list_has_task(list_json: &Value, task_id: &str) -> bool {
    list_json
        .get("tasks")
        .and_then(Value::as_array)
        .is_some_and(|tasks| {
            tasks
                .iter()
                .any(|task| task.get("id").and_then(Value::as_str) == Some(task_id))
        })
}

#[test]
#[serial]
fn test_cli_e2e_move_task_between_projects_and_section() {
    let Some(ctx) = setup_context() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let mut cleanup = CleanupGuard::new(ctx.clone());
    let suffix = unique_suffix();

    let project_a_name = format!("e2e-cli-move-a-{}", suffix);
    let project_b_name = format!("e2e-cli-move-b-{}", suffix);
    let section_b_name = format!("e2e-cli-section-b-{}", suffix);
    let task_content = format!("E2E CLI move task {}", suffix);

    let project_a = ctx.run_json(&["--json", "projects", "add", &project_a_name]);
    let project_a_id = required_str(&project_a, "id");
    cleanup.track_project(project_a_id.clone());

    let project_b = ctx.run_json(&["--json", "projects", "add", &project_b_name]);
    let project_b_id = required_str(&project_b, "id");
    cleanup.track_project(project_b_id.clone());

    let section_b = ctx.run_json(&[
        "--json",
        "sections",
        "add",
        &section_b_name,
        "--project",
        &project_b_name,
    ]);
    let section_b_id = required_str(&section_b, "id");
    cleanup.track_section(section_b_id.clone());

    let task = ctx.run_json(&[
        "--json",
        "add",
        &task_content,
        "--project",
        &project_a_name,
        "--priority",
        "2",
    ]);
    let task_id = required_str(&task, "id");
    cleanup.track_task(task_id.clone());

    // Move between projects first, then place into section.
    // Todoist item_move expects one destination field per command.
    ctx.run_json(&[
        "--json",
        "edit",
        &task_id,
        "--project",
        &project_b_name,
        "--description",
        "moved by cli e2e",
    ]);
    ctx.run_json(&["--json", "sync"]);
    ctx.run_json(&["--json", "edit", &task_id, "--section", &section_b_name]);

    let moved = ctx.run_json(&["--json", "--sync", "show", &task_id]);
    assert_eq!(
        moved.get("project_id").and_then(Value::as_str),
        Some(project_b_id.as_str())
    );
    assert_eq!(
        moved.get("section_id").and_then(Value::as_str),
        Some(section_b_id.as_str())
    );
    assert_eq!(
        moved.get("description").and_then(Value::as_str),
        Some("moved by cli e2e")
    );

    let project_b_tasks = ctx.run_json(&["--json", "list", "--project", &project_b_name, "--all"]);
    assert!(
        list_has_task(&project_b_tasks, &task_id),
        "moved task should be listed in destination project"
    );

    let project_a_tasks = ctx.run_json(&["--json", "list", "--project", &project_a_name, "--all"]);
    assert!(
        !list_has_task(&project_a_tasks, &task_id),
        "moved task should no longer be listed in source project"
    );

    let sync = ctx.run_json(&["--json", "sync"]);
    assert_eq!(sync.get("status").and_then(Value::as_str), Some("success"));
}

#[test]
#[serial]
fn test_cli_e2e_bulk_edit_and_delete() {
    let Some(ctx) = setup_context() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let mut cleanup = CleanupGuard::new(ctx.clone());
    let suffix = unique_suffix();

    let project_name = format!("e2e-cli-bulk-edit-{}", suffix);
    let task_one_content = format!("E2E CLI bulk edit task one {}", suffix);
    let task_two_content = format!("E2E CLI bulk edit task two {}", suffix);

    let project = ctx.run_json(&["--json", "projects", "add", &project_name]);
    let project_id = required_str(&project, "id");
    cleanup.track_project(project_id);

    let task_one = ctx.run_json(&[
        "--json",
        "add",
        &task_one_content,
        "--project",
        &project_name,
    ]);
    let task_one_id = required_str(&task_one, "id");
    cleanup.track_task(task_one_id.clone());

    let task_two = ctx.run_json(&[
        "--json",
        "add",
        &task_two_content,
        "--project",
        &project_name,
    ]);
    let task_two_id = required_str(&task_two, "id");
    cleanup.track_task(task_two_id.clone());

    ctx.run_json(&[
        "--json",
        "edit",
        &task_one_id,
        "--priority",
        "1",
        "--due",
        "tomorrow",
        "--description",
        "updated in bulk edit e2e",
    ]);

    let edited = ctx.run_json(&["--json", "--sync", "show", &task_one_id]);
    assert_eq!(
        edited.get("description").and_then(Value::as_str),
        Some("updated in bulk edit e2e")
    );
    assert_eq!(edited.get("priority").and_then(Value::as_u64), Some(1));
    assert!(
        edited.get("due").and_then(Value::as_object).is_some(),
        "edited task should have a due date"
    );

    let before_delete = ctx.run_json(&["--json", "list", "--project", &project_name, "--all"]);
    assert!(
        list_has_task(&before_delete, &task_one_id),
        "first task should exist before bulk delete"
    );
    assert!(
        list_has_task(&before_delete, &task_two_id),
        "second task should exist before bulk delete"
    );

    let delete_result = ctx.run_json_owned(vec![
        "--json".to_string(),
        "delete".to_string(),
        task_one_id.clone(),
        task_two_id.clone(),
        "--force".to_string(),
    ]);
    assert_eq!(
        delete_result.get("total_deleted").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        delete_result.get("total_failed").and_then(Value::as_u64),
        Some(0)
    );

    ctx.run_json(&["--json", "sync"]);
    let after_delete = ctx.run_json(&["--json", "list", "--project", &project_name, "--all"]);
    assert!(
        !list_has_task(&after_delete, &task_one_id),
        "deleted task should be absent from active list"
    );
    assert!(
        !list_has_task(&after_delete, &task_two_id),
        "deleted task should be absent from active list"
    );
}

#[test]
#[serial]
fn test_cli_e2e_label_filtering_after_move() {
    let Some(ctx) = setup_context() else {
        eprintln!("Skipping e2e test: no API token found");
        return;
    };

    let mut cleanup = CleanupGuard::new(ctx.clone());
    let suffix = unique_suffix();

    let source_project_name = format!("e2e-cli-label-src-{}", suffix);
    let target_project_name = format!("e2e-cli-label-dst-{}", suffix);
    let label_name = format!("e2e-cli-label-{}", suffix);
    let task_content = format!("E2E CLI label task {}", suffix);

    let source_project = ctx.run_json(&["--json", "projects", "add", &source_project_name]);
    let source_project_id = required_str(&source_project, "id");
    cleanup.track_project(source_project_id);

    let target_project = ctx.run_json(&["--json", "projects", "add", &target_project_name]);
    let target_project_id = required_str(&target_project, "id");
    cleanup.track_project(target_project_id.clone());

    let label = ctx.run_json(&["--json", "labels", "add", &label_name]);
    let label_id = required_str(&label, "id");
    cleanup.track_label(label_id);

    let task = ctx.run_json(&[
        "--json",
        "add",
        &task_content,
        "--project",
        &source_project_name,
        "--label",
        &label_name,
    ]);
    let task_id = required_str(&task, "id");
    cleanup.track_task(task_id.clone());

    let labeled_in_source = ctx.run_json(&[
        "--json",
        "list",
        "--project",
        &source_project_name,
        "--label",
        &label_name,
        "--all",
    ]);
    assert!(
        list_has_task(&labeled_in_source, &task_id),
        "task should be listed in source project with label filter"
    );

    ctx.run_json(&[
        "--json",
        "edit",
        &task_id,
        "--project",
        &target_project_name,
        "--remove-label",
        &label_name,
    ]);

    let moved = ctx.run_json(&["--json", "show", &task_id]);
    assert_eq!(
        moved.get("project_id").and_then(Value::as_str),
        Some(target_project_id.as_str())
    );
    let labels = moved
        .get("labels")
        .and_then(Value::as_array)
        .expect("show output missing labels array");
    assert!(
        !labels
            .iter()
            .any(|l| l.as_str().is_some_and(|name| name == label_name)),
        "task should no longer have the removed label"
    );

    let labeled_in_source_after = ctx.run_json(&[
        "--json",
        "list",
        "--project",
        &source_project_name,
        "--label",
        &label_name,
        "--all",
    ]);
    assert!(
        !list_has_task(&labeled_in_source_after, &task_id),
        "task should not remain in source project label-filtered list"
    );

    let target_tasks =
        ctx.run_json(&["--json", "list", "--project", &target_project_name, "--all"]);
    assert!(
        list_has_task(&target_tasks, &task_id),
        "task should be listed in target project after move"
    );
}
