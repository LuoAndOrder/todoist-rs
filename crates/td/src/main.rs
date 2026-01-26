use clap::Parser;

mod cli;

use cli::Cli;

fn main() {
    let cli = Cli::parse();

    // Handle verbosity
    if cli.verbose {
        eprintln!("Verbose mode enabled");
    }

    // For now, just print what command was invoked
    match &cli.command {
        Some(cmd) => {
            if cli.json {
                // JSON output mode
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "not_implemented",
                        "command": format!("{:?}", cmd)
                    })
                );
            } else if !cli.quiet {
                println!("Command: {:?}", cmd);
            }
        }
        None => {
            if !cli.quiet {
                println!("td - Todoist CLI");
                println!("Use --help for usage information");
            }
        }
    }
}
