//! Command dispatch module for routing CLI commands to their handlers.
//!
//! This module provides trait-based dispatch for CLI commands, replacing
//! the large match statement in main.rs with a more maintainable structure.

use crate::cli::{
    Cli, Commands, CommentsCommands, ConfigCommands, FiltersCommands, LabelsCommands,
    ProjectsCommands, RemindersCommands, SectionsCommands,
};
use crate::commands::{self, CommandContext, CommandError, Result};

/// Trait for commands that can be executed without authentication.
pub trait NoAuthCommand {
    /// Execute the command without requiring an API token.
    fn execute(&self, ctx: &CommandContext) -> Result<()>;
}

/// Trait for commands that require authentication.
#[allow(async_fn_in_trait)]
pub trait AuthCommand {
    /// Execute the command with the provided API token.
    async fn execute(&self, ctx: &CommandContext, token: &str) -> Result<()>;
}

/// Commands that don't require authentication.
pub enum NoAuthDispatch<'a> {
    Config(&'a Option<ConfigCommands>),
    Completions(&'a crate::cli::Shell),
    Help,
}

impl<'a> NoAuthDispatch<'a> {
    /// Try to create a no-auth dispatch from the CLI command.
    /// Returns None if the command requires authentication.
    pub fn try_from_cli(cli: &'a Cli) -> Option<Self> {
        match &cli.command {
            Some(Commands::Config { command }) => Some(Self::Config(command)),
            Some(Commands::Completions { shell }) => Some(Self::Completions(shell)),
            None => Some(Self::Help),
            _ => None,
        }
    }
}

impl NoAuthCommand for NoAuthDispatch<'_> {
    fn execute(&self, ctx: &CommandContext) -> Result<()> {
        match self {
            Self::Config(command) => dispatch_config(ctx, command),
            Self::Completions(shell) => {
                commands::completions::execute(shell).map_err(CommandError::Io)
            }
            Self::Help => {
                if !ctx.quiet {
                    println!("td - Todoist CLI");
                    println!("Use --help for usage information");
                }
                Ok(())
            }
        }
    }
}

/// Dispatch config subcommands.
fn dispatch_config(ctx: &CommandContext, command: &Option<ConfigCommands>) -> Result<()> {
    // Config commands are sync, but edit is async - we need to handle this specially
    // For now, we'll handle the sync ones here and let async be handled separately
    match command {
        Some(ConfigCommands::Show) | None => commands::config::execute_show(ctx),
        Some(ConfigCommands::Set { key, value }) => {
            let opts = commands::config::ConfigSetOptions {
                key: key.clone(),
                value: value.clone(),
            };
            commands::config::execute_set(ctx, &opts)
        }
        Some(ConfigCommands::Path) => commands::config::execute_path(ctx),
        Some(ConfigCommands::Edit) => {
            // Edit is async but called from sync context - need special handling
            // This is handled in the main dispatch function
            Err(CommandError::Config("edit requires async context".into()))
        }
    }
}

