# td - Todoist CLI

[![CI](https://github.com/LuoAndOrder/todoist-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/LuoAndOrder/todoist-rs/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/todoist-cli-rs.svg)](https://crates.io/crates/todoist-cli-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A fast, offline-capable command-line interface for Todoist written in Rust.

## Features

- **Offline-first**: Local cache enables instant reads without network calls
- **Sync on demand**: Explicit `--sync` flag or `td sync` command to fetch updates
- **Filter expressions**: Powerful query syntax compatible with Todoist filters
- **Natural language**: Quick-add tasks with dates, projects, and labels
- **Secure token storage**: OS keyring integration (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- **Interactive setup**: First-run wizard guides you through configuration
- **Smart suggestions**: "Did you mean?" hints for project and label typos
- **JSON output**: Machine-readable output for scripting and automation
- **Shell completions**: Tab completion for bash, zsh, fish, and PowerShell

## Project Structure

This is a Cargo workspace with three crates:

| Crate | Description |
|-------|-------------|
| [`todoist-cli-rs`](https://crates.io/crates/todoist-cli-rs) | CLI binary (installs as `td`) |
| [`todoist-api-rs`](https://crates.io/crates/todoist-api-rs) | Todoist Sync API client library |
| [`todoist-cache-rs`](https://crates.io/crates/todoist-cache-rs) | Local cache and filter expression engine |

## Installation

### Pre-built binaries (recommended)

Download the latest release for your platform from [GitHub Releases](https://github.com/LuoAndOrder/todoist-rs/releases):

| Platform | Download |
|----------|----------|
| macOS (Apple Silicon) | `td-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `td-x86_64-apple-darwin.tar.gz` |
| Linux (x64) | `td-x86_64-unknown-linux-gnu.tar.gz` |
| Linux (ARM64) | `td-aarch64-unknown-linux-gnu.tar.gz` |
| Windows (x64) | `td-x86_64-pc-windows-msvc.zip` |

```bash
# macOS/Linux: extract and move to PATH
tar xzf td-*.tar.gz
sudo mv td /usr/local/bin/

# Or install to user directory (no sudo)
mkdir -p ~/.local/bin
mv td ~/.local/bin/
```

### With Cargo

```bash
cargo install todoist-cli-rs
```

### Build from source

```bash
git clone https://github.com/LuoAndOrder/todoist-rs
cd todoist-rs
cargo build --release
```

The binary will be at `target/release/td`.

## Quick Start

### 1. Run any command to start the setup wizard

On first run, `td` will guide you through setup interactively:

```bash
td list
```

The wizard will:
1. Prompt for your API token (get it from [Todoist Settings > Integrations > Developer](https://todoist.com/app/settings/integrations/developer))
2. Validate the token by syncing your data
3. Ask where to store the token (OS keyring, config file, or environment variable)

### Manual setup (alternative)

```bash
# Option 1: Environment variable
export TODOIST_TOKEN="your-api-token"

# Option 2: Save to config
td config set token "your-api-token"
```

### 2. List your tasks

```bash
td list
td today
```

## Sync and Caching

### How the cache works

`td` maintains a local cache of your Todoist data at:

- **macOS/Linux**: `~/.cache/td/cache.json`
- **Windows**: `%LOCALAPPDATA%\td\cache.json`

The cache stores tasks, projects, labels, sections, and other data locally. This enables:

1. **Instant reads**: List and filter commands return immediately without network calls
2. **Offline access**: View your tasks even without internet connectivity
3. **Reduced API usage**: Only sync when you need fresh data

### When to sync

The cache is **not** automatically updated. You control when to sync:

```bash
# Explicit sync command
td sync

# Sync before any command with --sync flag
td list --sync
td today --sync
```

### Sync command options

```bash
# Incremental sync (default) - fast, only fetches changes
td sync

# Full sync - rebuilds cache from scratch
td sync --full
```

Use `--full` when:
- The cache seems corrupted
- You suspect data is out of sync
- After major changes in the Todoist web/mobile app

### The --sync flag

Every read command supports `--sync` to ensure fresh data:

```bash
td list --sync              # Sync then list all tasks
td today --sync             # Sync then show today's agenda
td list -f "p1" --sync      # Sync then filter by priority
td projects --sync          # Sync then list projects
```

**Best practice**: Use `--sync` when you need current data, skip it for speed when working with known tasks.

### Write operations

Commands that modify data (`add`, `done`, `edit`, `delete`) always communicate with the Todoist API directly. They also update the local cache to keep it consistent.

## Commands

### Command Aliases

Most commands have short aliases for quick access:

| Command | Alias | Description |
|---------|-------|-------------|
| `list` | `l` | List tasks |
| `add` | `a` | Add a task |
| `show` | `s` | Show task details |
| `edit` | `e` | Edit a task |
| `done` | `d` | Complete task(s) |
| `delete` | `rm` | Delete task(s) |
| `today` | `t` | Today's agenda |
| `quick` | `q` | Quick add with natural language |
| `projects` | `p` | Manage projects |
| `labels` | `lb` | Manage labels |
| `filters` | `f` | Manage saved filters |

### Task Management

```bash
# List tasks
td list                           # All active tasks (limit 50)
td list --all                     # All tasks, no limit
td list -f "today & p1"           # Filter: today's priority 1 tasks
td list -p "Work"                 # Tasks in Work project
td list -l "urgent"               # Tasks with @urgent label

# Show today's agenda
td today                          # Today's tasks + overdue
td today --no-overdue             # Just today, no overdue
td today --include-upcoming 3     # Include next 3 days

# Add tasks
td add "Buy groceries"
td add "Review PR" -p "Work" -P 1 -d "tomorrow"
td add "Research topic" -l "reading" -l "later"

# Quick add with natural language
td quick "Call mom tomorrow at 5pm #Personal @important"
td quick "Submit report every Friday p1"

# Complete tasks
td done <task-id>
td done <id1> <id2> <id3>         # Complete multiple
td done <id> --all-occurrences    # Complete recurring task permanently

# Edit tasks
td edit <task-id> -c "New content"
td edit <task-id> -d "next week"
td edit <task-id> --add-label "urgent"
td edit <task-id> --no-due        # Remove due date

# Delete tasks
td delete <task-id>

# Show task details
td show <task-id>
td show <task-id> --comments      # Include comments
td show <task-id> --reminders     # Include reminders

# Reopen completed tasks
td reopen <task-id>
```

### Projects

```bash
td projects                       # List all projects
td projects add "New Project"
td projects add "Sub" --parent "Parent Project"
td projects show <id>
td projects edit <id> --name "Renamed"
td projects archive <id>
td projects unarchive <id>
td projects delete <id>
```

### Sections

```bash
td sections                       # List all sections
td sections -p "Work"             # Sections in Work project
td sections add "In Progress" -p "Work"
td sections edit <id> --name "Done"
td sections delete <id>
```

### Labels

```bash
td labels                         # List all labels
td labels add "urgent"
td labels add "context/home" --color red
td labels edit <id> --name "important"
td labels delete <id>
```

### Comments

```bash
td comments --task <task-id>      # Comments on a task
td comments --project <project-id>
td comments add --task <id> "Comment text"
td comments edit <comment-id> "Updated text"
td comments delete <comment-id>
td comments attach --task <id> /path/to/file
td comments download <attachment-url>
```

### Reminders

```bash
td reminders --task <task-id>
td reminders add --task <id> --due "2025-01-15T09:00:00"
td reminders add --task <id> --offset 30   # 30 min before due
td reminders delete <id>
```

### Saved Filters

```bash
td filters                        # List saved filters
td filters add "Work Today" --query "today & #Work"
td filters show <id>
td filters edit <id> --name "New Name"
td filters delete <id>
```

### Configuration

```bash
td config show                    # Show current config
td config edit                    # Open in $EDITOR
td config set token "xxx"         # Set API token
td config path                    # Print config file path
```

#### Token Storage Options

`td` supports three methods for storing your API token:

| Method | Security | Setup |
|--------|----------|-------|
| **OS Keyring** | Most secure | Automatic on supported systems |
| **Config file** | Moderate | `td config set token "xxx"` |
| **Environment** | Varies | `export TODOIST_TOKEN="xxx"` |

The keyring uses:
- **macOS**: Keychain
- **Windows**: Credential Manager
- **Linux**: Secret Service API (requires libsecret/gnome-keyring)

#### Config File Location

| Platform | Path |
|----------|------|
| macOS/Linux | `~/.config/td/config.toml` |
| Windows | `%APPDATA%\td\config.toml` |

### Shell Completions

```bash
# Generate completions
td completions bash > ~/.local/share/bash-completion/completions/td
td completions zsh > ~/.zfunc/_td
td completions fish > ~/.config/fish/completions/td.fish
```

## Filter Expressions

Use the `-f/--filter` flag with `td list` to filter tasks using Todoist's filter syntax.

### Date Filters

| Filter | Description |
|--------|-------------|
| `today` | Tasks due today |
| `tomorrow` | Tasks due tomorrow |
| `overdue` | Tasks past their due date |
| `no date` | Tasks without a due date |
| `7 days` | Tasks due within the next 7 days |
| `Jan 15` | Tasks due on January 15 |
| `December 25` | Tasks due on December 25 |

### Priority Filters

| Filter | Description |
|--------|-------------|
| `p1` | Priority 1 (highest, red) |
| `p2` | Priority 2 (orange) |
| `p3` | Priority 3 (yellow) |
| `p4` | Priority 4 (default, blue) |

### Label Filters

| Filter | Description |
|--------|-------------|
| `@label` | Tasks with the specified label |
| `no labels` | Tasks without any labels |

### Project and Section Filters

| Filter | Description |
|--------|-------------|
| `#Project` | Tasks in exact project |
| `##Project` | Tasks in project and subprojects |
| `/Section` | Tasks in section |

### Boolean Operators

| Operator | Description |
|----------|-------------|
| `&` | AND - both conditions must match |
| `\|` | OR - either condition matches |
| `!` | NOT - negates the condition |
| `()` | Grouping for precedence |

### Examples

```bash
# High priority tasks due today or overdue
td list -f "p1 & (today | overdue)"

# Work tasks without a due date
td list -f "#Work & no date"

# Tasks with urgent label but not in Archive project
td list -f "@urgent & !#Archive"

# Tasks due this week in any Work subproject
td list -f "7 days & ##Work"

# Unlabeled tasks that are overdue
td list -f "no labels & overdue"

# Tasks due on a specific date
td list -f "Jan 15"
td list -f "December 25"
```

## Output Formats

### Human-readable (default for TTY)

Tasks display in a formatted table with colors for priorities and due dates.

### JSON (default for non-TTY, or with --json)

```bash
td list --json
td list --json | jq '.[] | .content'
```

JSON output is automatically enabled when:
- Output is piped to another command
- Output is redirected to a file
- Running in a script

Use `--json` to force JSON output in interactive mode.

### Quiet mode

```bash
td add "Task" -q                  # Only output the task ID
td done <id> -q                   # No output on success
```

## Global Flags

These flags work with any command:

| Flag | Description |
|------|-------------|
| `--sync` | Sync with Todoist before executing |
| `--json` | Force JSON output |
| `--quiet`, `-q` | Quiet mode (errors only) |
| `--verbose`, `-v` | Show debug information |
| `--no-color` | Disable colored output |
| `--token <TOKEN>` | Override API token |

## Environment Variables

| Variable | Description |
|----------|-------------|
| `TODOIST_TOKEN` | API token (alternative to config file) |
| `NO_COLOR` | Disable colored output when set |
| `EDITOR` | Editor for `td config edit` |

## Tips

### Scripting with JSON

```bash
# Get all task IDs from a project
td list -p "Work" --json | jq -r '.[].id'

# Complete all overdue tasks
td list -f "overdue" --json | jq -r '.[].id' | xargs td done

# Export today's tasks
td today --json > today.json
```

### Sync strategies

```bash
# Morning routine: sync once, then work offline
td sync
td today
td list -p "Work"

# Real-time workflow: sync with each command
alias tds="td --sync"
tds list
tds today
```

### Shell aliases

```bash
# Add to your .bashrc or .zshrc
alias tdt="td today"
alias tdl="td list"
alias tda="td add"
alias tdd="td done"
alias tdq="td quick"
alias tds="td sync && td today"
```

## Contributing

Contributions are welcome! Please open an issue or pull request on [GitHub](https://github.com/LuoAndOrder/todoist-rs).

## License

MIT
