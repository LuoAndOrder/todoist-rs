# Claude Code Skills

This directory contains [Claude Code](https://docs.anthropic.com/en/docs/claude-code) skills for the `td` CLI.

## Installing the Todoist Skill

Copy the skill to your Claude Code skills directory:

```bash
# Create the skills directory if it doesn't exist
mkdir -p ~/.claude/skills

# Copy the todoist skill
cp -r skills/todoist ~/.claude/skills/
```

Or with a one-liner from the repo:

```bash
mkdir -p ~/.claude/skills && cp -r skills/todoist ~/.claude/skills/
```

## What This Enables

Once installed, Claude Code will automatically use the `td` CLI when you ask about:
- "my tasks" / "task list"
- "add a task"
- "complete task"
- "todoist"

The skill teaches Claude the CLI's commands, sync behavior, and filter syntax so it can effectively manage your Todoist tasks.