/// Commands that require authentication.
pub enum AuthDispatch<'a> {
    List {
        filter: &'a Option<String>,
        project: &'a Option<String>,
        label: &'a Option<String>,
        priority: Option<u8>,
        section: &'a Option<String>,
        overdue: bool,
        no_due: bool,
        limit: u32,
        all: bool,
        cursor: &'a Option<String>,
        sort: &'a Option<crate::cli::SortField>,
        reverse: bool,
    },
    Add {
        content: &'a str,
        project: &'a Option<String>,
        priority: Option<u8>,
        due: &'a Option<String>,
        labels: &'a [String],
        section: &'a Option<String>,
        parent: &'a Option<String>,
        description: &'a Option<String>,
    },
    Show {
        task_id: &'a str,
        comments: bool,
        reminders: bool,
    },
    Edit {
        task_id: &'a str,
        content: &'a Option<String>,
        project: &'a Option<String>,
        priority: Option<u8>,
        due: &'a Option<String>,
        no_due: bool,
        labels: &'a [String],
        add_label: &'a Option<String>,
        remove_label: &'a Option<String>,
        section: &'a Option<String>,
        description: &'a Option<String>,
    },
    Done {
        task_ids: &'a [String],
        all_occurrences: bool,
        force: bool,
    },
    Reopen {
        task_ids: &'a [String],
        force: bool,
    },
    Delete {
        task_ids: &'a [String],
        force: bool,
    },
    Today {
        include_overdue: bool,
        include_upcoming: Option<u32>,
    },
    Quick {
        text: &'a str,
        auto_reminder: bool,
        note: &'a Option<String>,
    },
    Sync {
        full: bool,
    },
    ConfigEdit,
    Projects(&'a Option<ProjectsCommands>),
    Labels(&'a Option<LabelsCommands>),
    Sections {
        project: &'a Option<String>,
        command: &'a Option<SectionsCommands>,
    },
    Comments {
        task: &'a Option<String>,
        project: &'a Option<String>,
        command: &'a Option<CommentsCommands>,
    },
    Reminders {
        task: &'a Option<String>,
        command: &'a Option<RemindersCommands>,
    },
    Filters(&'a Option<FiltersCommands>),
}

impl<'a> AuthDispatch<'a> {
    /// Create an auth dispatch from the CLI command.
    /// Panics if the command doesn't require authentication (use NoAuthDispatch first).
    pub fn from_cli(cli: &'a Cli) -> Option<Self> {
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
            }) => Some(Self::List {
                filter,
                project,
                label,
                priority: *priority,
                section,
                overdue: *overdue,
                no_due: *no_due,
                limit: *limit,
                all: *all,
                cursor,
                sort,
                reverse: *reverse,
            }),
            Some(Commands::Add {
                content,
                project,
                priority,
                due,
                label,
                section,
                parent,
                description,
            }) => Some(Self::Add {
                content,
                project,
                priority: *priority,
                due,
                labels: label,
                section,
                parent,
                description,
            }),
            Some(Commands::Show {
                task_id,
                comments,
                reminders,
            }) => Some(Self::Show {
                task_id,
                comments: *comments,
                reminders: *reminders,
            }),
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
            }) => Some(Self::Edit {
                task_id,
                content,
                project,
                priority: *priority,
                due,
                no_due: *no_due,
                labels: label,
                add_label,
                remove_label,
                section,
                description,
            }),
            Some(Commands::Done {
                task_ids,
                all_occurrences,
                force,
            }) => Some(Self::Done {
                task_ids,
                all_occurrences: *all_occurrences,
                force: *force,
            }),
            Some(Commands::Reopen { task_ids, force }) => Some(Self::Reopen {
                task_ids,
                force: *force,
            }),
            Some(Commands::Delete { task_ids, force }) => Some(Self::Delete {
                task_ids,
                force: *force,
            }),
            Some(Commands::Today {
                no_overdue,
                include_upcoming,
            }) => Some(Self::Today {
                include_overdue: !no_overdue,
                include_upcoming: *include_upcoming,
            }),
            Some(Commands::Quick {
                text,
                auto_reminder,
                note,
            }) => Some(Self::Quick {
                text,
                auto_reminder: *auto_reminder,
                note,
            }),
            Some(Commands::Sync { full }) => Some(Self::Sync { full: *full }),
            Some(Commands::Config {
                command: Some(ConfigCommands::Edit),
            }) => Some(Self::ConfigEdit),
            Some(Commands::Projects { command }) => Some(Self::Projects(command)),
            Some(Commands::Labels { command }) => Some(Self::Labels(command)),
            Some(Commands::Sections { project, command }) => {
                Some(Self::Sections { project, command })
            }
            Some(Commands::Comments {
                task,
                project,
                command,
            }) => Some(Self::Comments {
                task,
                project,
                command,
            }),
            Some(Commands::Reminders { task, command }) => Some(Self::Reminders { task, command }),
            Some(Commands::Filters { command }) => Some(Self::Filters(command)),
            // Already handled by NoAuthDispatch
            Some(Commands::Config { .. }) | Some(Commands::Completions { .. }) | None => None,
        }
    }
}

