use clap::Parser;
use std::process::ExitCode;

mod cli;
mod commands;
mod output;

use cli::{Cli, Commands, CommentsCommands, LabelsCommands, ProjectsCommands, SectionsCommands};
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
            match command {
                Some(ProjectsCommands::List { tree, archived, limit }) => {
                    let opts = commands::projects::ProjectsListOptions {
                        tree: *tree,
                        archived: *archived,
                        limit: *limit,
                    };
                    commands::projects::execute(&ctx, &opts, &token).await
                }
                Some(ProjectsCommands::Add { name, color, parent, favorite }) => {
                    let opts = commands::projects::ProjectsAddOptions {
                        name: name.clone(),
                        color: color.clone(),
                        parent: parent.clone(),
                        favorite: *favorite,
                    };
                    commands::projects::execute_add(&ctx, &opts, &token).await
                }
                Some(ProjectsCommands::Show { project_id, sections, tasks }) => {
                    let opts = commands::projects::ProjectsShowOptions {
                        project_id: project_id.clone(),
                        sections: *sections,
                        tasks: *tasks,
                    };
                    commands::projects::execute_show(&ctx, &opts, &token).await
                }
                Some(ProjectsCommands::Edit { project_id, name, color, favorite, view_style }) => {
                    let opts = commands::projects::ProjectsEditOptions {
                        project_id: project_id.clone(),
                        name: name.clone(),
                        color: color.clone(),
                        favorite: *favorite,
                        view_style: view_style.clone(),
                    };
                    commands::projects::execute_edit(&ctx, &opts, &token).await
                }
                Some(ProjectsCommands::Archive { project_id, force }) => {
                    let opts = commands::projects::ProjectsArchiveOptions {
                        project_id: project_id.clone(),
                        force: *force,
                    };
                    commands::projects::execute_archive(&ctx, &opts, &token).await
                }
                Some(ProjectsCommands::Unarchive { project_id }) => {
                    let opts = commands::projects::ProjectsUnarchiveOptions {
                        project_id: project_id.clone(),
                    };
                    commands::projects::execute_unarchive(&ctx, &opts, &token).await
                }
                Some(ProjectsCommands::Delete { project_id, force }) => {
                    let opts = commands::projects::ProjectsDeleteOptions {
                        project_id: project_id.clone(),
                        force: *force,
                    };
                    commands::projects::execute_delete(&ctx, &opts, &token).await
                }
                None => {
                    // Default to List if no subcommand provided
                    let opts = commands::projects::ProjectsListOptions {
                        tree: false,
                        archived: false,
                        limit: None,
                    };
                    commands::projects::execute(&ctx, &opts, &token).await
                }
            }
        }

        Some(Commands::Labels { command }) => {
            match command {
                Some(LabelsCommands::List) => {
                    let opts = commands::labels::LabelsListOptions::default();
                    commands::labels::execute(&ctx, &opts, &token).await
                }
                Some(LabelsCommands::Add { name, color, favorite }) => {
                    let opts = commands::labels::LabelsAddOptions {
                        name: name.clone(),
                        color: color.clone(),
                        favorite: *favorite,
                    };
                    commands::labels::execute_add(&ctx, &opts, &token).await
                }
                Some(LabelsCommands::Edit { label_id, name, color, favorite }) => {
                    let opts = commands::labels::LabelsEditOptions {
                        label_id: label_id.clone(),
                        name: name.clone(),
                        color: color.clone(),
                        favorite: *favorite,
                    };
                    commands::labels::execute_edit(&ctx, &opts, &token).await
                }
                Some(LabelsCommands::Delete { label_id, force }) => {
                    let opts = commands::labels::LabelsDeleteOptions {
                        label_id: label_id.clone(),
                        force: *force,
                    };
                    commands::labels::execute_delete(&ctx, &opts, &token).await
                }
                None => {
                    // Default to List if no subcommand provided
                    let opts = commands::labels::LabelsListOptions::default();
                    commands::labels::execute(&ctx, &opts, &token).await
                }
            }
        }

        Some(Commands::Sections { project, command }) => {
            match command {
                Some(SectionsCommands::List) => {
                    let opts = commands::sections::SectionsListOptions {
                        project: project.clone(),
                        limit: None,
                    };
                    commands::sections::execute(&ctx, &opts, &token).await
                }
                Some(SectionsCommands::Add { name, project: proj }) => {
                    let opts = commands::sections::SectionsAddOptions {
                        name: name.clone(),
                        project: proj.clone(),
                    };
                    commands::sections::execute_add(&ctx, &opts, &token).await
                }
                Some(SectionsCommands::Edit { section_id, name }) => {
                    let opts = commands::sections::SectionsEditOptions {
                        section_id: section_id.clone(),
                        name: name.clone(),
                    };
                    commands::sections::execute_edit(&ctx, &opts, &token).await
                }
                Some(SectionsCommands::Delete { section_id, force }) => {
                    let opts = commands::sections::SectionsDeleteOptions {
                        section_id: section_id.clone(),
                        force: *force,
                    };
                    commands::sections::execute_delete(&ctx, &opts, &token).await
                }
                None => {
                    // Default to List if no subcommand provided
                    let opts = commands::sections::SectionsListOptions {
                        project: project.clone(),
                        limit: None,
                    };
                    commands::sections::execute(&ctx, &opts, &token).await
                }
            }
        }

        Some(Commands::Comments { task, project, command }) => {
            match command {
                Some(CommentsCommands::List) => {
                    let opts = commands::comments::CommentsListOptions {
                        task: task.clone(),
                        project: project.clone(),
                    };
                    commands::comments::execute(&ctx, &opts, &token).await
                }
                Some(CommentsCommands::Add { text, task: add_task, project: add_project }) => {
                    // Use the add-specific options if provided, otherwise fall back to parent options
                    let effective_task = add_task.clone().or_else(|| task.clone());
                    let effective_project = add_project.clone().or_else(|| project.clone());
                    let opts = commands::comments::CommentsAddOptions {
                        content: text.clone(),
                        task: effective_task,
                        project: effective_project,
                    };
                    commands::comments::execute_add(&ctx, &opts, &token).await
                }
                Some(CommentsCommands::Edit { comment_id, text }) => {
                    let opts = commands::comments::CommentsEditOptions {
                        comment_id: comment_id.clone(),
                        content: text.clone(),
                    };
                    commands::comments::execute_edit(&ctx, &opts, &token).await
                }
                Some(CommentsCommands::Delete { comment_id, force }) => {
                    let opts = commands::comments::CommentsDeleteOptions {
                        comment_id: comment_id.clone(),
                        force: *force,
                    };
                    commands::comments::execute_delete(&ctx, &opts, &token).await
                }
                None => {
                    // Default to List if no subcommand provided
                    let opts = commands::comments::CommentsListOptions {
                        task: task.clone(),
                        project: project.clone(),
                    };
                    commands::comments::execute(&ctx, &opts, &token).await
                }
                _ => {
                    // Other subcommands not yet implemented
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "status": "not_implemented",
                                "command": format!("{:?}", command)
                            })
                        );
                    } else if !cli.quiet {
                        println!("Comments subcommand not yet implemented: {:?}", command);
                    }
                    Ok(())
                }
            }
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
