use clap::Parser;
use std::process::ExitCode;

mod cli;
mod commands;
mod output;

use cli::{Cli, Commands, ProjectsCommands};
use commands::{CommandContext, CommandError};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(&cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if cli.json {
                let error_json = serde_json::json!({
                    "error": {
                        "code": error_code(&e),
                        "message": e.to_string(),
                    }
                });
                eprintln!("{}", serde_json::to_string_pretty(&error_json).unwrap());
            } else {
                eprintln!("Error: {e}");
            }
            error_exit_code(&e)
        }
    }
}

async fn run(cli: &Cli) -> commands::Result<()> {
    let ctx = CommandContext::from_cli(cli);

    // Get token from CLI or environment
    let token = cli
        .token
        .clone()
        .ok_or_else(|| CommandError::Config("No API token provided. Use --token or set TODOIST_TOKEN environment variable.".to_string()))?;

    match &cli.command {
        Some(Commands::List {
            filter,
            project,
            label,
            priority,
            section,
            overdue,
            no_due,
            limit,
            all,
            cursor,
            sort,
            reverse,
        }) => {
            let opts = commands::list::ListOptions {
                filter: filter.clone(),
                project: project.clone(),
                label: label.clone(),
                priority: *priority,
                section: section.clone(),
                overdue: *overdue,
                no_due: *no_due,
                limit: *limit,
                all: *all,
                cursor: cursor.clone(),
                sort: sort.clone(),
                reverse: *reverse,
            };
            commands::list::execute(&ctx, &opts, &token).await
        }

        Some(Commands::Add {
            content,
            project,
            priority,
            due,
            label,
            section,
            parent,
            description,
        }) => {
            let opts = commands::add::AddOptions {
                content: content.clone(),
                project: project.clone(),
                priority: *priority,
                due: due.clone(),
                labels: label.clone(),
                section: section.clone(),
                parent: parent.clone(),
                description: description.clone(),
            };
            commands::add::execute(&ctx, &opts, &token).await
        }

        Some(Commands::Show {
            task_id,
            comments,
            reminders,
        }) => {
            let opts = commands::show::ShowOptions {
                task_id: task_id.clone(),
                comments: *comments,
                reminders: *reminders,
            };
            commands::show::execute(&ctx, &opts, &token).await
        }

        Some(Commands::Edit {
            task_id,
            content,
            project,
            priority,
            due,
            no_due,
            label,
            add_label,
            remove_label,
            section,
            description,
        }) => {
            let opts = commands::edit::EditOptions {
                task_id: task_id.clone(),
                content: content.clone(),
                project: project.clone(),
                priority: *priority,
                due: due.clone(),
                no_due: *no_due,
                labels: label.clone(),
                add_label: add_label.clone(),
                remove_label: remove_label.clone(),
                section: section.clone(),
                description: description.clone(),
            };
            commands::edit::execute(&ctx, &opts, &token).await
        }

        Some(Commands::Done {
            task_ids,
            all_occurrences,
            force,
        }) => {
            let opts = commands::done::DoneOptions {
                task_ids: task_ids.clone(),
                all_occurrences: *all_occurrences,
                force: *force,
            };
            commands::done::execute(&ctx, &opts, &token).await
        }

        Some(Commands::Reopen { task_ids, force }) => {
            let opts = commands::reopen::ReopenOptions {
                task_ids: task_ids.clone(),
                force: *force,
            };
            commands::reopen::execute(&ctx, &opts, &token).await
        }

        Some(Commands::Delete { task_ids, force }) => {
            let opts = commands::delete::DeleteOptions {
                task_ids: task_ids.clone(),
                force: *force,
            };
            commands::delete::execute(&ctx, &opts, &token).await
        }

        Some(Commands::Today {
            no_overdue,
            include_upcoming,
        }) => {
            let opts = commands::today::TodayOptions {
                include_overdue: !no_overdue,
                include_upcoming: *include_upcoming,
            };
            commands::today::execute(&ctx, &opts, &token).await
        }

        Some(Commands::Quick {
            text,
            auto_reminder,
            note,
        }) => {
            let opts = commands::quick::QuickOptions {
                text: text.clone(),
                auto_reminder: *auto_reminder,
                note: note.clone(),
            };
            commands::quick::execute(&ctx, &opts, &token).await
        }

        Some(Commands::Projects { command }) => {
            // Default to List if no subcommand provided
            let (tree, archived, limit) = match command {
                Some(ProjectsCommands::List { tree, archived, limit }) => (*tree, *archived, *limit),
                None => (false, false, None),
                _ => {
                    // Other subcommands not yet implemented
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "status": "not_implemented",
                                "command": "projects subcommand"
                            })
                        );
                    } else if !cli.quiet {
                        println!("Projects subcommand not yet implemented");
                    }
                    return Ok(());
                }
            };

            let opts = commands::projects::ProjectsListOptions {
                tree,
                archived,
                limit,
            };
            commands::projects::execute(&ctx, &opts, &token).await
        }

        Some(cmd) => {
            // Other commands not yet implemented
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "not_implemented",
                        "command": format!("{:?}", cmd)
                    })
                );
            } else if !cli.quiet {
                println!("Command not yet implemented: {:?}", cmd);
            }
            Ok(())
        }

        None => {
            if !cli.quiet {
                println!("td - Todoist CLI");
                println!("Use --help for usage information");
            }
            Ok(())
        }
    }
}

/// Returns the error code string for JSON output.
fn error_code(e: &CommandError) -> &'static str {
    match e {
        CommandError::Sync(_) => "SYNC_ERROR",
        CommandError::CacheStore(_) => "CACHE_ERROR",
        CommandError::Filter(_) => "FILTER_ERROR",
        CommandError::Api(_) => "API_ERROR",
        CommandError::Config(_) => "CONFIG_ERROR",
        CommandError::Io(_) => "IO_ERROR",
        CommandError::Json(_) => "JSON_ERROR",
    }
}

/// Returns the exit code for an error.
fn error_exit_code(e: &CommandError) -> ExitCode {
    match e {
        CommandError::Config(_) => ExitCode::from(5),
        CommandError::Filter(_) => ExitCode::from(1),
        CommandError::Api(_) => ExitCode::from(2),
        CommandError::Sync(todoist_cache::SyncError::Api(_)) => ExitCode::from(2),
        CommandError::Sync(todoist_cache::SyncError::Cache(_)) => ExitCode::from(5),
        CommandError::CacheStore(_) => ExitCode::from(5),
        CommandError::Io(_) => ExitCode::from(3),
        CommandError::Json(_) => ExitCode::from(1),
    }
}