impl AuthCommand for AuthDispatch<'_> {
    async fn execute(&self, ctx: &CommandContext, token: &str) -> Result<()> {
        match self {
            Self::List {
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
            } => {
                let opts = commands::list::ListOptions {
                    filter: (*filter).clone(),
                    project: (*project).clone(),
                    label: (*label).clone(),
                    priority: *priority,
                    section: (*section).clone(),
                    overdue: *overdue,
                    no_due: *no_due,
                    limit: *limit,
                    all: *all,
                    cursor: (*cursor).clone(),
                    sort: (*sort).clone(),
                    reverse: *reverse,
                };
                commands::list::execute(ctx, &opts, token).await
            }

            Self::Add {
                content,
                project,
                priority,
                due,
                labels,
                section,
                parent,
                description,
            } => {
                let opts = commands::add::AddOptions {
                    content: (*content).to_string(),
                    project: (*project).clone(),
                    priority: *priority,
                    due: (*due).clone(),
                    labels: (*labels).to_vec(),
                    section: (*section).clone(),
                    parent: (*parent).clone(),
                    description: (*description).clone(),
                };
                commands::add::execute(ctx, &opts, token).await
            }

            Self::Show {
                task_id,
                comments,
                reminders,
            } => {
                let opts = commands::show::ShowOptions {
                    task_id: (*task_id).to_string(),
                    comments: *comments,
                    reminders: *reminders,
                };
                commands::show::execute(ctx, &opts, token).await
            }

            Self::Edit {
                task_id,
                content,
                project,
                priority,
                due,
                no_due,
                labels,
                add_label,
                remove_label,
                section,
                description,
            } => {
                let opts = commands::edit::EditOptions {
                    task_id: (*task_id).to_string(),
                    content: (*content).clone(),
                    project: (*project).clone(),
                    priority: *priority,
                    due: (*due).clone(),
                    no_due: *no_due,
                    labels: (*labels).to_vec(),
                    add_label: (*add_label).clone(),
                    remove_label: (*remove_label).clone(),
                    section: (*section).clone(),
                    description: (*description).clone(),
                };
                commands::edit::execute(ctx, &opts, token).await
            }

            Self::Done {
                task_ids,
                all_occurrences,
                force,
            } => {
                let opts = commands::done::DoneOptions {
                    task_ids: (*task_ids).to_vec(),
                    all_occurrences: *all_occurrences,
                    force: *force,
                };
                commands::done::execute(ctx, &opts, token).await
            }

            Self::Reopen { task_ids, force } => {
                let opts = commands::reopen::ReopenOptions {
                    task_ids: (*task_ids).to_vec(),
                    force: *force,
                };
                commands::reopen::execute(ctx, &opts, token).await
            }

            Self::Delete { task_ids, force } => {
                let opts = commands::delete::DeleteOptions {
                    task_ids: (*task_ids).to_vec(),
                    force: *force,
                };
                commands::delete::execute(ctx, &opts, token).await
            }

            Self::Today {
                include_overdue,
                include_upcoming,
            } => {
                let opts = commands::today::TodayOptions {
                    include_overdue: *include_overdue,
                    include_upcoming: *include_upcoming,
                };
                commands::today::execute(ctx, &opts, token).await
            }

            Self::Quick {
                text,
                auto_reminder,
                note,
            } => {
                let opts = commands::quick::QuickOptions {
                    text: (*text).to_string(),
                    auto_reminder: *auto_reminder,
                    note: (*note).clone(),
                };
                commands::quick::execute(ctx, &opts, token).await
            }

            Self::Sync { full } => {
                let opts = commands::sync::SyncOptions { full: *full };
                commands::sync::execute(ctx, &opts, token).await
            }

            Self::ConfigEdit => commands::config::execute_edit(ctx).await,

            Self::Projects(command) => dispatch_projects(ctx, command, token).await,
            Self::Labels(command) => dispatch_labels(ctx, command, token).await,
            Self::Sections { project, command } => {
                dispatch_sections(ctx, project, command, token).await
            }
            Self::Comments {
                task,
                project,
                command,
            } => dispatch_comments(ctx, task, project, command, token).await,
            Self::Reminders { task, command } => {
                dispatch_reminders(ctx, task, command, token).await
            }
            Self::Filters(command) => dispatch_filters(ctx, command, token).await,
        }
    }
}

