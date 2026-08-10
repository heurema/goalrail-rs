use std::process::ExitCode;

use clap::{Parser, Subcommand};
use gr_inspect_codex::{
    InspectionOutcome, SkillsInspectionOutcome, Verdict, inspect_codex as run_codex_inspection,
    inspect_codex_skill_actions as run_codex_skill_actions_inspection,
    inspect_codex_skills as run_codex_skills_inspection,
};

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
        #[command(subcommand)]
        section: Option<CodexSection>,
    },
}

#[derive(Debug, Subcommand)]
enum CodexSection {
    Skills {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        actionable: bool,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Inspect {
            target:
                InspectTarget::Codex {
                    json,
                    section: None,
                },
        } => inspect_codex(json),
        Command::Inspect {
            target:
                InspectTarget::Codex {
                    json: outer_json,
                    section:
                        Some(CodexSection::Skills {
                            json: inner_json,
                            actionable,
                        }),
                },
        } => inspect_codex_skills(outer_json || inner_json, actionable),
    }
}

fn inspect_codex_skills(json: bool, actionable: bool) -> ExitCode {
    let outcome = if actionable {
        run_codex_skill_actions_inspection()
    } else {
        run_codex_skills_inspection()
    };
    let verdict = render_skills_outcome(json, &outcome);

    ExitCode::from(verdict.exit_code())
}

fn inspect_codex(json: bool) -> ExitCode {
    let outcome = run_codex_inspection();
    let verdict = render_outcome(json, &outcome);

    ExitCode::from(verdict.exit_code())
}

fn render_skills_outcome(json: bool, outcome: &SkillsInspectionOutcome) -> Verdict {
    if json {
        match outcome.to_pretty_json() {
            Ok(output) => println!("{output}"),
            Err(error) => {
                eprintln!("gr inspect codex skills: failed to serialize JSON report: {error}");
                return Verdict::Incomplete;
            }
        }
    } else {
        let output = outcome.to_human();
        if outcome.is_failure() {
            eprintln!("gr inspect codex skills: {output}");
        } else {
            print!("{output}");
        }
    }

    outcome.verdict()
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
                target: InspectTarget::Codex {
                    json: true,
                    section: None
                }
            }
        ));
    }

    #[test]
    fn parses_skills_drilldown_with_json_output() {
        let cli = Cli::try_parse_from(["gr", "inspect", "codex", "skills", "--json"])
            .expect("command should parse");

        assert!(matches!(
            cli.command,
            Command::Inspect {
                target: InspectTarget::Codex {
                    section: Some(CodexSection::Skills {
                        json: true,
                        actionable: false,
                    }),
                    ..
                }
            }
        ));
    }

    #[test]
    fn parses_skills_drilldown_with_outer_json_output() {
        let cli = Cli::try_parse_from(["gr", "inspect", "codex", "--json", "skills"])
            .expect("command should parse");

        assert!(matches!(
            cli.command,
            Command::Inspect {
                target: InspectTarget::Codex {
                    json: true,
                    section: Some(CodexSection::Skills {
                        json: false,
                        actionable: false,
                    }),
                }
            }
        ));
    }

    #[test]
    fn parses_actionable_skills_drilldown() {
        let cli =
            Cli::try_parse_from(["gr", "inspect", "codex", "skills", "--actionable", "--json"])
                .expect("command should parse");

        assert!(matches!(
            cli.command,
            Command::Inspect {
                target: InspectTarget::Codex {
                    section: Some(CodexSection::Skills {
                        json: true,
                        actionable: true,
                    }),
                    ..
                }
            }
        ));
    }
}
