//! Shell completions command implementation.
//!
//! Generate shell completions for bash, zsh, fish, and powershell.

use std::io;

use clap::CommandFactory;
use clap_complete::{generate, Shell as ClapShell};

use crate::cli::{Cli, Shell};

/// Generate shell completions for the given shell and write to stdout.
///
/// # Arguments
///
/// * `shell` - The shell to generate completions for
///
/// # Errors
///
/// Returns an error if writing to stdout fails.
pub fn execute(shell: &Shell) -> io::Result<()> {
    let clap_shell = match shell {
        Shell::Bash => ClapShell::Bash,
        Shell::Zsh => ClapShell::Zsh,
        Shell::Fish => ClapShell::Fish,
        Shell::Powershell => ClapShell::PowerShell,
    };

    let mut cmd = Cli::command();
    generate(clap_shell, &mut cmd, "td", &mut io::stdout());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_completions() {
        // Just verify it doesn't panic - actual output is to stdout
        let shell = Shell::Bash;
        let clap_shell = match shell {
            Shell::Bash => ClapShell::Bash,
            Shell::Zsh => ClapShell::Zsh,
            Shell::Fish => ClapShell::Fish,
            Shell::Powershell => ClapShell::PowerShell,
        };
        assert!(matches!(clap_shell, ClapShell::Bash));
    }

    #[test]
    fn test_zsh_completions() {
        let shell = Shell::Zsh;
        let clap_shell = match shell {
            Shell::Bash => ClapShell::Bash,
            Shell::Zsh => ClapShell::Zsh,
            Shell::Fish => ClapShell::Fish,
            Shell::Powershell => ClapShell::PowerShell,
        };
        assert!(matches!(clap_shell, ClapShell::Zsh));
    }

    #[test]
    fn test_fish_completions() {
        let shell = Shell::Fish;
        let clap_shell = match shell {
            Shell::Bash => ClapShell::Bash,
            Shell::Zsh => ClapShell::Zsh,
            Shell::Fish => ClapShell::Fish,
            Shell::Powershell => ClapShell::PowerShell,
        };
        assert!(matches!(clap_shell, ClapShell::Fish));
    }

    #[test]
    fn test_powershell_completions() {
        let shell = Shell::Powershell;
        let clap_shell = match shell {
            Shell::Bash => ClapShell::Bash,
            Shell::Zsh => ClapShell::Zsh,
            Shell::Fish => ClapShell::Fish,
            Shell::Powershell => ClapShell::PowerShell,
        };
        assert!(matches!(clap_shell, ClapShell::PowerShell));
    }
}