async fn dispatch_projects(
    ctx: &CommandContext,
    command: &Option<ProjectsCommands>,
    token: &str,
) -> Result<()> {
    match command {
        Some(ProjectsCommands::List {
            tree,
            archived,
            limit,
        }) => {
            let opts = commands::projects::ProjectsListOptions {
                tree: *tree,
                archived: *archived,
                limit: *limit,
            };
            commands::projects::execute(ctx, &opts, token).await
        }
        Some(ProjectsCommands::Add {
            name,
            color,
            parent,
            favorite,
        }) => {
            let opts = commands::projects::ProjectsAddOptions {
                name: name.clone(),
                color: color.clone(),
                parent: parent.clone(),
                favorite: *favorite,
            };
            commands::projects::execute_add(ctx, &opts, token).await
        }
        Some(ProjectsCommands::Show {
            project_id,
            sections,
            tasks,
        }) => {
            let opts = commands::projects::ProjectsShowOptions {
                project_id: project_id.clone(),
                sections: *sections,
                tasks: *tasks,
            };
            commands::projects::execute_show(ctx, &opts, token).await
        }
        Some(ProjectsCommands::Edit {
            project_id,
            name,
            color,
            favorite,
            view_style,
        }) => {
            let opts = commands::projects::ProjectsEditOptions {
                project_id: project_id.clone(),
                name: name.clone(),
                color: color.clone(),
                favorite: *favorite,
                view_style: view_style.clone(),
            };
            commands::projects::execute_edit(ctx, &opts, token).await
        }
        Some(ProjectsCommands::Archive { project_id, force }) => {
            let opts = commands::projects::ProjectsArchiveOptions {
                project_id: project_id.clone(),
                force: *force,
            };
            commands::projects::execute_archive(ctx, &opts, token).await
        }
        Some(ProjectsCommands::Unarchive { project_id }) => {
            let opts = commands::projects::ProjectsUnarchiveOptions {
                project_id: project_id.clone(),
            };
            commands::projects::execute_unarchive(ctx, &opts, token).await
        }
        Some(ProjectsCommands::Delete { project_id, force }) => {
            let opts = commands::projects::ProjectsDeleteOptions {
                project_id: project_id.clone(),
                force: *force,
            };
            commands::projects::execute_delete(ctx, &opts, token).await
        }
        None => {
            let opts = commands::projects::ProjectsListOptions {
                tree: false,
                archived: false,
                limit: None,
            };
            commands::projects::execute(ctx, &opts, token).await
        }
    }
}

async fn dispatch_labels(
    ctx: &CommandContext,
    command: &Option<LabelsCommands>,
    token: &str,
) -> Result<()> {
    match command {
        Some(LabelsCommands::List) | None => {
            let opts = commands::labels::LabelsListOptions::default();
            commands::labels::execute(ctx, &opts, token).await
        }
        Some(LabelsCommands::Add {
            name,
            color,
            favorite,
        }) => {
            let opts = commands::labels::LabelsAddOptions {
                name: name.clone(),
                color: color.clone(),
                favorite: *favorite,
            };
            commands::labels::execute_add(ctx, &opts, token).await
        }
        Some(LabelsCommands::Edit {
            label_id,
            name,
            color,
            favorite,
        }) => {
            let opts = commands::labels::LabelsEditOptions {
                label_id: label_id.clone(),
                name: name.clone(),
                color: color.clone(),
                favorite: *favorite,
            };
            commands::labels::execute_edit(ctx, &opts, token).await
        }
        Some(LabelsCommands::Delete { label_id, force }) => {
            let opts = commands::labels::LabelsDeleteOptions {
                label_id: label_id.clone(),
                force: *force,
            };
            commands::labels::execute_delete(ctx, &opts, token).await
        }
    }
}

