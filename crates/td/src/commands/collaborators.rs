//! Collaborators command implementation.
//!
//! Lists collaborators for a shared project.

use todoist_api_rs::client::TodoistClient;
use todoist_cache_rs::{CacheStore, SyncManager};

use super::{CommandContext, CommandError, Result};

/// Options for the collaborators command.
#[derive(Debug)]
pub struct CollaboratorsOptions {
    /// Project name or ID.
    pub project: String,
}

/// Executes the collaborators command.
pub async fn execute(ctx: &CommandContext, opts: &CollaboratorsOptions, token: &str) -> Result<()> {
    let client = TodoistClient::new(token)?;
    let store = CacheStore::new()?;
    let mut manager = SyncManager::new(client, store)?;

    if ctx.sync_first {
        if ctx.verbose {
            eprintln!("Syncing with Todoist...");
        }
        manager.sync().await?;
    }

    let cache = manager.cache();

    // Resolve project
    let project = cache
        .projects
        .iter()
        .find(|p| {
            !p.is_deleted
                && (p.name.to_lowercase() == opts.project.to_lowercase() || p.id == opts.project)
        })
        .ok_or_else(|| CommandError::Config(format!("Project not found: {}", opts.project)))?;

    // Get collaborator states for this project
    let active_states: Vec<_> = cache
        .collaborator_states
        .iter()
        .filter(|s| s.project_id == project.id)
        .collect();

    if active_states.is_empty() {
        if ctx.json_output {
            println!("{{\"collaborators\": []}}");
        } else if !ctx.quiet {
            println!(
                "No collaborators found for project \"{}\" — it may be a personal project.",
                project.name
            );
        }
        return Ok(());
    }

    if ctx.json_output {
        let collabs: Vec<serde_json::Value> = active_states
            .iter()
            .filter_map(|state| {
                cache
                    .collaborators
                    .iter()
                    .find(|c| c.id == state.user_id)
                    .map(|c| {
                        serde_json::json!({
                            "id": c.id,
                            "name": c.full_name,
                            "email": c.email,
                            "status": state.state,
                        })
                    })
            })
            .collect();
        let output = serde_json::json!({ "collaborators": collabs });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if !ctx.quiet {
        // Table output
        println!("{:<25} {:<30} Status", "Name", "Email");
        for state in &active_states {
            if let Some(collab) = cache.collaborators.iter().find(|c| c.id == state.user_id) {
                let name = collab.full_name.as_deref().unwrap_or("(unknown)");
                let email = collab.email.as_deref().unwrap_or("");
                let is_me = cache.user.as_ref().is_some_and(|u| u.id == collab.id);
                let status = if is_me {
                    format!("{} (you)", state.state)
                } else {
                    state.state.clone()
                };
                println!("{:<25} {:<30} {}", name, email, status);
            }
        }
    }

    Ok(())
}
