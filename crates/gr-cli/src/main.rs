use std::process::ExitCode;

use clap::{Parser, Subcommand};
use gr_inspect_codex::{InspectionOutcome, Verdict, inspect_codex as run_codex_inspection};

#[derive(Debug, Parser)]
#[command(name = "gr", version, about = "Goalrail command-line interface")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Inspect {
        #[command(subcommand)]
        target: InspectTarget,
    },
}

#[derive(Debug, Subcommand)]
enum InspectTarget {
    Codex {
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Inspect {
            target: InspectTarget::Codex { json },
        } => inspect_codex(json),
    }
}

fn inspect_codex(json: bool) -> ExitCode {
    let outcome = run_codex_inspection();
    let verdict = render_outcome(json, &outcome);

    ExitCode::from(verdict.exit_code())
}

fn render_outcome(json: bool, outcome: &InspectionOutcome) -> Verdict {
    if json {
        match outcome.to_pretty_json() {
            Ok(output) => println!("{output}"),
            Err(error) => {
                eprintln!("gr inspect codex: failed to serialize JSON report: {error}");
                return Verdict::Incomplete;
            }
        }
    } else {
        let output = outcome.to_human();
        if outcome.is_failure() {
            eprintln!("gr inspect codex: {output}");
        } else {
            print!("{output}");
        }
    }

    outcome.verdict()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_inspection_with_json_output() {
        let cli = Cli::try_parse_from(["gr", "inspect", "codex", "--json"])
            .expect("command should parse");

        assert!(matches!(
            cli.command,
            Command::Inspect {
                target: InspectTarget::Codex { json: true }
            }
        ));
    }
}