async fn dispatch_sections(
    ctx: &CommandContext,
    project: &Option<String>,
    command: &Option<SectionsCommands>,
    token: &str,
) -> Result<()> {
    match command {
        Some(SectionsCommands::List) | None => {
            let opts = commands::sections::SectionsListOptions {
                project: project.clone(),
                limit: None,
            };
            commands::sections::execute(ctx, &opts, token).await
        }
        Some(SectionsCommands::Add {
            name,
            project: proj,
        }) => {
            let opts = commands::sections::SectionsAddOptions {
                name: name.clone(),
                project: proj.clone(),
            };
            commands::sections::execute_add(ctx, &opts, token).await
        }
        Some(SectionsCommands::Edit { section_id, name }) => {
            let opts = commands::sections::SectionsEditOptions {
                section_id: section_id.clone(),
                name: name.clone(),
            };
            commands::sections::execute_edit(ctx, &opts, token).await
        }
        Some(SectionsCommands::Delete { section_id, force }) => {
            let opts = commands::sections::SectionsDeleteOptions {
                section_id: section_id.clone(),
                force: *force,
            };
            commands::sections::execute_delete(ctx, &opts, token).await
        }
    }
}

async fn dispatch_comments(
    ctx: &CommandContext,
    task: &Option<String>,
    project: &Option<String>,
    command: &Option<CommentsCommands>,
    token: &str,
) -> Result<()> {
    match command {
        Some(CommentsCommands::List) | None => {
            let opts = commands::comments::CommentsListOptions {
                task: task.clone(),
                project: project.clone(),
            };
            commands::comments::execute(ctx, &opts, token).await
        }
        Some(CommentsCommands::Add {
            text,
            task: add_task,
            project: add_project,
        }) => {
            let effective_task = add_task.clone().or_else(|| task.clone());
            let effective_project = add_project.clone().or_else(|| project.clone());
            let opts = commands::comments::CommentsAddOptions {
                content: text.clone(),
                task: effective_task,
                project: effective_project,
            };
            commands::comments::execute_add(ctx, &opts, token).await
        }
        Some(CommentsCommands::Edit { comment_id, text }) => {
            let opts = commands::comments::CommentsEditOptions {
                comment_id: comment_id.clone(),
                content: text.clone(),
            };
            commands::comments::execute_edit(ctx, &opts, token).await
        }
        Some(CommentsCommands::Delete { comment_id, force }) => {
            let opts = commands::comments::CommentsDeleteOptions {
                comment_id: comment_id.clone(),
                force: *force,
            };
            commands::comments::execute_delete(ctx, &opts, token).await
        }
        Some(CommentsCommands::Attach { .. }) | Some(CommentsCommands::Download { .. }) => {
            if ctx.json_output {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "not_implemented",
                        "command": format!("{:?}", command)
                    })
                );
            } else if !ctx.quiet {
                println!("Comments subcommand not yet implemented: {:?}", command);
            }
            Ok(())
        }
    }
}

async fn dispatch_reminders(
    ctx: &CommandContext,
    task: &Option<String>,
    command: &Option<RemindersCommands>,
    token: &str,
) -> Result<()> {
    match command {
        Some(RemindersCommands::List) | None => {
            let opts = commands::reminders::RemindersListOptions { task: task.clone() };
            commands::reminders::execute(ctx, &opts, token).await
        }
        Some(RemindersCommands::Add {
            task: add_task,
            due,
            offset,
        }) => {
            let opts = commands::reminders::RemindersAddOptions {
                task: add_task.clone(),
                due: due.clone(),
                offset: *offset,
            };
            commands::reminders::execute_add(ctx, &opts, token).await
        }
        Some(RemindersCommands::Delete { reminder_id, force }) => {
            let opts = commands::reminders::RemindersDeleteOptions {
                reminder_id: reminder_id.clone(),
                force: *force,
            };
            commands::reminders::execute_delete(ctx, &opts, token).await
        }
    }
}

