//! CLI argument parsing using clap derive macros.
//!
//! This module defines the command-line interface for the td CLI.

use clap::{Parser, Subcommand, ValueEnum};

/// td - A Rust CLI for the Todoist API
#[derive(Parser, Debug)]
#[command(name = "td")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Verbose output (show debug information)
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Quiet mode (errors only)
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Force JSON output (auto-detected when not a TTY)
    #[arg(long, global = true)]
    pub json: bool,

    /// Disable colors in output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Override API token (default: from config/env/keyring)
    #[arg(long, global = true, env = "TODOIST_TOKEN", hide_env_values = true)]
    pub token: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List tasks
    #[command(alias = "l")]
    List {
        /// Filter expression (e.g., "today & p1")
        #[arg(short, long)]
        filter: Option<String>,

        /// Filter by project name or ID
        #[arg(short, long)]
        project: Option<String>,

        /// Filter by label
        #[arg(short, long)]
        label: Option<String>,

        /// Filter by priority (1=highest, 4=lowest)
        #[arg(short = 'P', long, value_parser = clap::value_parser!(u8).range(1..=4))]
        priority: Option<u8>,

        /// Filter by section
        #[arg(long)]
        section: Option<String>,

        /// Show only overdue tasks
        #[arg(long)]
        overdue: bool,

        /// Show only tasks without due date
        #[arg(long)]
        no_due: bool,

        /// Limit results (default: 50)
        #[arg(long, default_value = "50")]
        limit: u32,

        /// Show all tasks (no limit)
        #[arg(long)]
        all: bool,

        /// Pagination cursor for programmatic use
        #[arg(long)]
        cursor: Option<String>,

        /// Sort by field
        #[arg(long, value_enum)]
        sort: Option<SortField>,

        /// Reverse sort order
        #[arg(long)]
        reverse: bool,
    },

    /// Add a new task
    #[command(alias = "a")]
    Add {
        /// Task content/title
        content: String,

        /// Target project (default: Inbox)
        #[arg(short, long)]
        project: Option<String>,

        /// Priority level (1=highest, 4=lowest)
        #[arg(short = 'P', long, value_parser = clap::value_parser!(u8).range(1..=4))]
        priority: Option<u8>,

        /// Due date (natural language or ISO)
        #[arg(short, long)]
        due: Option<String>,

        /// Add label (repeatable)
        #[arg(short, long, action = clap::ArgAction::Append)]
        label: Vec<String>,

        /// Target section within project
        #[arg(long)]
        section: Option<String>,

        /// Parent task ID (creates subtask)
        #[arg(long)]
        parent: Option<String>,

        /// Task description/notes
        #[arg(long)]
        description: Option<String>,
    },

    /// Show task details
    #[command(alias = "s")]
    Show {
        /// Task ID
        task_id: String,

        /// Include comments
        #[arg(long)]
        comments: bool,

        /// Include reminders
        #[arg(long)]
        reminders: bool,
    },

    /// Edit a task
    #[command(alias = "e")]
    Edit {
        /// Task ID
        task_id: String,

        /// Update content
        #[arg(short, long)]
        content: Option<String>,

        /// Move to project
        #[arg(short, long)]
        project: Option<String>,

        /// Change priority
        #[arg(short = 'P', long, value_parser = clap::value_parser!(u8).range(1..=4))]
        priority: Option<u8>,

        /// Change due date
        #[arg(short, long)]
        due: Option<String>,

        /// Remove due date
        #[arg(long)]
        no_due: bool,

        /// Set labels (replaces existing)
        #[arg(short, long, action = clap::ArgAction::Append)]
        label: Vec<String>,

        /// Add label
        #[arg(long)]
        add_label: Option<String>,

        /// Remove label
        #[arg(long)]
        remove_label: Option<String>,

        /// Move to section
        #[arg(long)]
        section: Option<String>,

        /// Update description
        #[arg(long)]
        description: Option<String>,
    },

    /// Complete task(s)
    #[command(alias = "d")]
    Done {
        /// Task ID(s)
        #[arg(required = true)]
        task_ids: Vec<String>,

        /// Complete all future occurrences (recurring tasks)
        #[arg(long)]
        all_occurrences: bool,

        /// Skip confirmation for multiple tasks
        #[arg(short, long)]
        force: bool,
    },

    /// Reopen completed task(s)
    Reopen {
        /// Task ID(s)
        #[arg(required = true)]
        task_ids: Vec<String>,

        /// Skip confirmation for multiple tasks
        #[arg(short, long)]
        force: bool,
    },

    /// Delete task(s)
    #[command(alias = "rm")]
    Delete {
        /// Task ID(s)
        #[arg(required = true)]
        task_ids: Vec<String>,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// Show today's agenda
    #[command(alias = "t")]
    Today {
        /// Exclude overdue tasks (default: include overdue)
        #[arg(long)]
        no_overdue: bool,

        /// Include tasks due within N days
        #[arg(long)]
        include_upcoming: Option<u32>,
    },

    /// Quick add with natural language
    #[command(alias = "q")]
    Quick {
        /// Natural language task description
        text: String,

        /// Add default reminder when task has due time
        #[arg(long)]
        auto_reminder: bool,

        /// Add a note/comment to the created task
        #[arg(long)]
        note: Option<String>,
    },

    /// Sync local cache with Todoist
    Sync {
        /// Force full sync (ignore cache)
        #[arg(long)]
        full: bool,
    },

    /// List and manage projects
    #[command(alias = "p")]
    Projects {
        #[command(subcommand)]
        command: Option<ProjectsCommands>,
    },

    /// List and manage labels
    #[command(alias = "lb")]
    Labels {
        #[command(subcommand)]
        command: Option<LabelsCommands>,
    },

    /// List and manage sections
    Sections {
        /// Filter by project
        #[arg(short, long)]
        project: Option<String>,

        #[command(subcommand)]
        command: Option<SectionsCommands>,
    },

    /// List and manage comments
    Comments {
        /// Comments for task
        #[arg(long)]
        task: Option<String>,

        /// Comments for project
        #[arg(long)]
        project: Option<String>,

        #[command(subcommand)]
        command: Option<CommentsCommands>,
    },

    /// List and manage reminders
    Reminders {
        /// Reminders for task
        #[arg(long)]
        task: Option<String>,

        #[command(subcommand)]
        command: Option<RemindersCommands>,
    },

    /// List and manage saved filters
    #[command(alias = "f")]
    Filters {
        #[command(subcommand)]
        command: Option<FiltersCommands>,
    },

    /// View and edit configuration
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommands>,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

/// Sort fields for list command
#[derive(ValueEnum, Clone, Debug)]
pub enum SortField {
    Due,
    Priority,
    Created,
    Project,
}

/// Shell types for completions
#[derive(ValueEnum, Clone, Debug)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

/// Project subcommands
#[derive(Subcommand, Debug)]
pub enum ProjectsCommands {
    /// List all projects (default)
    List {
        /// Show projects as a tree hierarchy
        #[arg(long)]
        tree: bool,

        /// Include archived projects
        #[arg(long)]
        archived: bool,

        /// Limit results
        #[arg(long)]
        limit: Option<u32>,
    },

    /// Create a new project
    Add {
        /// Project name
        name: String,

        /// Project color
        #[arg(long)]
        color: Option<String>,

        /// Parent project ID
        #[arg(long)]
        parent: Option<String>,

        /// Mark as favorite
        #[arg(long)]
        favorite: bool,
    },

    /// Show project details
    Show {
        /// Project ID
        project_id: String,

        /// List sections in this project
        #[arg(long)]
        sections: bool,

        /// List tasks in this project
        #[arg(long)]
        tasks: bool,
    },

    /// Edit a project
    Edit {
        /// Project ID
        project_id: String,

        /// New name
        #[arg(long)]
        name: Option<String>,

        /// New color
        #[arg(long)]
        color: Option<String>,

        /// Set favorite status
        #[arg(long)]
        favorite: Option<bool>,

        /// View style (list, board)
        #[arg(long)]
        view_style: Option<String>,
    },

    /// Archive a project
    Archive {
        /// Project ID
        project_id: String,

        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Unarchive a project
    Unarchive {
        /// Project ID
        project_id: String,
    },

    /// Delete a project
    Delete {
        /// Project ID
        project_id: String,

        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
}

/// Label subcommands
#[derive(Subcommand, Debug)]
pub enum LabelsCommands {
    /// List all labels (default)
    List,

    /// Create a new label
    Add {
        /// Label name
        name: String,

        /// Label color
        #[arg(long)]
        color: Option<String>,

        /// Mark as favorite
        #[arg(long)]
        favorite: bool,
    },

    /// Edit a label
    Edit {
        /// Label ID
        label_id: String,

        /// New name
        #[arg(long)]
        name: Option<String>,

        /// New color
        #[arg(long)]
        color: Option<String>,

        /// Toggle favorite
        #[arg(long)]
        favorite: Option<bool>,
    },

    /// Delete a label
    Delete {
        /// Label ID
        label_id: String,

        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
}

/// Section subcommands
#[derive(Subcommand, Debug)]
pub enum SectionsCommands {
    /// List sections (default)
    List,

    /// Create a new section
    Add {
        /// Section name
        name: String,

        /// Project for the section (required)
        #[arg(short, long, required = true)]
        project: String,
    },

    /// Edit a section
    Edit {
        /// Section ID
        section_id: String,

        /// New name
        #[arg(long)]
        name: Option<String>,
    },

    /// Delete a section
    Delete {
        /// Section ID
        section_id: String,

        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
}

/// Comment subcommands
#[derive(Subcommand, Debug)]
pub enum CommentsCommands {
    /// List comments (default)
    List,

    /// Add a comment
    Add {
        /// Comment text
        text: String,

        /// Task ID
        #[arg(long)]
        task: Option<String>,

        /// Project ID
        #[arg(long)]
        project: Option<String>,
    },

    /// Edit a comment
    Edit {
        /// Comment ID
        comment_id: String,

        /// New text
        #[arg(long)]
        text: String,
    },

    /// Delete a comment
    Delete {
        /// Comment ID
        comment_id: String,

        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Attach a file to a comment
    Attach {
        /// File path
        file: String,

        /// Task ID
        #[arg(long)]
        task: Option<String>,

        /// Project ID
        #[arg(long)]
        project: Option<String>,
    },

    /// Download an attachment
    Download {
        /// Attachment ID
        attachment_id: String,

        /// Output path
        #[arg(short, long)]
        output: Option<String>,
    },
}

/// Reminder subcommands
#[derive(Subcommand, Debug)]
pub enum RemindersCommands {
    /// List reminders (default)
    List,

    /// Create a reminder
    Add {
        /// Task ID (required)
        #[arg(long, required = true)]
        task: String,

        /// Absolute due date/time for reminder (e.g., "2025-01-26T10:00:00")
        #[arg(long, conflicts_with = "offset")]
        due: Option<String>,

        /// Minutes before task due time (for relative reminders)
        #[arg(long, conflicts_with = "due")]
        offset: Option<i32>,
    },

    /// Delete a reminder
    Delete {
        /// Reminder ID
        reminder_id: String,

        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
}

/// Filter subcommands
#[derive(Subcommand, Debug)]
pub enum FiltersCommands {
    /// List all filters (default)
    List,

    /// Create a new filter
    Add {
        /// Filter name
        name: String,

        /// Filter query string (e.g., "today & p1")
        #[arg(long)]
        query: String,

        /// Filter color
        #[arg(long)]
        color: Option<String>,

        /// Mark as favorite
        #[arg(long)]
        favorite: bool,
    },

    /// Show filter details
    Show {
        /// Filter ID
        filter_id: String,
    },

    /// Edit a filter
    Edit {
        /// Filter ID
        filter_id: String,

        /// New name
        #[arg(long)]
        name: Option<String>,

        /// New query
        #[arg(long)]
        query: Option<String>,

        /// New color
        #[arg(long)]
        color: Option<String>,

        /// Toggle favorite
        #[arg(long)]
        favorite: Option<bool>,
    },

    /// Delete a filter
    Delete {
        /// Filter ID
        filter_id: String,

        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
}

/// Config subcommands
#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,

    /// Open config in $EDITOR
    Edit,

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,

        /// Configuration value
        value: String,
    },

    /// Print config file path
    Path,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        // This verifies that the CLI is correctly defined
        Cli::command().debug_assert();
    }

    #[test]
    fn test_global_flags() {
        let cli = Cli::parse_from(["td", "--verbose", "list"]);
        assert!(cli.verbose);
        assert!(!cli.quiet);
        assert!(!cli.json);

        let cli = Cli::parse_from(["td", "--quiet", "--json", "list"]);
        assert!(!cli.verbose);
        assert!(cli.quiet);
        assert!(cli.json);
    }

    #[test]
    fn test_no_color_flag() {
        let cli = Cli::parse_from(["td", "--no-color", "list"]);
        assert!(cli.no_color);
    }

    #[test]
    fn test_token_flag() {
        let cli = Cli::parse_from(["td", "--token", "test-token", "list"]);
        assert_eq!(cli.token, Some("test-token".to_string()));
    }

    #[test]
    fn test_list_alias() {
        let cli = Cli::parse_from(["td", "l"]);
        assert!(matches!(cli.command, Some(Commands::List { .. })));
    }

    #[test]
    fn test_add_alias() {
        let cli = Cli::parse_from(["td", "a", "Test task"]);
        assert!(matches!(cli.command, Some(Commands::Add { .. })));
    }

    #[test]
    fn test_done_alias() {
        let cli = Cli::parse_from(["td", "d", "task-id"]);
        assert!(matches!(cli.command, Some(Commands::Done { .. })));
    }

    #[test]
    fn test_show_alias() {
        let cli = Cli::parse_from(["td", "s", "task-id"]);
        assert!(matches!(cli.command, Some(Commands::Show { .. })));
    }

    #[test]
    fn test_edit_alias() {
        let cli = Cli::parse_from(["td", "e", "task-id"]);
        assert!(matches!(cli.command, Some(Commands::Edit { .. })));
    }

    #[test]
    fn test_delete_alias() {
        let cli = Cli::parse_from(["td", "rm", "task-id"]);
        assert!(matches!(cli.command, Some(Commands::Delete { .. })));
    }

    #[test]
    fn test_today_alias() {
        let cli = Cli::parse_from(["td", "t"]);
        assert!(matches!(cli.command, Some(Commands::Today { .. })));
    }

    #[test]
    fn test_quick_alias() {
        let cli = Cli::parse_from(["td", "q", "Buy milk tomorrow"]);
        assert!(matches!(cli.command, Some(Commands::Quick { .. })));
    }

    #[test]
    fn test_projects_alias() {
        let cli = Cli::parse_from(["td", "p"]);
        assert!(matches!(cli.command, Some(Commands::Projects { .. })));
    }

    #[test]
    fn test_labels_alias() {
        let cli = Cli::parse_from(["td", "lb"]);
        assert!(matches!(cli.command, Some(Commands::Labels { .. })));
    }

    #[test]
    fn test_list_with_options() {
        let cli = Cli::parse_from([
            "td",
            "list",
            "--filter",
            "today & p1",
            "--project",
            "Work",
            "--priority",
            "1",
            "--limit",
            "10",
        ]);
        if let Some(Commands::List {
            filter,
            project,
            priority,
            limit,
            ..
        }) = cli.command
        {
            assert_eq!(filter, Some("today & p1".to_string()));
            assert_eq!(project, Some("Work".to_string()));
            assert_eq!(priority, Some(1));
            assert_eq!(limit, 10);
        } else {
            panic!("Expected List command");
        }
    }

    #[test]
    fn test_add_with_labels() {
        let cli = Cli::parse_from([
            "td",
            "add",
            "Test task",
            "-l",
            "urgent",
            "-l",
            "work",
            "-P",
            "1",
        ]);
        if let Some(Commands::Add {
            content,
            label,
            priority,
            ..
        }) = cli.command
        {
            assert_eq!(content, "Test task");
            assert_eq!(label, vec!["urgent", "work"]);
            assert_eq!(priority, Some(1));
        } else {
            panic!("Expected Add command");
        }
    }

    #[test]
    fn test_done_multiple_ids() {
        let cli = Cli::parse_from(["td", "done", "id1", "id2", "id3", "--force"]);
        if let Some(Commands::Done {
            task_ids, force, ..
        }) = cli.command
        {
            assert_eq!(task_ids, vec!["id1", "id2", "id3"]);
            assert!(force);
        } else {
            panic!("Expected Done command");
        }
    }

    #[test]
    fn test_projects_subcommands() {
        let cli = Cli::parse_from(["td", "projects", "add", "New Project", "--favorite"]);
        if let Some(Commands::Projects {
            command: Some(ProjectsCommands::Add { name, favorite, .. }),
        }) = cli.command
        {
            assert_eq!(name, "New Project");
            assert!(favorite);
        } else {
            panic!("Expected Projects Add command");
        }
    }

    #[test]
    fn test_config_subcommands() {
        let cli = Cli::parse_from(["td", "config", "set", "token_storage", "keyring"]);
        if let Some(Commands::Config {
            command: Some(ConfigCommands::Set { key, value }),
        }) = cli.command
        {
            assert_eq!(key, "token_storage");
            assert_eq!(value, "keyring");
        } else {
            panic!("Expected Config Set command");
        }
    }

    #[test]
    fn test_completions() {
        let cli = Cli::parse_from(["td", "completions", "zsh"]);
        if let Some(Commands::Completions { shell }) = cli.command {
            assert!(matches!(shell, Shell::Zsh));
        } else {
            panic!("Expected Completions command");
        }
    }

    #[test]
    fn test_priority_range() {
        // Valid priorities
        assert!(Cli::try_parse_from(["td", "add", "task", "-P", "1"]).is_ok());
        assert!(Cli::try_parse_from(["td", "add", "task", "-P", "4"]).is_ok());

        // Invalid priorities
        assert!(Cli::try_parse_from(["td", "add", "task", "-P", "0"]).is_err());
        assert!(Cli::try_parse_from(["td", "add", "task", "-P", "5"]).is_err());
    }
}