async fn dispatch_filters(
    ctx: &CommandContext,
    command: &Option<FiltersCommands>,
    token: &str,
) -> Result<()> {
    match command {
        Some(FiltersCommands::List) | None => {
            let opts = commands::filters::FiltersListOptions::default();
            commands::filters::execute(ctx, &opts, token).await
        }
        Some(FiltersCommands::Add {
            name,
            query,
            color,
            favorite,
        }) => {
            let opts = commands::filters::FiltersAddOptions {
                name: name.clone(),
                query: query.clone(),
                color: color.clone(),
                favorite: *favorite,
            };
            commands::filters::execute_add(ctx, &opts, token).await
        }
        Some(FiltersCommands::Show { filter_id }) => {
            let opts = commands::filters::FiltersShowOptions {
                filter_id: filter_id.clone(),
            };
            commands::filters::execute_show(ctx, &opts, token).await
        }
        Some(FiltersCommands::Edit {
            filter_id,
            name,
            query,
            color,
            favorite,
        }) => {
            let opts = commands::filters::FiltersEditOptions {
                filter_id: filter_id.clone(),
                name: name.clone(),
                query: query.clone(),
                color: color.clone(),
                favorite: *favorite,
            };
            commands::filters::execute_edit(ctx, &opts, token).await
        }
        Some(FiltersCommands::Delete { filter_id, force }) => {
            let opts = commands::filters::FiltersDeleteOptions {
                filter_id: filter_id.clone(),
                force: *force,
            };
            commands::filters::execute_delete(ctx, &opts, token).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    #[test]
    fn test_no_auth_dispatch_config_show() {
        let cli = Cli::parse_from(["td", "config", "show"]);
        let dispatch = NoAuthDispatch::try_from_cli(&cli);
        assert!(matches!(dispatch, Some(NoAuthDispatch::Config(_))));
    }

    #[test]
    fn test_no_auth_dispatch_completions() {
        let cli = Cli::parse_from(["td", "completions", "zsh"]);
        let dispatch = NoAuthDispatch::try_from_cli(&cli);
        assert!(matches!(dispatch, Some(NoAuthDispatch::Completions(_))));
    }

    #[test]
    fn test_no_auth_dispatch_help() {
        let cli = Cli::parse_from(["td"]);
        let dispatch = NoAuthDispatch::try_from_cli(&cli);
        assert!(matches!(dispatch, Some(NoAuthDispatch::Help)));
    }

    #[test]
    fn test_no_auth_dispatch_returns_none_for_list() {
        let cli = Cli::parse_from(["td", "list"]);
        let dispatch = NoAuthDispatch::try_from_cli(&cli);
        assert!(dispatch.is_none());
    }

    #[test]
    fn test_auth_dispatch_list() {
        let cli = Cli::parse_from(["td", "list", "--filter", "today"]);
        let dispatch = AuthDispatch::from_cli(&cli);
        assert!(matches!(dispatch, Some(AuthDispatch::List { .. })));
    }

    #[test]
    fn test_auth_dispatch_add() {
        let cli = Cli::parse_from(["td", "add", "Test task"]);
        let dispatch = AuthDispatch::from_cli(&cli);
        assert!(matches!(dispatch, Some(AuthDispatch::Add { .. })));
    }

    #[test]
    fn test_auth_dispatch_projects() {
        let cli = Cli::parse_from(["td", "projects", "list"]);
        let dispatch = AuthDispatch::from_cli(&cli);
        assert!(matches!(dispatch, Some(AuthDispatch::Projects(_))));
    }

    #[test]
    fn test_auth_dispatch_returns_none_for_config() {
        let cli = Cli::parse_from(["td", "config", "show"]);
        let dispatch = AuthDispatch::from_cli(&cli);
        assert!(dispatch.is_none());
    }
}
